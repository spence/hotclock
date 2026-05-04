use std::ffi::c_void;
use std::io;
use std::os::raw::{c_int, c_long, c_ulong};
use std::ptr;
use std::sync::OnceLock;
use std::sync::atomic::{Ordering, compiler_fence};

const SYS_PERF_EVENT_OPEN: c_long = 298;
const PERF_TYPE_HARDWARE: u32 = 0;
const PERF_COUNT_HW_CPU_CYCLES: u64 = 0;
const PERF_FLAG_FD_CLOEXEC: c_ulong = 1 << 3;
const PERF_ATTR_FLAG_EXCLUDE_KERNEL: u64 = 1 << 5;
const PERF_ATTR_FLAG_EXCLUDE_HV: u64 = 1 << 6;
const PROT_READ: c_int = 0x1;
const MAP_SHARED: c_int = 0x01;
const PAGE_SIZE: usize = 4096;
const FIXED_CORE_CYCLES: u32 = (1 << 30) | 1;

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

struct PerfRdpmc {
  fd: c_int,
  page: *mut PerfEventMmapPage,
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
}

thread_local! {
  static PERF_RDPMC: PerfRdpmc = PerfRdpmc::open();
}

#[inline(always)]
pub fn rdpmc_fixed_core_cycles() -> u64 {
  static DIRECT_RDPMC_WORKS: OnceLock<()> = OnceLock::new();
  DIRECT_RDPMC_WORKS.get_or_init(validate_direct_fixed_rdpmc);
  // SAFETY: validation checks that Linux allows direct userspace RDPMC and that
  // the fixed-function core-cycle counter is advancing in this process.
  unsafe { rdpmc(FIXED_CORE_CYCLES) }
}

#[inline(always)]
pub fn perf_rdpmc_cpu_cycles() -> u64 {
  PERF_RDPMC.with(PerfRdpmc::read)
}

fn validate_direct_fixed_rdpmc() {
  if !direct_rdpmc_enabled_for_all_processes() {
    panic!("direct RDPMC requires /sys/bus/event_source/devices/cpu/rdpmc >= 2");
  }

  // SAFETY: the sysfs policy check above confirms direct userspace RDPMC is enabled.
  let before = unsafe { rdpmc(FIXED_CORE_CYCLES) };
  for _ in 0..10_000 {
    std::hint::spin_loop();
  }
  // SAFETY: the same validated RDPMC policy applies to this second read.
  let after = unsafe { rdpmc(FIXED_CORE_CYCLES) };
  if after <= before {
    panic!("direct RDPMC fixed core-cycle counter did not advance");
  }
}

fn direct_rdpmc_enabled_for_all_processes() -> bool {
  ["/sys/bus/event_source/devices/cpu/rdpmc", "/sys/devices/cpu/rdpmc"]
    .iter()
    .filter_map(|path| std::fs::read_to_string(path).ok())
    .filter_map(|value| value.trim().parse::<u64>().ok())
    .any(|value| value >= 2)
}

impl PerfRdpmc {
  fn open() -> Self {
    let fd = open_perf_event().unwrap_or_else(|error| panic!("perf_event_open failed: {error}"));
    let page = mmap_perf_page(fd).unwrap_or_else(|error| {
      close_fd(fd);
      panic!("perf_event mmap failed: {error}");
    });

    let counter = Self { fd, page };
    counter.assert_rdpmc_available();
    counter
  }

  #[inline(always)]
  fn read(&self) -> u64 {
    for _ in 0..100 {
      let snapshot = self.snapshot();
      if let Some(count) = snapshot.count {
        return count;
      }
    }
    panic!("perf RDPMC counter was not scheduled");
  }

  #[inline(always)]
  fn snapshot(&self) -> PerfSnapshot {
    let page = self.page;

    loop {
      // SAFETY: `page` points at a live perf metadata page owned by this thread-local handle.
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

      let count = if index != 0 && cap_user_rdpmc(capabilities) && pmc_width > 0 {
        // SAFETY: `cap_user_rdpmc` and a nonzero perf index are Linux's contract that RDPMC
        // may read this event from userspace; perf exposes a one-based index, RDPMC wants zero.
        let pmc = unsafe { rdpmc(index - 1) };
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

  fn assert_rdpmc_available(&self) {
    let snapshot = self.snapshot();
    if snapshot.count.is_none() {
      panic!("perf event does not expose userspace RDPMC");
    }
  }
}

impl Drop for PerfRdpmc {
  fn drop(&mut self) {
    // SAFETY: `page` was returned by `mmap` with `PAGE_SIZE` and has not been unmapped yet.
    unsafe {
      munmap(self.page.cast::<c_void>(), PAGE_SIZE);
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

  // SAFETY: this is the raw `perf_event_open` syscall. `attr` points to a valid
  // `perf_event_attr` prefix, pid=0 means current thread, cpu=-1 means any CPU.
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

fn mmap_perf_page(fd: c_int) -> io::Result<*mut PerfEventMmapPage> {
  // SAFETY: maps the perf metadata page for a live perf event fd.
  let page = unsafe { mmap(ptr::null_mut(), PAGE_SIZE, PROT_READ, MAP_SHARED, fd, 0) };
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

#[inline(always)]
unsafe fn rdpmc(counter: u32) -> u64 {
  let low: u32;
  let high: u32;
  // SAFETY: callers must ensure Linux has enabled userspace RDPMC for the requested counter.
  unsafe {
    core::arch::asm!(
      "rdpmc",
      in("ecx") counter,
      out("eax") low,
      out("edx") high,
      options(nomem, nostack, preserves_flags)
    );
  }
  (u64::from(high) << 32) | u64::from(low)
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
