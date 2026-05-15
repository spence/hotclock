// Direct OS-clock fallbacks for targets without an architectural counter.
// Each submodule is cfg-gated to its platform; `direct::ticks()` selects one
// based on target_os.

#[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
mod mach {
  unsafe extern "C" {
    fn mach_absolute_time() -> u64;
  }

  #[inline(always)]
  pub fn mach_time() -> u64 {
    // SAFETY: `mach_absolute_time` takes no arguments and returns the host
    // monotonic tick value with no Rust-side aliasing requirements.
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
    fn clock_gettime(clk_id: i32, tp: *mut Timespec) -> i32;
  }

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
pub use monotonic::*;

#[cfg(target_os = "wasi")]
mod wasi {
  #[link(wasm_import_module = "wasi_snapshot_preview1")]
  unsafe extern "C" {
    fn clock_time_get(id: u32, precision: u64, time: *mut u64) -> u16;
  }

  const CLOCK_MONOTONIC: u32 = 1;

  #[inline(always)]
  pub fn wasi_clock_monotonic() -> u64 {
    let mut t: u64 = 0;
    // SAFETY: writes a single u64 the host fills in. CLOCK_MONOTONIC and
    // precision=0 are always-valid inputs for wasi_snapshot_preview1.
    let _ = unsafe { clock_time_get(CLOCK_MONOTONIC, 0, &mut t) };
    t
  }
}

#[cfg(target_os = "wasi")]
pub use wasi::*;
