use std::ffi::c_void;
use std::io;
use std::os::raw::{c_int, c_long, c_ulong};
use std::ptr;
use std::sync::atomic::{Ordering, compiler_fence};

const SYS_PERF_EVENT_OPEN: c_long = 241;

const PERF_TYPE_HARDWARE: u32 = 0;
const PERF_COUNT_HW_CPU_CYCLES: u64 = 0;
const PERF_FLAG_FD_CLOEXEC: c_ulong = 1 << 3;
const PERF_ATTR_FLAG_EXCLUDE_KERNEL: u64 = 1 << 5;
const PERF_ATTR_FLAG_EXCLUDE_HV: u64 = 1 << 6;
const PROT_READ: c_int = 0x1;
const MAP_SHARED: c_int = 0x01;
const SC_PAGESIZE: c_int = 30;

// Linux perf maps the dedicated PMUv3 cycle counter (PMCCNTR_EL0) at index 32 in the
// mmap page when userspace access is enabled.
const ARM_PMU_CYCLE_COUNTER_INDEX: u32 = 32;

#[repr(C)]
#[derive(Default)]
struct PerfEventAttr {
  type_: u32,
  size: u32,
  config: u64,
  sample_period: u64,
  sample_type: u64,
  read_format: u64,
  flags: u64,
  wakeup_events: u32,
  bp_type: u32,
  bp_addr: u64,
  bp_len: u64,
}

#[repr(C)]
struct PerfEventMmapPage {
  version: u32,
  compat_version: u32,
  lock: u32,
  index: u32,
  offset: i64,
  time_enabled: u64,
  time_running: u64,
  capabilities: u64,
  pmc_width: u16,
  time_shift: u16,
  time_mult: u32,
  time_offset: u64,
}

struct PerfPmccntr {
  fd: c_int,
  page: *mut PerfEventMmapPage,
  len: usize,
}

unsafe extern "C" {
  fn syscall(number: c_long, ...) -> c_long;
  fn mmap(
    addr: *mut c_void,
    length: usize,
    prot: c_int,
    flags: c_int,
    fd: c_int,
    offset: c_long,
  ) -> *mut c_void;
  fn munmap(addr: *mut c_void, length: usize) -> c_int;
  fn close(fd: c_int) -> c_int;
  fn sysconf(name: c_int) -> c_long;
}

