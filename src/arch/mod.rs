use std::sync::OnceLock;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;
pub mod fallback;
#[cfg(target_arch = "loongarch64")]
pub mod loongarch64;
#[cfg(target_arch = "powerpc64")]
pub mod powerpc64;
#[cfg(target_arch = "riscv64")]
pub mod riscv64;
#[cfg(target_arch = "s390x")]
pub mod s390x;
#[cfg(target_arch = "x86")]
pub mod x86;
#[cfg(target_arch = "x86_64")]
pub mod x86_64;

static FREQUENCY: OnceLock<u64> = OnceLock::new();

// These targets have one useful counter path: Apple Silicon uses CNTVCT_EL0, and
// unsupported architectures have only the OS fallback. Runtime selection would only add
// hot-path dispatch.
#[cfg(any(
  all(target_arch = "aarch64", target_os = "macos"),
  not(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "powerpc64",
    target_arch = "s390x",
    target_arch = "loongarch64",
  )),
))]
mod direct;

#[cfg(any(
  all(target_arch = "aarch64", target_os = "macos"),
  not(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "powerpc64",
    target_arch = "s390x",
    target_arch = "loongarch64",
  )),
))]
pub use direct::{implementation, ticks};

// Runtime-selected targets keep the fallback path because the fastest compiled counter can
// fail the monotonicity contract on some CPUs, kernels, or hypervisors.
#[cfg(not(any(
  all(target_arch = "aarch64", target_os = "macos"),
  not(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "powerpc64",
    target_arch = "s390x",
    target_arch = "loongarch64",
  )),
)))]
mod selected;

#[cfg(not(any(
  all(target_arch = "aarch64", target_os = "macos"),
  not(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "powerpc64",
    target_arch = "s390x",
    target_arch = "loongarch64",
  )),
)))]
pub use selected::{implementation, indices, ticks};

#[inline]
#[must_use]
pub fn frequency() -> u64 {
  *FREQUENCY.get_or_init(crate::calibration::calibrate_frequency)
}
