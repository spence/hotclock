use std::hint::unreachable_unchecked;
use std::sync::OnceLock;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;
#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
pub mod aarch64_linux;
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

static mut SELECTED: u8 = 0;
static mut SELECTED_NAME: &str = "";
static FREQUENCY: OnceLock<u64> = OnceLock::new();

pub mod indices {
  #[cfg(target_arch = "x86_64")]
  pub use x86_64::*;
  #[cfg(target_arch = "x86_64")]
  mod x86_64 {
    pub const RDTSC: u8 = 0;
    #[cfg(target_os = "macos")]
    pub const MACH_TIME: u8 = 1;
    #[cfg(all(unix, not(target_os = "macos")))]
    pub const CLOCK_MONOTONIC: u8 = 1;
    #[cfg(not(unix))]
    pub const STD_INSTANT: u8 = 1;
  }

  #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
  pub use aarch64_linux::*;
  #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
  mod aarch64_linux {
    pub const PMCCNTR: u8 = 0;
    pub const CNTVCT: u8 = 1;
    pub const CLOCK_MONOTONIC: u8 = 2;
  }

  #[cfg(all(target_arch = "aarch64", not(target_os = "linux")))]
  pub use aarch64_other::*;
  #[cfg(all(target_arch = "aarch64", not(target_os = "linux")))]
  mod aarch64_other {
    pub const CNTVCT: u8 = 0;
    #[cfg(target_os = "macos")]
    pub const MACH_TIME: u8 = 1;
    #[cfg(not(target_os = "macos"))]
    pub const STD_INSTANT: u8 = 1;
  }

  #[cfg(target_arch = "x86")]
  pub use x86::*;
  #[cfg(target_arch = "x86")]
  mod x86 {
    pub const RDTSC: u8 = 0;
    #[cfg(target_os = "macos")]
    pub const MACH_TIME: u8 = 1;
    #[cfg(all(unix, not(target_os = "macos")))]
    pub const CLOCK_MONOTONIC: u8 = 1;
    #[cfg(not(unix))]
    pub const STD_INSTANT: u8 = 1;
  }

  #[cfg(target_arch = "riscv64")]
  pub use riscv64::*;
  #[cfg(target_arch = "riscv64")]
  mod riscv64 {
    pub const RDCYCLE: u8 = 0;
    #[cfg(unix)]
    pub const CLOCK_MONOTONIC: u8 = 1;
    #[cfg(not(unix))]
    pub const STD_INSTANT: u8 = 1;
  }

  #[cfg(target_arch = "powerpc64")]
  pub use powerpc64::*;
  #[cfg(target_arch = "powerpc64")]
  mod powerpc64 {
    pub const MFTB: u8 = 0;
    #[cfg(unix)]
    pub const CLOCK_MONOTONIC: u8 = 1;
    #[cfg(not(unix))]
    pub const STD_INSTANT: u8 = 1;
  }

  #[cfg(target_arch = "s390x")]
  pub use s390x::*;
  #[cfg(target_arch = "s390x")]
  mod s390x {
    pub const STCKF: u8 = 0;
    #[cfg(unix)]
    pub const CLOCK_MONOTONIC: u8 = 1;
    #[cfg(not(unix))]
    pub const STD_INSTANT: u8 = 1;
  }

  #[cfg(target_arch = "loongarch64")]
  pub use loongarch64::*;
  #[cfg(target_arch = "loongarch64")]
  mod loongarch64 {
    pub const RDTIME: u8 = 0;
    #[cfg(unix)]
    pub const CLOCK_MONOTONIC: u8 = 1;
    #[cfg(not(unix))]
    pub const STD_INSTANT: u8 = 1;
  }

  #[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "powerpc64",
    target_arch = "s390x",
    target_arch = "loongarch64",
  )))]
  pub use fallback_only::*;
  #[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "powerpc64",
    target_arch = "s390x",
    target_arch = "loongarch64",
  )))]
  mod fallback_only {
    #[cfg(target_os = "macos")]
    pub const MACH_TIME: u8 = 0;
    #[cfg(all(unix, not(target_os = "macos")))]
    pub const CLOCK_MONOTONIC: u8 = 0;
    #[cfg(not(unix))]
    pub const STD_INSTANT: u8 = 0;
  }
}

