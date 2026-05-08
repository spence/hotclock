use std::ffi::c_void;
use std::io;
use std::os::raw::{c_int, c_long, c_ulong};
use std::ptr;
use std::sync::atomic::{Ordering, compiler_fence};

#[cfg(target_arch = "x86")]
const SYS_PERF_EVENT_OPEN: c_long = 336;
#[cfg(target_arch = "x86_64")]
const SYS_PERF_EVENT_OPEN: c_long = 298;

const PERF_TYPE_HARDWARE: u32 = 0;
const PERF_COUNT_HW_CPU_CYCLES: u64 = 0;
const PERF_FLAG_FD_CLOEXEC: c_ulong = 1 << 3;
const PERF_ATTR_FLAG_EXCLUDE_KERNEL: u64 = 1 << 5;
const PERF_ATTR_FLAG_EXCLUDE_HV: u64 = 1 << 6;
const PROT_READ: c_int = 0x1;
const MAP_SHARED: c_int = 0x01;
const SC_PAGESIZE: c_int = 30;
const RDPMC_FIXED_CORE_CYCLES: u32 = (1 << 30) | 1;

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
  static PERF_RDPMC: Option<PerfRdpmc> = PerfRdpmc::open().ok();
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn direct_rdpmc_fixed_core_cycles() -> u64 {
  // SAFETY: callers only install this path after `direct_rdpmc_fixed_core_cycles_available`
  // confirms Linux exposes userspace fixed-counter RDPMC for all processes.
  unsafe { rdpmc(RDPMC_FIXED_CORE_CYCLES) }
}

#[must_use]
pub fn direct_rdpmc_fixed_core_cycles_available() -> bool {
  if !cpu_vendor_is_intel() || !direct_rdpmc_enabled_for_all_processes() {
    return false;
  }

  let before = direct_rdpmc_fixed_core_cycles();
  for _ in 0..10_000 {
    std::hint::spin_loop();
  }
  let after = direct_rdpmc_fixed_core_cycles();
  after > before
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn perf_rdpmc_cpu_cycles_checked() -> u64 {
  perf_rdpmc_cpu_cycles().expect("perf RDPMC counter is unavailable")
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn perf_rdpmc_cpu_cycles() -> Option<u64> {
  PERF_RDPMC.with(|counter| counter.as_ref().and_then(PerfRdpmc::read))
}

#[must_use]
pub fn perf_rdpmc_cpu_cycles_available() -> bool {
  PerfRdpmc::open().and_then(|counter| counter.read().ok_or(())).is_ok()
}

impl PerfRdpmc {
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

      let count = if index != 0 && cap_user_rdpmc(capabilities) && pmc_width > 0 {
        // SAFETY: Linux exposes a one-based perf index only when userspace RDPMC is legal.
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
}

impl Drop for PerfRdpmc {
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

fn direct_rdpmc_enabled_for_all_processes() -> bool {
  ["/sys/bus/event_source/devices/cpu/rdpmc", "/sys/devices/cpu/rdpmc"]
    .iter()
    .filter_map(|path| std::fs::read_to_string(path).ok())
    .filter_map(|value| value.trim().parse::<u64>().ok())
    .any(|value| value >= 2)
}

#[cfg(target_arch = "x86_64")]
fn cpu_vendor_is_intel() -> bool {
  use core::arch::x86_64::__cpuid;

  let cpuid = __cpuid(0);
  let mut vendor = [0u8; 12];
  vendor[0..4].copy_from_slice(&cpuid.ebx.to_le_bytes());
  vendor[4..8].copy_from_slice(&cpuid.edx.to_le_bytes());
  vendor[8..12].copy_from_slice(&cpuid.ecx.to_le_bytes());
  vendor == *b"GenuineIntel"
}

#[cfg(target_arch = "x86")]
fn cpu_vendor_is_intel() -> bool {
  use core::arch::x86::__cpuid;

  let cpuid = __cpuid(0);
  let mut vendor = [0u8; 12];
  vendor[0..4].copy_from_slice(&cpuid.ebx.to_le_bytes());
  vendor[4..8].copy_from_slice(&cpuid.edx.to_le_bytes());
  vendor[8..12].copy_from_slice(&cpuid.ecx.to_le_bytes());
  vendor == *b"GenuineIntel"
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
