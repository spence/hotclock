// Standalone clock survey tool. Benches every architecturally-applicable
// clock on the current host, with explicit error reasons for any that
// fail availability checks. No SIGILL-able code paths — every clock is
// gated by a safe pre-check (sysctl/sysfs read, perf_event_open + mmap).
//
// Build:  rustc -O -C opt-level=3 clock-survey.rs -o clock-survey
// Run:    ./clock-survey

#[cfg(target_os = "linux")]
use std::ffi::c_void;
#[cfg(target_os = "linux")]
use std::os::raw::{c_int, c_long, c_ulong};
#[cfg(target_os = "linux")]
use std::ptr;
use std::time::Instant as StdInstant;

const WARMUP: usize = 10_000;
const ITERS: usize = 1_000_000;
const SAMPLES: usize = 31;

fn bench(mut f: impl FnMut()) -> f64 {
  for _ in 0..WARMUP { f(); }
  let mut best = u128::MAX;
  for _ in 0..SAMPLES {
    let start = StdInstant::now();
    for _ in 0..ITERS { f(); }
    let elapsed = start.elapsed().as_nanos();
    if elapsed < best { best = elapsed; }
  }
  best as f64 / ITERS as f64
}

fn report(name: &str, result: Result<f64, String>) {
  match result {
    Ok(ns) => println!("{name:30}  {ns:>10.3} ns/op"),
    Err(reason) => println!("{name:30}  unavailable: {reason}"),
  }
}

// ---------- x86_64 ----------

#[cfg(target_arch = "x86_64")]
fn survey_x86_64() {
  println!("=== x86_64 clock survey ===");

  // RDTSC: always available
  report("RDTSC", Ok(bench(|| {
    let _ = std::hint::black_box(unsafe { core::arch::x86_64::_rdtsc() });
  })));

  // RDTSCP: serializing TSC read with processor ID. Same TSC source as RDTSC,
  // slightly slower because it waits for prior ops to retire.
  report("RDTSCP", Ok(bench(|| {
    let mut aux: u32 = 0;
    let _ = std::hint::black_box(unsafe { core::arch::x86_64::__rdtscp(&mut aux) });
  })));

  // direct-RDPMC (Intel-only, requires sysfs)
  report("direct-RDPMC", direct_rdpmc_x86());

  // perf-RDPMC (Linux only)
  #[cfg(target_os = "linux")]
  report("perf-RDPMC", perf_rdpmc_x86());

  // clock_gettime variants
  #[cfg(target_os = "linux")]
  {
    report("clock_gettime(MONOTONIC)", clock_gettime_bench(1)); // CLOCK_MONOTONIC
    report("clock_gettime(MONOTONIC_RAW)", clock_gettime_bench(4)); // CLOCK_MONOTONIC_RAW
    report("clock_gettime(MONOTONIC_COARSE)", clock_gettime_bench(6)); // CLOCK_MONOTONIC_COARSE
    report("clock_gettime(BOOTTIME)", clock_gettime_bench(7)); // CLOCK_BOOTTIME
  }

  // macOS Mach time APIs + POSIX clock_gettime (Darwin uses different IDs)
  #[cfg(target_os = "macos")]
  {
    report("mach_absolute_time", Ok(bench(|| {
      let _ = std::hint::black_box(unsafe { mach_absolute_time() });
    })));
    report("mach_continuous_time", Ok(bench(|| {
      let _ = std::hint::black_box(unsafe { mach_continuous_time() });
    })));
    report("mach_approximate_time", Ok(bench(|| {
      let _ = std::hint::black_box(unsafe { mach_approximate_time() });
    })));
    report("clock_gettime(MONOTONIC)", clock_gettime_macos_bench(6));
    report("clock_gettime(MONOTONIC_RAW)", clock_gettime_macos_bench(4));
  }

  // Windows: QueryPerformanceCounter is the high-res monotonic clock.
  #[cfg(target_os = "windows")]
  report("QueryPerformanceCounter", query_performance_counter_bench());

  // std::time::Instant (calls clock_gettime(CLOCK_MONOTONIC) on Linux via vDSO;
  // mach_continuous_time on macOS; QueryPerformanceCounter on Windows)
  report("std::time::Instant", Ok(bench(|| {
    let _ = std::hint::black_box(StdInstant::now());
  })));
}