#[inline(always)]
fn read_selected(sel: u8) -> u64 {
  #[cfg(target_arch = "x86_64")]
  return match sel {
    indices::RDTSC => x86_64::rdtsc(),
    #[cfg(target_os = "macos")]
    indices::MACH_TIME => fallback::mach_time(),
    #[cfg(all(unix, not(target_os = "macos")))]
    indices::CLOCK_MONOTONIC => fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => fallback::instant_elapsed(),
    // SAFETY: selection::init() only sets valid indices
    _ => unsafe { unreachable_unchecked() },
  };

  #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
  return match sel {
    indices::PMCCNTR => aarch64_linux::pmccntr(),
    indices::CNTVCT => aarch64::cntvct(),
    indices::CLOCK_MONOTONIC => fallback::clock_monotonic(),
    _ => unsafe { unreachable_unchecked() },
  };

  #[cfg(all(target_arch = "aarch64", not(target_os = "linux")))]
  return match sel {
    indices::CNTVCT => aarch64::cntvct(),
    #[cfg(target_os = "macos")]
    indices::MACH_TIME => fallback::mach_time(),
    #[cfg(not(target_os = "macos"))]
    indices::STD_INSTANT => fallback::instant_elapsed(),
    _ => unsafe { unreachable_unchecked() },
  };

  #[cfg(target_arch = "x86")]
  return match sel {
    indices::RDTSC => x86::rdtsc(),
    #[cfg(target_os = "macos")]
    indices::MACH_TIME => fallback::mach_time(),
    #[cfg(all(unix, not(target_os = "macos")))]
    indices::CLOCK_MONOTONIC => fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => fallback::instant_elapsed(),
    _ => unsafe { unreachable_unchecked() },
  };

  #[cfg(target_arch = "riscv64")]
  return match sel {
    indices::RDCYCLE => riscv64::rdcycle(),
    #[cfg(unix)]
    indices::CLOCK_MONOTONIC => fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => fallback::instant_elapsed(),
    _ => unsafe { unreachable_unchecked() },
  };

  #[cfg(target_arch = "powerpc64")]
  return match sel {
    indices::MFTB => powerpc64::mftb(),
    #[cfg(unix)]
    indices::CLOCK_MONOTONIC => fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => fallback::instant_elapsed(),
    _ => unsafe { unreachable_unchecked() },
  };

  #[cfg(target_arch = "s390x")]
  return match sel {
    indices::STCKF => s390x::stckf(),
    #[cfg(unix)]
    indices::CLOCK_MONOTONIC => fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => fallback::instant_elapsed(),
    _ => unsafe { unreachable_unchecked() },
  };

  #[cfg(target_arch = "loongarch64")]
  return match sel {
    indices::RDTIME => loongarch64::rdtime(),
    #[cfg(unix)]
    indices::CLOCK_MONOTONIC => fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => fallback::instant_elapsed(),
    _ => unsafe { unreachable_unchecked() },
  };

  #[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "powerpc64",
    target_arch = "s390x",
    target_arch = "loongarch64",
  )))]
  return match sel {
    #[cfg(target_os = "macos")]
    indices::MACH_TIME => fallback::mach_time(),
    #[cfg(all(unix, not(target_os = "macos")))]
    indices::CLOCK_MONOTONIC => fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => fallback::instant_elapsed(),
    _ => unsafe { unreachable_unchecked() },
  };
}

pub fn set_selected(idx: u8, name: &'static str) {
  // SAFETY: Called once from static init before any other access.
  unsafe {
    SELECTED = idx;
    SELECTED_NAME = name;
  }
}

#[inline(always)]
pub fn ticks() -> u64 {
  // SAFETY: Static init guarantees SELECTED is set before first call.
  read_selected(unsafe { SELECTED })
}

#[inline(always)]
pub fn frequency() -> u64 {
  *FREQUENCY.get_or_init(crate::selection::calibrate_frequency)
}

#[inline(always)]
pub fn implementation() -> &'static str {
  // SAFETY: Static init guarantees SELECTED_NAME is set before first call.
  unsafe { SELECTED_NAME }
}
