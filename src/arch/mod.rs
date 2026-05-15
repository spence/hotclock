use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(target_arch = "aarch64")]
pub mod aarch64;
pub mod fallback;
#[cfg(target_arch = "loongarch64")]
pub mod loongarch64;
#[cfg(target_arch = "riscv64")]
pub mod riscv64;
#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
pub mod wasm;
#[cfg(target_arch = "x86")]
pub mod x86;
#[cfg(target_arch = "x86_64")]
pub mod x86_64;

mod direct;
pub use direct::ticks;

// Cached at first elapsed() call. Stored as fixed-point Q32:
//   nanos_per_tick_q32 = (1e9 << 32) / frequency
// Then converting ticks to nanos becomes (ticks * scale) >> 32, replacing
// the per-call u128 division with a multiply + shift.
static NANOS_PER_TICK_Q32: AtomicU64 = AtomicU64::new(0);

const NANOS_PER_SECOND_Q32: u128 = 1_000_000_000u128 << 32;

#[inline]
#[must_use]
pub fn nanos_per_tick_q32() -> u64 {
  let cached = NANOS_PER_TICK_Q32.load(Ordering::Relaxed);
  if cached != 0 {
    return cached;
  }
  let freq = read_frequency();
  let scale = u64::try_from(NANOS_PER_SECOND_Q32 / u128::from(freq)).unwrap_or(u64::MAX);
  NANOS_PER_TICK_Q32.store(scale, Ordering::Relaxed);
  scale
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
  // SAFETY: `mach_timebase_info` populates the struct.
  unsafe { mach_timebase_info(&mut info) };
  1_000_000_000u64 * u64::from(info.denom) / u64::from(info.numer)
}

#[cfg(all(not(target_arch = "aarch64"), target_os = "windows"))]
#[inline]
fn read_frequency() -> u64 {
  unsafe extern "system" {
    fn QueryPerformanceFrequency(lpFrequency: *mut i64) -> i32;
  }
  let mut freq: i64 = 0;
  // SAFETY: `QueryPerformanceFrequency` writes a single i64.
  unsafe { QueryPerformanceFrequency(&mut freq) };
  freq as u64
}

#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
#[inline]
fn read_frequency() -> u64 {
  // ticks() returns nanos directly; identity Q32 transform.
  1_000_000_000
}

#[cfg(target_os = "wasi")]
#[inline]
fn read_frequency() -> u64 {
  // ticks() returns nanos from clock_time_get directly; identity transform.
  1_000_000_000
}

#[cfg(not(any(
  target_arch = "aarch64",
  target_os = "macos",
  target_os = "windows",
  target_os = "wasi",
  all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
)))]
#[inline]
fn read_frequency() -> u64 {
  crate::calibration::calibrate_frequency()
}
