// These fallbacks are reachable from `direct.rs` only on targets without a canonical
// architectural counter (powerpc64, s390x, etc.). On supported architectures the
// canonical counter is used directly and these symbols are dead — hence `allow(dead_code)`.

#[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
mod mach {
  unsafe extern "C" {
    #[allow(dead_code)]
    fn mach_absolute_time() -> u64;
  }

  #[allow(dead_code)]
  #[inline(always)]
  pub fn mach_time() -> u64 {
    // SAFETY: `mach_absolute_time` takes no arguments, has no Rust-side aliasing
    // requirements, and returns the host monotonic tick value.
    unsafe { mach_absolute_time() }
  }
}

#[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
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
    #[allow(dead_code)]
    fn clock_gettime(clk_id: i32, tp: *mut Timespec) -> i32;
  }

  #[allow(dead_code)]
  #[inline(always)]
  pub fn clock_monotonic() -> u64 {
    let mut ts = Timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `ts` is a valid, writable `timespec` pointer for the duration of the call.
    let rc = unsafe { clock_gettime(CLOCK_MONOTONIC, &mut ts) };
    debug_assert_eq!(rc, 0);
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
  }
}

#[cfg(all(unix, not(target_os = "macos")))]
#[allow(unused_imports)]
pub use monotonic::*;

#[cfg(not(unix))]
mod instant {
  use std::sync::OnceLock;
  use std::time::Instant;

  static START: OnceLock<Instant> = OnceLock::new();

  #[allow(dead_code)]
  #[inline(always)]
  pub fn instant_elapsed() -> u64 {
    START.get_or_init(Instant::now).elapsed().as_nanos() as u64
  }
}

#[cfg(not(unix))]
#[allow(unused_imports)]
pub use instant::*;
