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
pub use direct::ticks;

static FREQUENCY: OnceLock<u64> = OnceLock::new();

#[inline]
#[must_use]
pub fn frequency() -> u64 {
  *FREQUENCY.get_or_init(crate::calibration::calibrate_frequency)
}
