use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(target_arch = "aarch64")]
pub mod aarch64;
pub mod fallback;
#[cfg(target_arch = "loongarch64")]
pub mod loongarch64;
#[cfg(target_arch = "riscv64")]
pub mod riscv64;
#[cfg(target_arch = "x86")]
pub mod x86;
#[cfg(target_arch = "x86_64")]
pub mod x86_64;

mod direct;
pub use direct::ticks;

static FREQUENCY: AtomicU64 = AtomicU64::new(0);

#[inline]
#[must_use]
pub fn frequency() -> u64 {
  let cached = FREQUENCY.load(Ordering::Relaxed);
  if cached != 0 {
    return cached;
  }
  let f = read_frequency();
  FREQUENCY.store(f, Ordering::Relaxed);
  f
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn read_frequency() -> u64 {
  aarch64::cntfrq()
}

#[cfg(all(not(target_arch = "aarch64"), target_os = "macos"))]
#[inline]
fn read_frequency() -> u64 {
  // mach_timebase_info reports (numer, denom) such that
  //   nanoseconds = ticks * numer / denom
  // so the effective tick rate is 1e9 * denom / numer Hz.
  #[repr(C)]
  struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
  }
  unsafe extern "C" {
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
  }
  let mut info = MachTimebaseInfo { numer: 0, denom: 0 };
  // SAFETY: `mach_timebase_info` populates the struct and returns 0 on success.
  let rc = unsafe { mach_timebase_info(&mut info) };
  if rc != 0 || info.numer == 0 {
    return crate::calibration::calibrate_frequency();
  }
  1_000_000_000u64 * u64::from(info.denom) / u64::from(info.numer)
}

#[cfg(all(not(target_arch = "aarch64"), target_os = "windows"))]
#[inline]
fn read_frequency() -> u64 {
  unsafe extern "system" {
    fn QueryPerformanceFrequency(lpFrequency: *mut i64) -> i32;
  }
  let mut freq: i64 = 0;
  // SAFETY: `QueryPerformanceFrequency` writes a single i64 and returns nonzero on success.
  let ok = unsafe { QueryPerformanceFrequency(&mut freq) };
  if ok == 0 || freq <= 0 {
    return crate::calibration::calibrate_frequency();
  }
  freq as u64
}

#[cfg(not(any(target_arch = "aarch64", target_os = "macos", target_os = "windows")))]
#[inline]
fn read_frequency() -> u64 {
  crate::calibration::calibrate_frequency()
}
