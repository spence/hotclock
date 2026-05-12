use std::sync::OnceLock;

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
pub use direct::{implementation, ticks};

// Cycles patching infrastructure — only on the Linux targets where selection between
// PMU candidates and the wall-clock fallback earns its keep. On every other target,
// `Cycles::now()` compile-time-resolves to the Instant tick reader.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod patch;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod perf_rdpmc_linux;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod x86_64_linux;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use x86_64_linux::indices;

static FREQUENCY: OnceLock<u64> = OnceLock::new();
static CYCLE_FREQUENCY: OnceLock<u64> = OnceLock::new();

/// Cycles read entry-point.
///
/// On Linux x86 / x86_64, routes through the Cycles patchpoint module — first call
/// runs single-threaded selection between PMU candidates and the wall-clock fallback,
/// then patches every callsite to inline the winner's bytes. On every other target,
/// `Cycles::now()` reads the same wall-clock-rate counter `Instant::now()` reads.
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_ticks() -> u64 {
  #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
  return x86_64_linux::cycle_ticks();

  #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
  return ticks();
}

#[inline]
#[must_use]
pub fn cycle_implementation() -> &'static str {
  #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
  return x86_64_linux::cycle_implementation();

  #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
  return implementation();
}

#[inline]
#[must_use]
pub fn frequency() -> u64 {
  *FREQUENCY.get_or_init(crate::calibration::calibrate_frequency)
}

#[inline]
#[must_use]
pub fn cycle_frequency() -> u64 {
  *CYCLE_FREQUENCY.get_or_init(crate::calibration::calibrate_cycle_frequency)
}