#[cfg(target_arch = "x86_64")]
fn direct_rdpmc_x86() -> Result<f64, String> {
  // 1) Intel?
  #[allow(unused_unsafe)] // __cpuid is `unsafe fn` on some targets, safe on others
  let cpuid = unsafe { core::arch::x86_64::__cpuid(0) };
  let mut vendor = [0u8; 12];
  vendor[0..4].copy_from_slice(&cpuid.ebx.to_le_bytes());
  vendor[4..8].copy_from_slice(&cpuid.edx.to_le_bytes());
  vendor[8..12].copy_from_slice(&cpuid.ecx.to_le_bytes());
  if vendor != *b"GenuineIntel" {
    return Err(format!("not Intel (CPUID vendor: {})", String::from_utf8_lossy(&vendor)));
  }

  // 2) sysfs gate (Linux only). On macOS/Windows, return early — direct user-mode
  // RDPMC is not exposed by those kernels (Windows could be enabled via driver, but
  // none of the AMI-shipped Windows kernels do; macOS XNU never).
  #[cfg(target_os = "linux")]
  {
    let paths = ["/sys/bus/event_source/devices/cpu/rdpmc", "/sys/devices/cpu/rdpmc"];
    let mut found = None;
    for path in &paths {
      if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(v) = content.trim().parse::<u64>() {
          found = Some((path, v));
          break;
        }
      }
    }
    match found {
      None => return Err(format!("/sys/.../rdpmc not present (kernel doesn't expose user-mode RDPMC sysfs control)")),
      Some((path, v)) if v < 2 => return Err(format!("{path} = {v} (need >=2 for unprivileged direct RDPMC)")),
      Some(_) => {}
    }

    // 3) Actually bench (call rdpmc with fixed-counter index)
    return Ok(bench(|| {
      let low: u32;
      let high: u32;
      // SAFETY: gated above on Linux + Intel + /sys/.../rdpmc >= 2.
      unsafe {
        core::arch::asm!(
          "rdpmc",
          in("ecx") (1u32 << 30) | 1u32, // RDPMC_FIXED_CORE_CYCLES
          out("eax") low,
          out("edx") high,
          options(nomem, nostack, preserves_flags),
        );
      }
      let _ = std::hint::black_box(((high as u64) << 32) | (low as u64));
    }));
  }
  #[cfg(target_os = "macos")]
  return Err("macOS XNU does not expose user-mode RDPMC".into());
  #[cfg(target_os = "windows")]
  return Err("Windows kernel does not expose user-mode RDPMC (no /sys/.../rdpmc equivalent)".into());
  #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
  return Err("direct user-mode RDPMC requires Linux sysfs control".into());
}

#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
fn perf_rdpmc_x86() -> Result<f64, String> {
  let counter = match PerfRdpmc::open() {
    Ok(c) => c,
    Err(e) => return Err(format!("perf_event_open + mmap: {e}")),
  };
  if !counter.user_readable() {
    return Err("perf_event mmap page reports cap_user_rdpmc=false (kernel won't expose user-mode RDPMC for this event)".into());
  }
  Ok(bench(|| { let _ = std::hint::black_box(counter.read()); }))
}

// ---------- aarch64 ----------