thread_local! {
  static PERF_PMCCNTR: Option<PerfPmccntr> = PerfPmccntr::open().ok();
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn perf_pmccntr_cpu_cycles_checked() -> u64 {
  perf_pmccntr_cpu_cycles().expect("perf PMCCNTR counter is unavailable")
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn perf_pmccntr_cpu_cycles() -> Option<u64> {
  PERF_PMCCNTR.with(|counter| counter.as_ref().and_then(PerfPmccntr::read))
}

#[must_use]
pub fn perf_pmccntr_cpu_cycles_available() -> bool {
  PerfPmccntr::open().and_then(|counter| counter.read().ok_or(())).is_ok()
}

impl PerfPmccntr {
  fn open() -> Result<Self, ()> {
    let fd = open_perf_event().map_err(|_| ())?;
    let len = page_size().map_err(|_| ())?;
    let page = mmap_perf_page(fd, len).map_err(|_| {
      close_fd(fd);
    })?;

    let counter = Self { fd, page, len };
    if counter.read().is_some() { Ok(counter) } else { Err(()) }
  }

  #[inline(always)]
  fn read(&self) -> Option<u64> {
    for _ in 0..100 {
      let snapshot = self.snapshot();
      if let Some(count) = snapshot.count {
        return Some(count);
      }
    }
    None
  }

  #[inline(always)]
  fn snapshot(&self) -> PerfSnapshot {
    let page = self.page;

    loop {
      // SAFETY: `page` points at a live perf metadata page owned by this handle.
      let sequence = unsafe { ptr::read_volatile(ptr::addr_of!((*page).lock)) };
      compiler_fence(Ordering::Acquire);

      // SAFETY: all reads below access fields inside the same live perf metadata page.
      let index = unsafe { ptr::read_volatile(ptr::addr_of!((*page).index)) };
      // SAFETY: see safety note for `index`.
      let offset = unsafe { ptr::read_volatile(ptr::addr_of!((*page).offset)) };
      // SAFETY: see safety note for `index`.
      let capabilities = unsafe { ptr::read_volatile(ptr::addr_of!((*page).capabilities)) };
      // SAFETY: see safety note for `index`.
      let pmc_width = unsafe { ptr::read_volatile(ptr::addr_of!((*page).pmc_width)) };

      let count = if index == ARM_PMU_CYCLE_COUNTER_INDEX
        && cap_user_rdpmc(capabilities)
        && pmc_width > 0
      {
        // SAFETY: Linux only assigns the PMUv3 cycle counter index when user-mode PMCCNTR_EL0
        // access is enabled (PMUSERENR_EL0 set via /proc/sys/kernel/perf_user_access).
        let pmc = unsafe { read_pmccntr() };
        Some(offset.wrapping_add(sign_extend(pmc, pmc_width)) as u64)
      } else {
        None
      };

      compiler_fence(Ordering::Acquire);
      // SAFETY: `page` is still the same live perf metadata page.
      let after = unsafe { ptr::read_volatile(ptr::addr_of!((*page).lock)) };

      if sequence == after && sequence & 1 == 0 {
        return PerfSnapshot { count };
      }
    }
  }
}

impl Drop for PerfPmccntr {
  fn drop(&mut self) {
    // SAFETY: `page` was returned by `mmap` with `len` and has not been unmapped yet.
    unsafe {
      munmap(self.page.cast::<c_void>(), self.len);
    }
    close_fd(self.fd);
  }
}

struct PerfSnapshot {
  count: Option<u64>,
}

fn open_perf_event() -> io::Result<c_int> {
  let attr = PerfEventAttr {
    type_: PERF_TYPE_HARDWARE,
    size: std::mem::size_of::<PerfEventAttr>() as u32,
    config: PERF_COUNT_HW_CPU_CYCLES,
    flags: PERF_ATTR_FLAG_EXCLUDE_KERNEL | PERF_ATTR_FLAG_EXCLUDE_HV,
    ..PerfEventAttr::default()
  };

  // SAFETY: raw `perf_event_open`; `attr` is a valid prefix, pid=0 is current thread,
  // cpu=-1 is any CPU, and group_fd=-1 opens a standalone event.
  let fd = unsafe {
    syscall(
      SYS_PERF_EVENT_OPEN,
      ptr::addr_of!(attr),
      0 as c_long,
      -1 as c_long,
      -1 as c_long,
      PERF_FLAG_FD_CLOEXEC,
    )
  };

  if fd < 0 { Err(io::Error::last_os_error()) } else { Ok(fd as c_int) }
}

fn mmap_perf_page(fd: c_int, len: usize) -> io::Result<*mut PerfEventMmapPage> {
  // SAFETY: maps the perf metadata page for a live perf event fd.
  let page = unsafe { mmap(ptr::null_mut(), len, PROT_READ, MAP_SHARED, fd, 0) };
  if page as isize == -1 {
    Err(io::Error::last_os_error())
  } else {
    Ok(page.cast::<PerfEventMmapPage>())
  }
}

fn close_fd(fd: c_int) {
  // SAFETY: best-effort close of a file descriptor owned by this handle.
  unsafe {
    close(fd);
  }
}

fn page_size() -> io::Result<usize> {
  // SAFETY: `sysconf(_SC_PAGESIZE)` has no Rust-side safety preconditions.
  let value = unsafe { sysconf(SC_PAGESIZE) };
  if value <= 0 {
    Err(io::Error::last_os_error())
  } else {
    usize::try_from(value).map_err(|_| io::Error::last_os_error())
  }
}

#[inline(always)]
unsafe fn read_pmccntr() -> u64 {
  let count: u64;
  // SAFETY: callers must only invoke this when Linux has enabled userspace PMCCNTR_EL0
  // access. The perf mmap page only reports the PMUv3 cycle-counter index after the kernel
  // sets PMUSERENR_EL0 to permit user-mode reads.
  unsafe {
    core::arch::asm!(
      "mrs {}, pmccntr_el0",
      out(reg) count,
      options(nomem, nostack, preserves_flags)
    );
  }
  count
}

#[inline(always)]
fn cap_user_rdpmc(capabilities: u64) -> bool {
  const CAP_LEGACY_USER_RDPMC: u64 = 1 << 0;
  const CAP_BIT0_IS_DEPRECATED: u64 = 1 << 1;
  const CAP_USER_RDPMC: u64 = 1 << 2;

  if capabilities & CAP_BIT0_IS_DEPRECATED != 0 {
    capabilities & CAP_USER_RDPMC != 0
  } else {
    capabilities & CAP_LEGACY_USER_RDPMC != 0
  }
}

#[inline(always)]
fn sign_extend(value: u64, width: u16) -> i64 {
  let shift = 64u32.saturating_sub(u32::from(width));
  ((value << shift) as i64) >> shift
}
