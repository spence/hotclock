use std::hint::unreachable_unchecked;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

const UNSELECTED: u8 = u8::MAX;

static SELECTED: AtomicU8 = AtomicU8::new(UNSELECTED);
static SELECTED_INIT: OnceLock<()> = OnceLock::new();
static SELECTED_NAME: OnceLock<&'static str> = OnceLock::new();

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
    pub const CNTVCT: u8 = 0;
    pub const CLOCK_MONOTONIC: u8 = 1;
  }

  #[cfg(all(target_arch = "aarch64", not(any(target_os = "linux", target_os = "macos"))))]
  pub use aarch64_other::*;
  #[cfg(all(target_arch = "aarch64", not(any(target_os = "linux", target_os = "macos"))))]
  mod aarch64_other {
    pub const CNTVCT: u8 = 0;
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
}

#[inline(always)]
fn read_selected(sel: u8) -> u64 {
  #[cfg(target_arch = "x86_64")]
  return match sel {
    indices::RDTSC => super::x86_64::rdtsc(),
    #[cfg(target_os = "macos")]
    indices::MACH_TIME => super::fallback::mach_time(),
    #[cfg(all(unix, not(target_os = "macos")))]
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => super::fallback::instant_elapsed(),
    _ => {
      // SAFETY: `sel` is installed only from `selection::candidates()` for this target.
      unsafe { unreachable_unchecked() }
    }
  };

  #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
  return match sel {
    indices::CNTVCT => super::aarch64::cntvct(),
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    _ => {
      // SAFETY: `sel` is installed only from `selection::candidates()` for this target.
      unsafe { unreachable_unchecked() }
    }
  };

  #[cfg(all(target_arch = "aarch64", not(any(target_os = "linux", target_os = "macos"))))]
  return match sel {
    indices::CNTVCT => super::aarch64::cntvct(),
    indices::STD_INSTANT => super::fallback::instant_elapsed(),
    _ => {
      // SAFETY: `sel` is installed only from `selection::candidates()` for this target.
      unsafe { unreachable_unchecked() }
    }
  };

  #[cfg(target_arch = "x86")]
  return match sel {
    indices::RDTSC => super::x86::rdtsc(),
    #[cfg(target_os = "macos")]
    indices::MACH_TIME => super::fallback::mach_time(),
    #[cfg(all(unix, not(target_os = "macos")))]
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => super::fallback::instant_elapsed(),
    _ => {
      // SAFETY: `sel` is installed only from `selection::candidates()` for this target.
      unsafe { unreachable_unchecked() }
    }
  };

  #[cfg(target_arch = "riscv64")]
  return match sel {
    indices::RDCYCLE => super::riscv64::rdcycle(),
    #[cfg(unix)]
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => super::fallback::instant_elapsed(),
    _ => {
      // SAFETY: `sel` is installed only from `selection::candidates()` for this target.
      unsafe { unreachable_unchecked() }
    }
  };

  #[cfg(target_arch = "powerpc64")]
  return match sel {
    indices::MFTB => super::powerpc64::mftb(),
    #[cfg(unix)]
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => super::fallback::instant_elapsed(),
    _ => {
      // SAFETY: `sel` is installed only from `selection::candidates()` for this target.
      unsafe { unreachable_unchecked() }
    }
  };

  #[cfg(target_arch = "s390x")]
  return match sel {
    indices::STCKF => super::s390x::stckf(),
    #[cfg(unix)]
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => super::fallback::instant_elapsed(),
    _ => {
      // SAFETY: `sel` is installed only from `selection::candidates()` for this target.
      unsafe { unreachable_unchecked() }
    }
  };

  #[cfg(target_arch = "loongarch64")]
  return match sel {
    indices::RDTIME => super::loongarch64::rdtime(),
    #[cfg(unix)]
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => super::fallback::instant_elapsed(),
    _ => {
      // SAFETY: `sel` is installed only from `selection::candidates()` for this target.
      unsafe { unreachable_unchecked() }
    }
  };
}

fn install_selected(idx: u8, name: &'static str) {
  let _ = SELECTED_NAME.set(name);
  SELECTED.store(idx, Ordering::Release);
}

#[inline]
fn ensure_selected() -> u8 {
  let selected = SELECTED.load(Ordering::Acquire);
  if selected != UNSELECTED {
    return selected;
  }

  SELECTED_INIT.get_or_init(|| {
    let (idx, name) = crate::selection::select_best();
    install_selected(idx, name);
  });

  let selected = SELECTED.load(Ordering::Acquire);
  debug_assert_ne!(selected, UNSELECTED);
  selected
}

#[inline(always)]
pub fn ticks() -> u64 {
  read_selected(ensure_selected())
}

#[inline(always)]
pub fn implementation() -> &'static str {
  ensure_selected();
  SELECTED_NAME.get().copied().unwrap_or("unknown")
}