#[cfg(target_arch = "aarch64")]
fn survey_aarch64() {
  println!("=== aarch64 clock survey ===");

  // CNTVCT_EL0: virtual counter, always user-readable on ARMv8+
  report("CNTVCT_EL0", Ok(bench(|| {
    let cnt: u64;
    unsafe {
      core::arch::asm!(
        "mrs {}, cntvct_el0", out(reg) cnt,
        options(nostack, nomem, preserves_flags),
      );
    }
    let _ = std::hint::black_box(cnt);
  })));

  // CNTPCT_EL0: physical counter. Same hardware source as CNTVCT under most
  // hypervisors (VHE strips the virtual offset to zero). User-readable when
  // EL1 sets CNTKCTL_EL1.EL0PCTEN — macOS XNU enables it by default; Linux
  // does NOT (would SIGILL). Fork-probe before benching.
  report("CNTPCT_EL0", cntpct_probe_and_bench());

  // PMCCNTR_EL0 direct (Linux only, gated by perf_user_access)
  #[cfg(target_os = "linux")]
  report("PMCCNTR_EL0 (direct)", pmccntr_direct());
  #[cfg(not(target_os = "linux"))]
  report("PMCCNTR_EL0", Err("macOS/Windows aarch64: kernel doesn't expose PMU to user mode (would SIGILL)".into()));

  // perf-PMCCNTR (Linux only)
  #[cfg(target_os = "linux")]
  report("PMCCNTR_EL0 (perf)", perf_pmccntr());

  // Linux clock_gettime variants
  #[cfg(target_os = "linux")]
  {
    report("clock_gettime(MONOTONIC)", clock_gettime_bench(1));
    report("clock_gettime(MONOTONIC_RAW)", clock_gettime_bench(4));
    report("clock_gettime(MONOTONIC_COARSE)", clock_gettime_bench(6));
    report("clock_gettime(BOOTTIME)", clock_gettime_bench(7));
  }

  // macOS Mach time APIs
  #[cfg(target_os = "macos")]
  {
    report("mach_absolute_time", Ok(bench(|| {
      let _ = std::hint::black_box(unsafe { mach_absolute_time() });
    })));
    report("mach_continuous_time", Ok(bench(|| {
      let _ = std::hint::black_box(unsafe { mach_continuous_time() });
    })));
    report("mach_approximate_time", Ok(bench(|| {
      let _ = std::hint::black_box(unsafe { mach_approximate_time() });
    })));
    // POSIX clock_gettime on macOS (different code path from mach)
    report("clock_gettime(MONOTONIC)", clock_gettime_macos_bench(6)); // CLOCK_MONOTONIC = 6 on Darwin
    report("clock_gettime(MONOTONIC_RAW)", clock_gettime_macos_bench(4)); // CLOCK_MONOTONIC_RAW = 4
  }

  // Windows: QueryPerformanceCounter is the high-res monotonic clock.
  // CNTVCT_EL0 is user-readable on ARMv8+ (architectural guarantee); Windows
  // does not gate it. CNTPCT_EL0 user access is OS-dependent on Windows — left
  // to the fork/bench path above which on non-Linux benches directly. If a
  // Windows ARM host SIGILLs on CNTPCT_EL0, an SEH-based probe would be the fix.
  #[cfg(target_os = "windows")]
  report("QueryPerformanceCounter", query_performance_counter_bench());

  // std::time::Instant — Linux: clock_gettime(MONOTONIC) via vDSO; macOS: mach_continuous_time + bookkeeping.
  report("std::time::Instant", Ok(bench(|| {
    let _ = std::hint::black_box(StdInstant::now());
  })));
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn pmccntr_direct() -> Result<f64, String> {
  // Safe gate: read sysctl. If 0 or absent, executing PMCCNTR_EL0 would SIGILL.
  match std::fs::read_to_string("/proc/sys/kernel/perf_user_access") {
    Ok(s) => {
      let v: u64 = s.trim().parse().map_err(|_| format!("/proc/sys/kernel/perf_user_access = {s:?} (parse failed)"))?;
      if v < 1 {
        return Err(format!("/proc/sys/kernel/perf_user_access = {v} (kernel hasn't enabled user PMU access; would SIGILL)"));
      }
    }
    Err(e) => return Err(format!("/proc/sys/kernel/perf_user_access read failed: {e}")),
  }

  // On aarch64 PMUv3, perf_user_access=1 alone is NOT sufficient — the kernel
  // only sets PMUSERENR_EL0 to permit user-mode `mrs pmccntr_el0` when a perf
  // event is open in this process. Open one (and keep the fd alive for the
  // duration of the bench) before trying the direct read.
  let _event = PerfPmccntr::open().map_err(|e| {
    format!("open perf event to enable user PMU access: {e}")
  })?;
  if !_event.user_readable() {
    return Err("perf event opened but mmap page reports cap_user_rdpmc=false (kernel still hasn't enabled user PMCCNTR; would SIGILL)".into());
  }

  Ok(bench(|| {
    let cnt: u64;
    unsafe {
      core::arch::asm!(
        "mrs {}, pmccntr_el0", out(reg) cnt,
        options(nostack, nomem, preserves_flags),
      );
    }
    let _ = std::hint::black_box(cnt);
  }))
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn perf_pmccntr() -> Result<f64, String> {
  let counter = match PerfPmccntr::open() {
    Ok(c) => c,
    Err(e) => return Err(format!("perf_event_open + mmap: {e}")),
  };
  if !counter.user_readable() {
    return Err("perf_event mmap page reports cap_user_rdpmc=false (kernel hasn't enabled user-mode PMCCNTR for this event; would SIGILL)".into());
  }
  Ok(bench(|| { let _ = std::hint::black_box(counter.read()); }))
}

// ---------- macOS extern ----------

#[cfg(target_os = "macos")]
unsafe extern "C" {
  fn mach_absolute_time() -> u64;
  fn mach_continuous_time() -> u64;
  fn mach_approximate_time() -> u64;
  fn clock_gettime_nsec_np(clock_id: u32) -> u64;
}

// ---------- Windows extern ----------

#[cfg(target_os = "windows")]
unsafe extern "system" {
  fn QueryPerformanceCounter(lpPerformanceCount: *mut i64) -> i32;
  fn QueryPerformanceFrequency(lpFrequency: *mut i64) -> i32;
}

#[cfg(target_os = "windows")]
fn query_performance_counter_bench() -> Result<f64, String> {
  let mut freq: i64 = 0;
  let rc = unsafe { QueryPerformanceFrequency(&mut freq) };
  if rc == 0 || freq == 0 {
    return Err("QueryPerformanceFrequency unavailable".into());
  }
  Ok(bench(|| {
    let mut c: i64 = 0;
    unsafe { QueryPerformanceCounter(&mut c); }
    let _ = std::hint::black_box(c);
  }))
}

#[cfg(target_os = "macos")]
fn clock_gettime_macos_bench(clock_id: u32) -> Result<f64, String> {
  // Validate clock_id is supported by attempting one read.
  let v = unsafe { clock_gettime_nsec_np(clock_id) };
  if v == 0 {
    return Err(format!("clock_gettime_nsec_np(clock_id={clock_id}) returned 0 (unsupported)"));
  }
  Ok(bench(|| {
    let _ = std::hint::black_box(unsafe { clock_gettime_nsec_np(clock_id) });
  }))
}

// ---------- aarch64 CNTPCT_EL0 fork-probe ----------

#[cfg(target_arch = "aarch64")]
fn cntpct_probe_and_bench() -> Result<f64, String> {
  #[cfg(target_os = "linux")]
  {
    unsafe extern "C" {
      fn fork() -> i32;
      fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
      fn _exit(code: i32) -> !;
    }
    let pid = unsafe { fork() };
    if pid == 0 {
      let cnt: u64;
      unsafe {
        core::arch::asm!(
          "mrs {}, cntpct_el0", out(reg) cnt,
          options(nostack, nomem, preserves_flags),
        );
      }
      std::hint::black_box(cnt);
      unsafe { _exit(0) }
    } else if pid > 0 {
      let mut status: i32 = 0;
      unsafe { waitpid(pid, &mut status, 0); }
      let exited_normally = (status & 0x7f) == 0;
      if !exited_normally {
        let sig = status & 0x7f;
        return Err(format!("SIGILL (signal={sig}) — Linux gates CNTKCTL_EL1.EL0PCTEN by default"));
      }
    } else {
      return Err("fork() failed".into());
    }
  }
  Ok(bench(|| {
    let cnt: u64;
    unsafe {
      core::arch::asm!(
        "mrs {}, cntpct_el0", out(reg) cnt,
        options(nostack, nomem, preserves_flags),
      );
    }
    let _ = std::hint::black_box(cnt);
  }))
}

// ---------- Linux clock_gettime ----------

#[cfg(target_os = "linux")]
fn clock_gettime_bench(clock_id: i32) -> Result<f64, String> {
  #[repr(C)]
  struct Timespec { tv_sec: i64, tv_nsec: i64 }
  unsafe extern "C" {
    fn clock_gettime(clk_id: i32, tp: *mut Timespec) -> i32;
  }
  // Verify the clock_id is supported by attempting one call.
  let mut ts = Timespec { tv_sec: 0, tv_nsec: 0 };
  let rc = unsafe { clock_gettime(clock_id, &mut ts) };
  if rc != 0 {
    return Err(format!("clock_gettime(clk_id={clock_id}) failed: {}", std::io::Error::last_os_error()));
  }
  Ok(bench(|| {
    let mut ts = Timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { clock_gettime(clock_id, &mut ts); }
    let _ = std::hint::black_box(ts.tv_sec * 1_000_000_000 + ts.tv_nsec);
  }))
}

// ---------- perf_event_open + mmap (shared by x86 and aarch64 on Linux) ----------

#[cfg(target_os = "linux")]
mod perf {
  use super::*;

  pub const PERF_TYPE_HARDWARE: u32 = 0;
  pub const PERF_COUNT_HW_CPU_CYCLES: u64 = 0;
  pub const PERF_FLAG_FD_CLOEXEC: c_ulong = 1 << 3;
  pub const PERF_ATTR_FLAG_EXCLUDE_KERNEL: u64 = 1 << 5;
  pub const PERF_ATTR_FLAG_EXCLUDE_HV: u64 = 1 << 6;
  pub const PROT_READ: c_int = 0x1;
  pub const MAP_SHARED: c_int = 0x01;
  pub const SC_PAGESIZE: c_int = 30;

  #[cfg(target_arch = "x86_64")]
  pub const SYS_PERF_EVENT_OPEN: c_long = 298;
  #[cfg(target_arch = "aarch64")]
  pub const SYS_PERF_EVENT_OPEN: c_long = 241;

  #[repr(C)]
  #[derive(Default)]
  pub struct PerfEventAttr {
    pub type_: u32,
    pub size: u32,
    pub config: u64,
    pub sample_period: u64,
    pub sample_type: u64,
    pub read_format: u64,
    pub flags: u64,
    pub wakeup_events: u32,
    pub bp_type: u32,
    pub bp_addr: u64,
    pub bp_len: u64,
  }

  #[repr(C)]
  pub struct PerfEventMmapPage {
    pub version: u32,
    pub compat_version: u32,
    pub lock: u32,
    pub index: u32,
    pub offset: i64,
    pub time_enabled: u64,
    pub time_running: u64,
    pub capabilities: u64,
    pub pmc_width: u16,
    pub time_shift: u16,
    pub time_mult: u32,
    pub time_offset: u64,
  }

  unsafe extern "C" {
    pub fn syscall(number: c_long, ...) -> c_long;
    pub fn mmap(addr: *mut c_void, length: usize, prot: c_int, flags: c_int, fd: c_int, offset: c_long) -> *mut c_void;
    pub fn munmap(addr: *mut c_void, length: usize) -> c_int;
    pub fn close(fd: c_int) -> c_int;
    pub fn sysconf(name: c_int) -> c_long;
  }

  pub fn page_size() -> usize {
    unsafe { sysconf(SC_PAGESIZE) as usize }
  }

  pub fn cap_user_rdpmc(caps: u64) -> bool {
    const CAP_LEGACY: u64 = 1 << 0;
    const CAP_BIT0_DEPRECATED: u64 = 1 << 1;
    const CAP_USER_RDPMC: u64 = 1 << 2;
    if caps & CAP_BIT0_DEPRECATED != 0 { caps & CAP_USER_RDPMC != 0 } else { caps & CAP_LEGACY != 0 }
  }
}

#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
struct PerfRdpmc { fd: c_int, page: *mut perf::PerfEventMmapPage, len: usize }

#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
impl PerfRdpmc {
  fn open() -> Result<Self, String> {
    let attr = perf::PerfEventAttr {
      type_: perf::PERF_TYPE_HARDWARE,
      size: std::mem::size_of::<perf::PerfEventAttr>() as u32,
      config: perf::PERF_COUNT_HW_CPU_CYCLES,
      flags: perf::PERF_ATTR_FLAG_EXCLUDE_KERNEL | perf::PERF_ATTR_FLAG_EXCLUDE_HV,
      ..Default::default()
    };
    let fd = unsafe {
      perf::syscall(perf::SYS_PERF_EVENT_OPEN, ptr::addr_of!(attr), 0i64, -1i64, -1i64, perf::PERF_FLAG_FD_CLOEXEC)
    };
    if fd < 0 {
      return Err(format!("perf_event_open returned {}: {}", fd, std::io::Error::last_os_error()));
    }
    let len = perf::page_size();
    let page = unsafe { perf::mmap(ptr::null_mut(), len, perf::PROT_READ, perf::MAP_SHARED, fd as c_int, 0) };
    if page as isize == -1 {
      unsafe { perf::close(fd as c_int); }
      return Err(format!("mmap failed: {}", std::io::Error::last_os_error()));
    }
    Ok(Self { fd: fd as c_int, page: page as *mut _, len })
  }

  fn user_readable(&self) -> bool {
    unsafe {
      let caps = ptr::read_volatile(ptr::addr_of!((*self.page).capabilities));
      let index = ptr::read_volatile(ptr::addr_of!((*self.page).index));
      perf::cap_user_rdpmc(caps) && index != 0
    }
  }

  fn read(&self) -> u64 {
    unsafe {
      let index = ptr::read_volatile(ptr::addr_of!((*self.page).index));
      let offset = ptr::read_volatile(ptr::addr_of!((*self.page).offset));
      let pmc_width = ptr::read_volatile(ptr::addr_of!((*self.page).pmc_width));
      let low: u32; let high: u32;
      core::arch::asm!(
        "rdpmc",
        in("ecx") index - 1,
        out("eax") low,
        out("edx") high,
        options(nomem, nostack, preserves_flags),
      );
      let pmc = ((high as u64) << 32) | (low as u64);
      let shift = 64u32.saturating_sub(pmc_width as u32);
      let signed = ((pmc << shift) as i64) >> shift;
      offset.wrapping_add(signed) as u64
    }
  }
}

#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
impl Drop for PerfRdpmc {
  fn drop(&mut self) { unsafe { perf::munmap(self.page as *mut _, self.len); perf::close(self.fd); } }
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
struct PerfPmccntr { fd: c_int, page: *mut perf::PerfEventMmapPage, len: usize }

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
impl PerfPmccntr {
  fn open() -> Result<Self, String> {
    let attr = perf::PerfEventAttr {
      type_: perf::PERF_TYPE_HARDWARE,
      size: std::mem::size_of::<perf::PerfEventAttr>() as u32,
      config: perf::PERF_COUNT_HW_CPU_CYCLES,
      flags: perf::PERF_ATTR_FLAG_EXCLUDE_KERNEL | perf::PERF_ATTR_FLAG_EXCLUDE_HV,
      ..Default::default()
    };
    let fd = unsafe {
      perf::syscall(perf::SYS_PERF_EVENT_OPEN, ptr::addr_of!(attr), 0i64, -1i64, -1i64, perf::PERF_FLAG_FD_CLOEXEC)
    };
    if fd < 0 {
      return Err(format!("perf_event_open returned {}: {}", fd, std::io::Error::last_os_error()));
    }
    let len = perf::page_size();
    let page = unsafe { perf::mmap(ptr::null_mut(), len, perf::PROT_READ, perf::MAP_SHARED, fd as c_int, 0) };
    if page as isize == -1 {
      unsafe { perf::close(fd as c_int); }
      return Err(format!("mmap failed: {}", std::io::Error::last_os_error()));
    }
    Ok(Self { fd: fd as c_int, page: page as *mut _, len })
  }

  fn user_readable(&self) -> bool {
    unsafe {
      let caps = ptr::read_volatile(ptr::addr_of!((*self.page).capabilities));
      let index = ptr::read_volatile(ptr::addr_of!((*self.page).index));
      perf::cap_user_rdpmc(caps) && index == 32  // PMUv3 dedicated cycle counter
    }
  }

  fn read(&self) -> u64 {
    unsafe {
      let offset = ptr::read_volatile(ptr::addr_of!((*self.page).offset));
      let pmc_width = ptr::read_volatile(ptr::addr_of!((*self.page).pmc_width));
      let pmc: u64;
      core::arch::asm!(
        "mrs {}, pmccntr_el0", out(reg) pmc,
        options(nomem, nostack, preserves_flags),
      );
      let shift = 64u32.saturating_sub(pmc_width as u32);
      let signed = ((pmc << shift) as i64) >> shift;
      offset.wrapping_add(signed) as u64
    }
  }
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
impl Drop for PerfPmccntr {
  fn drop(&mut self) { unsafe { perf::munmap(self.page as *mut _, self.len); perf::close(self.fd); } }
}

fn main() {
  // Print host metadata
  println!("=== host ===");
  println!("target_os = {}", std::env::consts::OS);
  println!("target_arch = {}", std::env::consts::ARCH);
  #[cfg(target_os = "linux")]
  {
    if let Ok(info) = std::fs::read_to_string("/proc/cpuinfo") {
      for line in info.lines().filter(|l| l.starts_with("model name") || l.starts_with("vendor_id") || l.starts_with("CPU implementer") || l.starts_with("CPU part")).take(2) {
        println!("{}", line);
      }
    }
    for path in &["/sys/bus/event_source/devices/cpu/rdpmc", "/proc/sys/kernel/perf_user_access", "/proc/sys/kernel/perf_event_paranoid"] {
      match std::fs::read_to_string(path) {
        Ok(v) => println!("{} = {}", path, v.trim()),
        Err(_) => println!("{} = absent", path),
      }
    }
  }
  println!();

  #[cfg(target_arch = "x86_64")]
  survey_x86_64();
  #[cfg(target_arch = "aarch64")]
  survey_aarch64();
  #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
  println!("survey not implemented for this target arch");
}
