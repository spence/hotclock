#[cfg(target_os = "macos")]
mod mach {
  unsafe extern "C" {
    fn mach_absolute_time() -> u64;
  }

  #[inline(always)]
  pub fn mach_time() -> u64 {
    unsafe { mach_absolute_time() }
  }
}

#[cfg(target_os = "macos")]
pub use mach::*;

#[cfg(all(unix, not(target_os = "macos")))]
mod monotonic {
  #[repr(C)]
  struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
  }

  const CLOCK_MONOTONIC: i32 = 1;

  unsafe extern "C" {
    fn clock_gettime(clk_id: i32, tp: *mut Timespec) -> i32;
  }

  #[inline(always)]
  pub fn clock_monotonic() -> u64 {
    let mut ts = Timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe {
      clock_gettime(CLOCK_MONOTONIC, &mut ts);
    }
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
  }
}

#[cfg(all(unix, not(target_os = "macos")))]
pub use monotonic::*;

#[cfg(not(unix))]
mod instant {
  use std::sync::OnceLock;
  use std::time::Instant;

  static START: OnceLock<Instant> = OnceLock::new();

  #[inline(always)]
  pub fn instant_elapsed() -> u64 {
    START.get_or_init(Instant::now).elapsed().as_nanos() as u64
  }
}

#[cfg(not(unix))]
pub use instant::*;
