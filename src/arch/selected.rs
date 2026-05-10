use core::arch::asm;
use core::ptr::NonNull;
use std::hint::unreachable_unchecked;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use super::patch;

const UNSELECTED: u8 = u8::MAX;
const SELECTING: u8 = u8::MAX - 1;

#[cfg(target_arch = "x86_64")]
const GATE_LEN: usize = 9;
#[cfg(any(target_arch = "x86", target_arch = "s390x"))]
const GATE_LEN: usize = 8;
#[cfg(any(
  target_arch = "aarch64",
  target_arch = "riscv64",
  target_arch = "powerpc64",
  target_arch = "loongarch64",
))]
const GATE_LEN: usize = 4;

#[cfg(any(target_arch = "x86", target_arch = "s390x", all(target_arch = "x86_64", not(windows))))]
const COMMIT_LEN: usize = 8;

static SELECTED: AtomicU8 = AtomicU8::new(UNSELECTED);
static SELECTED_NAME: OnceLock<&'static str> = OnceLock::new();
static CYCLE_SELECTED: AtomicU8 = AtomicU8::new(UNSELECTED);
static CYCLE_SELECTED_NAME: OnceLock<&'static str> = OnceLock::new();

static INSTANT_PATCH_REGISTERED: AtomicUsize = AtomicUsize::new(0);
static INSTANT_PATCH_PATCHED: AtomicUsize = AtomicUsize::new(0);
static INSTANT_PATCH_ALREADY_PATCHED: AtomicUsize = AtomicUsize::new(0);
static INSTANT_PATCH_FAILED: AtomicUsize = AtomicUsize::new(0);
static CYCLE_PATCH_REGISTERED: AtomicUsize = AtomicUsize::new(0);
static CYCLE_PATCH_PATCHED: AtomicUsize = AtomicUsize::new(0);
static CYCLE_PATCH_ALREADY_PATCHED: AtomicUsize = AtomicUsize::new(0);
static CYCLE_PATCH_FAILED: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClockClass {
  Instant,
  Cycles,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CallsiteRecord {
  patch_address: usize,
  cold_address: usize,
  fallback_address: usize,
  hardware_address: usize,
}

#[cfg(not(any(target_os = "macos", windows)))]
#[used]
#[unsafe(link_section = "tach_instant_cs")]
static INSTANT_CALLSITE_SENTINEL: CallsiteRecord =
  CallsiteRecord { patch_address: 0, cold_address: 0, fallback_address: 0, hardware_address: 0 };

#[cfg(not(any(target_os = "macos", windows)))]
#[used]
#[unsafe(link_section = "tach_cycle_cs")]
static CYCLE_CALLSITE_SENTINEL: CallsiteRecord =
  CallsiteRecord { patch_address: 0, cold_address: 0, fallback_address: 0, hardware_address: 0 };

#[cfg(not(any(target_os = "macos", windows)))]
unsafe extern "C" {
  #[link_name = "__start_tach_instant_cs"]
  static INSTANT_CALLSITE_START: CallsiteRecord;

  #[link_name = "__stop_tach_instant_cs"]
  static INSTANT_CALLSITE_STOP: CallsiteRecord;

  #[link_name = "__start_tach_cycle_cs"]
  static CYCLE_CALLSITE_START: CallsiteRecord;

  #[link_name = "__stop_tach_cycle_cs"]
  static CYCLE_CALLSITE_STOP: CallsiteRecord;
}

#[cfg(not(any(target_os = "macos", windows)))]
macro_rules! instant_callsite_section_start {
  () => {
    ".pushsection tach_instant_cs,\"awR\",@progbits"
  };
}

#[cfg(not(any(target_os = "macos", windows)))]
macro_rules! cycle_callsite_section_start {
  () => {
    ".pushsection tach_cycle_cs,\"awR\",@progbits"
  };
}

#[cfg(not(any(target_os = "macos", windows)))]
#[allow(unused_macros)]
macro_rules! cold_section_start {
  () => {
    ".pushsection .text.tach_cold,\"ax\",@progbits"
  };
}

#[cfg(target_os = "macos")]
#[allow(unused_macros)]
macro_rules! cold_section_start {
  () => {
    ".pushsection __TEXT,__tach_cold,regular,pure_instructions"
  };
}

#[cfg(windows)]
#[allow(unused_macros)]
macro_rules! cold_section_start {
  () => {
    ".pushsection .text$tach_cold,\"xr\""
  };
}

#[cfg(target_pointer_width = "64")]
#[allow(unused_macros)]
macro_rules! callsite_record {
  () => {
    ".balign 8\n.quad 2f\n.quad 4f\n.quad 5f\n.quad 6f"
  };
}

#[cfg(target_pointer_width = "32")]
#[allow(unused_macros)]
macro_rules! callsite_record {
  () => {
    ".balign 4\n.long 2f\n.long 4f\n.long 5f\n.long 6f"
  };
}

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
    pub const PMCCNTR: u8 = 2;
    pub const PERF_PMCCNTR: u8 = 3;
  }

  #[cfg(all(target_arch = "aarch64", not(any(target_os = "linux", target_os = "macos"))))]
  pub use aarch64_other::*;
  #[cfg(all(target_arch = "aarch64", not(any(target_os = "linux", target_os = "macos"))))]
  mod aarch64_other {
    pub const CNTVCT: u8 = 0;
    #[cfg(unix)]
    pub const CLOCK_MONOTONIC: u8 = 1;
    #[cfg(not(unix))]
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
    #[cfg(target_os = "linux")]
    pub const DIRECT_RDPMC: u8 = 2;
    #[cfg(target_os = "linux")]
    pub const PERF_RDPMC: u8 = 3;
  }

  #[cfg(target_arch = "riscv64")]
  pub use riscv64::*;
  #[cfg(target_arch = "riscv64")]
  mod riscv64 {
    pub const RDTIME: u8 = 0;
    #[cfg(unix)]
    pub const CLOCK_MONOTONIC: u8 = 1;
    #[cfg(not(unix))]
    pub const STD_INSTANT: u8 = 1;
    pub const RDCYCLE: u8 = 2;
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
    #[cfg(unix)]
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    #[cfg(not(unix))]
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
    indices::RDTIME => super::riscv64::rdtime(),
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

#[inline(always)]
fn read_cycle_selected(sel: u8) -> u64 {
  #[cfg(target_arch = "x86_64")]
  return read_selected(sel);

  #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
  return match sel {
    indices::PMCCNTR => super::aarch64::pmccntr_el0(),
    indices::PERF_PMCCNTR => super::perf_pmccntr_linux::perf_pmccntr_cpu_cycles()
      .unwrap_or_else(super::aarch64::cntvct),
    indices::CNTVCT => super::aarch64::cntvct(),
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    _ => {
      // SAFETY: `sel` is installed only from `selection::cycle_candidates()` for this target.
      unsafe { unreachable_unchecked() }
    }
  };

  #[cfg(all(target_arch = "aarch64", not(any(target_os = "linux", target_os = "macos"))))]
  return read_selected(sel);

  #[cfg(target_arch = "x86")]
  return match sel {
    #[cfg(target_os = "linux")]
    indices::DIRECT_RDPMC => super::perf_rdpmc_linux::direct_rdpmc_fixed_core_cycles(),
    #[cfg(target_os = "linux")]
    indices::PERF_RDPMC => {
      super::perf_rdpmc_linux::perf_rdpmc_cpu_cycles().unwrap_or_else(super::x86::rdtsc)
    }
    indices::RDTSC => super::x86::rdtsc(),
    #[cfg(target_os = "macos")]
    indices::MACH_TIME => super::fallback::mach_time(),
    #[cfg(all(unix, not(target_os = "macos")))]
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => super::fallback::instant_elapsed(),
    _ => {
      // SAFETY: `sel` is installed only from `selection::cycle_candidates()` for this target.
      unsafe { unreachable_unchecked() }
    }
  };

  #[cfg(target_arch = "riscv64")]
  return match sel {
    indices::RDCYCLE => super::riscv64::rdcycle(),
    indices::RDTIME => super::riscv64::rdtime(),
    #[cfg(unix)]
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    #[cfg(not(unix))]
    indices::STD_INSTANT => super::fallback::instant_elapsed(),
    _ => {
      // SAFETY: `sel` is installed only from `selection::cycle_candidates()` for this target.
      unsafe { unreachable_unchecked() }
    }
  };

  #[cfg(target_arch = "powerpc64")]
  return read_selected(sel);

  #[cfg(target_arch = "s390x")]
  return read_selected(sel);

  #[cfg(target_arch = "loongarch64")]
  return read_selected(sel);
}

#[cfg(not(any(target_os = "macos", windows)))]
extern "C" fn instant_select_fallback() -> u64 {
  read_selected(ensure_selected())
}

#[cfg(not(any(target_os = "macos", windows)))]
extern "C" fn cycle_select_fallback() -> u64 {
  read_cycle_selected(ensure_cycle_selected())
}

#[cfg(any(target_os = "macos", windows))]
extern "C" fn instant_select_and_patch_current(
  patch_address: usize,
  cold_address: usize,
  fallback_address: usize,
  hardware_address: usize,
) -> u64 {
  select_and_patch_current(
    ClockClass::Instant,
    patch_address,
    cold_address,
    fallback_address,
    hardware_address,
  )
}

#[cfg(any(target_os = "macos", windows))]
extern "C" fn cycle_select_and_patch_current(
  patch_address: usize,
  cold_address: usize,
  fallback_address: usize,
  hardware_address: usize,
) -> u64 {
  select_and_patch_current(
    ClockClass::Cycles,
    patch_address,
    cold_address,
    fallback_address,
    hardware_address,
  )
}

#[cfg(any(target_os = "macos", windows))]
fn select_and_patch_current(
  class: ClockClass,
  patch_address: usize,
  cold_address: usize,
  fallback_address: usize,
  hardware_address: usize,
) -> u64 {
  let selected = ensure_class_selected(class);
  let record = CallsiteRecord { patch_address, cold_address, fallback_address, hardware_address };
  let mut stats = patch::PatchStats { registered: 1, ..patch::PatchStats::default() };

  match NonNull::new(patch_address as *mut u8)
    .map(|patch_address| patch_callsite(class, patch_address, &record, selected))
  {
    Some(PatchOutcome::Patched) => stats.patched = 1,
    Some(PatchOutcome::AlreadyPatched) => stats.already_patched = 1,
    Some(PatchOutcome::Failed) | None => stats.failed = 1,
  }

  store_patch_stats(class, stats);
  read_class_selected(class, selected)
}

extern "C" fn instant_direct_fallback() -> u64 {
  platform_fallback()
}

extern "C" fn cycle_direct_fallback() -> u64 {
  platform_fallback()
}

extern "C" fn instant_direct_hardware() -> u64 {
  #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
  {
    return read_selected(indices::RDTSC);
  }

  #[cfg(target_arch = "aarch64")]
  {
    return read_selected(indices::CNTVCT);
  }

  #[cfg(target_arch = "riscv64")]
  {
    return read_selected(indices::RDTIME);
  }

  #[cfg(target_arch = "powerpc64")]
  {
    return read_selected(indices::MFTB);
  }

  #[cfg(target_arch = "s390x")]
  {
    return read_selected(indices::STCKF);
  }

  #[cfg(target_arch = "loongarch64")]
  {
    read_selected(indices::RDTIME)
  }
}

extern "C" fn cycle_direct_hardware() -> u64 {
  read_cycle_selected(ensure_cycle_selected())
}

fn platform_fallback() -> u64 {
  #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
  {
    return super::fallback::mach_time();
  }
  #[cfg(all(target_arch = "x86_64", unix, not(target_os = "macos")))]
  {
    return super::fallback::clock_monotonic();
  }
  #[cfg(all(target_arch = "x86_64", not(unix)))]
  {
    return super::fallback::instant_elapsed();
  }

  #[cfg(all(target_arch = "x86", target_os = "macos"))]
  {
    return super::fallback::mach_time();
  }
  #[cfg(all(target_arch = "x86", unix, not(target_os = "macos")))]
  {
    return super::fallback::clock_monotonic();
  }
  #[cfg(all(target_arch = "x86", not(unix)))]
  {
    return super::fallback::instant_elapsed();
  }

  #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
  {
    return super::fallback::clock_monotonic();
  }
  #[cfg(all(target_arch = "aarch64", unix, not(any(target_os = "linux", target_os = "macos"))))]
  {
    return super::fallback::clock_monotonic();
  }
  #[cfg(all(
    target_arch = "aarch64",
    not(unix),
    not(any(target_os = "linux", target_os = "macos"))
  ))]
  {
    return super::fallback::instant_elapsed();
  }

  #[cfg(all(
    any(
      target_arch = "riscv64",
      target_arch = "powerpc64",
      target_arch = "s390x",
      target_arch = "loongarch64",
    ),
    unix,
  ))]
  {
    return super::fallback::clock_monotonic();
  }
  #[cfg(all(
    any(
      target_arch = "riscv64",
      target_arch = "powerpc64",
      target_arch = "s390x",
      target_arch = "loongarch64",
    ),
    not(unix),
  ))]
  {
    super::fallback::instant_elapsed()
  }
}

#[cfg(any(target_os = "macos", windows))]
fn ensure_class_selected(class: ClockClass) -> u8 {
  match class {
    ClockClass::Instant => ensure_selected(),
    ClockClass::Cycles => ensure_cycle_selected(),
  }
}

#[cfg(any(target_os = "macos", windows))]
fn read_class_selected(class: ClockClass, selected: u8) -> u64 {
  match class {
    ClockClass::Instant => read_selected(selected),
    ClockClass::Cycles => read_cycle_selected(selected),
  }
}

fn ensure_selected() -> u8 {
  loop {
    match SELECTED.load(Ordering::Acquire) {
      UNSELECTED => {
        if SELECTED
          .compare_exchange(UNSELECTED, SELECTING, Ordering::AcqRel, Ordering::Acquire)
          .is_ok()
        {
          let (idx, name) = crate::selection::select_best();
          let _ = SELECTED_NAME.set(name);
          #[cfg(not(any(target_os = "macos", windows)))]
          store_patch_stats(ClockClass::Instant, patch_callsites(ClockClass::Instant, idx));
          SELECTED.store(idx, Ordering::Release);
          return idx;
        }
      }
      SELECTING => std::hint::spin_loop(),
      selected => return selected,
    }
  }
}

fn ensure_cycle_selected() -> u8 {
  loop {
    match CYCLE_SELECTED.load(Ordering::Acquire) {
      UNSELECTED => {
        if CYCLE_SELECTED
          .compare_exchange(UNSELECTED, SELECTING, Ordering::AcqRel, Ordering::Acquire)
          .is_ok()
        {
          let (idx, name) = crate::selection::select_best_cycles();
          let _ = CYCLE_SELECTED_NAME.set(name);
          #[cfg(not(any(target_os = "macos", windows)))]
          store_patch_stats(ClockClass::Cycles, patch_callsites(ClockClass::Cycles, idx));
          CYCLE_SELECTED.store(idx, Ordering::Release);
          return idx;
        }
      }
      SELECTING => std::hint::spin_loop(),
      selected => return selected,
    }
  }
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn implementation() -> &'static str {
  ensure_selected();
  SELECTED_NAME.get().copied().unwrap_or("unknown")
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_implementation() -> &'static str {
  ensure_cycle_selected();
  CYCLE_SELECTED_NAME.get().copied().unwrap_or("unknown")
}

#[cfg(all(target_arch = "x86_64", not(any(target_os = "macos", windows))))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  let out: u64;

  // SAFETY: The assembly emits a patchable gate plus direct cold/fallback trampolines. The
  // clobber ABI tells LLVM not to keep caller-saved values live across the inline region.
  unsafe {
    asm!(
      instant_callsite_section_start!(),
      callsite_record!(),
      ".popsection",
      cold_section_start!(),
      ".p2align 4",
      "4:",
      "mov r11, rsp",
      "and rsp, -16",
      "sub rsp, 48",
      "mov qword ptr [rsp + 32], r11",
      "call {select}",
      "mov rsp, qword ptr [rsp + 32]",
      "jmp 3f",
      ".p2align 4",
      "5:",
      "mov r11, rsp",
      "and rsp, -16",
      "sub rsp, 48",
      "mov qword ptr [rsp + 32], r11",
      "call {fallback}",
      "mov rsp, qword ptr [rsp + 32]",
      "jmp 3f",
      ".p2align 4",
      "6:",
      "mov r11, rsp",
      "and rsp, -16",
      "sub rsp, 48",
      "mov qword ptr [rsp + 32], r11",
      "call {hardware}",
      "mov rsp, qword ptr [rsp + 32]",
      "jmp 3f",
      ".popsection",
      ".p2align 3",
      "2:",
      "jmp 4b",
      ".rept 4",
      "nop",
      ".endr",
      "3:",
      select = sym instant_select_fallback,
      fallback = sym instant_direct_fallback,
      hardware = sym instant_direct_hardware,
      lateout("rax") out,
      clobber_abi("C"),
    );
  }

  out
}

#[cfg(all(target_arch = "x86_64", not(any(target_os = "macos", windows))))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_ticks() -> u64 {
  let out: u64;

  // SAFETY: See `ticks`; this uses an independent callsite section and cycle selector.
  unsafe {
    asm!(
      cycle_callsite_section_start!(),
      callsite_record!(),
      ".popsection",
      cold_section_start!(),
      ".p2align 4",
      "4:",
      "mov r11, rsp",
      "and rsp, -16",
      "sub rsp, 48",
      "mov qword ptr [rsp + 32], r11",
      "call {select}",
      "mov rsp, qword ptr [rsp + 32]",
      "jmp 3f",
      ".p2align 4",
      "5:",
      "mov r11, rsp",
      "and rsp, -16",
      "sub rsp, 48",
      "mov qword ptr [rsp + 32], r11",
      "call {fallback}",
      "mov rsp, qword ptr [rsp + 32]",
      "jmp 3f",
      ".p2align 4",
      "6:",
      "mov r11, rsp",
      "and rsp, -16",
      "sub rsp, 48",
      "mov qword ptr [rsp + 32], r11",
      "call {hardware}",
      "mov rsp, qword ptr [rsp + 32]",
      "jmp 3f",
      ".popsection",
      ".p2align 3",
      "2:",
      "jmp 4b",
      ".rept 4",
      "nop",
      ".endr",
      "3:",
      select = sym cycle_select_fallback,
      fallback = sym cycle_direct_fallback,
      hardware = sym cycle_direct_hardware,
      lateout("rax") out,
      clobber_abi("C"),
    );
  }

  out
}

#[cfg(all(target_arch = "x86_64", any(target_os = "macos", windows)))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  x86_64_instant_self_patching_ticks()
}

#[cfg(all(target_arch = "x86_64", any(target_os = "macos", windows)))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_ticks() -> u64 {
  x86_64_cycle_self_patching_ticks()
}

#[cfg(all(target_arch = "x86_64", target_os = "macos"))]
macro_rules! x86_64_self_patching_ticks_fn {
  ($name:ident, $select:ident, $fallback:ident, $hardware:ident $(,)?) => {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    fn $name() -> u64 {
      let out: u64;

      // SAFETY: Mach-O keeps trampolines in the same text section to avoid unsupported
      // cross-section branch relocations. Patch failure falls back to selected dispatch.
      // Direct symbol calls avoid register-allocation conflicts with ABI argument setup.
      unsafe {
        asm!(
          ".p2align 3",
          "2:",
          ".byte 0xE9",
          ".long 4f - . - 4",
          ".rept 4",
          "nop",
          ".endr",
          "jmp 3f",
          ".p2align 4",
          "4:",
          "lea rdi, [rip + 2b]",
          "lea rsi, [rip + 4b]",
          "lea rdx, [rip + 5f]",
          "lea rcx, [rip + 6f]",
          "mov r11, rsp",
          "and rsp, -16",
          "sub rsp, 48",
          "mov qword ptr [rsp + 32], r11",
          "call {select}",
          "mov rsp, qword ptr [rsp + 32]",
          "jmp 3f",
          ".p2align 4",
          "5:",
          "mov r11, rsp",
          "and rsp, -16",
          "sub rsp, 48",
          "mov qword ptr [rsp + 32], r11",
          "call {fallback}",
          "mov rsp, qword ptr [rsp + 32]",
          "jmp 3f",
          ".p2align 4",
          "6:",
          "mov r11, rsp",
          "and rsp, -16",
          "sub rsp, 48",
          "mov qword ptr [rsp + 32], r11",
          "call {hardware}",
          "mov rsp, qword ptr [rsp + 32]",
          "jmp 3f",
          "3:",
          select = sym $select,
          fallback = sym $fallback,
          hardware = sym $hardware,
          lateout("rax") out,
          lateout("rcx") _,
          lateout("rdx") _,
          lateout("rdi") _,
          lateout("rsi") _,
          lateout("r8") _,
          lateout("r9") _,
          lateout("r10") _,
          lateout("r11") _,
          lateout("xmm0") _,
          lateout("xmm1") _,
          lateout("xmm2") _,
          lateout("xmm3") _,
          lateout("xmm4") _,
          lateout("xmm5") _,
          lateout("xmm6") _,
          lateout("xmm7") _,
          lateout("xmm8") _,
          lateout("xmm9") _,
          lateout("xmm10") _,
          lateout("xmm11") _,
          lateout("xmm12") _,
          lateout("xmm13") _,
          lateout("xmm14") _,
          lateout("xmm15") _,
        );
      }

      out
    }
  };
}

#[cfg(all(target_arch = "x86_64", windows))]
macro_rules! x86_64_self_patching_ticks_fn {
  ($name:ident, $select:ident, $fallback:ident, $hardware:ident $(,)?) => {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    fn $name() -> u64 {
      let out: u64;

      // SAFETY: The Win64 cold trampolines preserve every volatile register they use except
      // `rax`/`rdx`, which match the patched `RDTSC` gate's output/clobber shape. That keeps the
      // warmed path as raw counter bytes without forcing LLVM to compile around cold-call clobbers.
      unsafe {
        asm!(
          "2:",
          ".byte 0xE9",
          ".long 4f - . - 4",
          ".rept 4",
          "nop",
          ".endr",
          "3:",
          ".pushsection .text.tach_x86_64_cold,\"xr\"",
          ".p2align 4",
          "4:",
          "mov rax, rsp",
          "and rsp, -16",
          "sub rsp, 176",
          "mov qword ptr [rsp + 32], rax",
          "mov qword ptr [rsp + 40], rcx",
          "mov qword ptr [rsp + 48], r8",
          "mov qword ptr [rsp + 56], r9",
          "mov qword ptr [rsp + 64], r10",
          "mov qword ptr [rsp + 72], r11",
          "movdqu xmmword ptr [rsp + 80], xmm0",
          "movdqu xmmword ptr [rsp + 96], xmm1",
          "movdqu xmmword ptr [rsp + 112], xmm2",
          "movdqu xmmword ptr [rsp + 128], xmm3",
          "movdqu xmmword ptr [rsp + 144], xmm4",
          "movdqu xmmword ptr [rsp + 160], xmm5",
          "lea rcx, [rip + 2b]",
          "lea rdx, [rip + 4b]",
          "lea r8, [rip + 5f]",
          "lea r9, [rip + 6f]",
          "call {select}",
          "mov rcx, qword ptr [rsp + 40]",
          "mov r8, qword ptr [rsp + 48]",
          "mov r9, qword ptr [rsp + 56]",
          "mov r10, qword ptr [rsp + 64]",
          "mov r11, qword ptr [rsp + 72]",
          "movdqu xmm0, xmmword ptr [rsp + 80]",
          "movdqu xmm1, xmmword ptr [rsp + 96]",
          "movdqu xmm2, xmmword ptr [rsp + 112]",
          "movdqu xmm3, xmmword ptr [rsp + 128]",
          "movdqu xmm4, xmmword ptr [rsp + 144]",
          "movdqu xmm5, xmmword ptr [rsp + 160]",
          "mov rsp, qword ptr [rsp + 32]",
          "jmp 3b",
          ".p2align 4",
          "5:",
          "mov rax, rsp",
          "and rsp, -16",
          "sub rsp, 176",
          "mov qword ptr [rsp + 32], rax",
          "mov qword ptr [rsp + 40], rcx",
          "mov qword ptr [rsp + 48], r8",
          "mov qword ptr [rsp + 56], r9",
          "mov qword ptr [rsp + 64], r10",
          "mov qword ptr [rsp + 72], r11",
          "movdqu xmmword ptr [rsp + 80], xmm0",
          "movdqu xmmword ptr [rsp + 96], xmm1",
          "movdqu xmmword ptr [rsp + 112], xmm2",
          "movdqu xmmword ptr [rsp + 128], xmm3",
          "movdqu xmmword ptr [rsp + 144], xmm4",
          "movdqu xmmword ptr [rsp + 160], xmm5",
          "call {fallback}",
          "mov rcx, qword ptr [rsp + 40]",
          "mov r8, qword ptr [rsp + 48]",
          "mov r9, qword ptr [rsp + 56]",
          "mov r10, qword ptr [rsp + 64]",
          "mov r11, qword ptr [rsp + 72]",
          "movdqu xmm0, xmmword ptr [rsp + 80]",
          "movdqu xmm1, xmmword ptr [rsp + 96]",
          "movdqu xmm2, xmmword ptr [rsp + 112]",
          "movdqu xmm3, xmmword ptr [rsp + 128]",
          "movdqu xmm4, xmmword ptr [rsp + 144]",
          "movdqu xmm5, xmmword ptr [rsp + 160]",
          "mov rsp, qword ptr [rsp + 32]",
          "jmp 3b",
          ".p2align 4",
          "6:",
          "mov rax, rsp",
          "and rsp, -16",
          "sub rsp, 176",
          "mov qword ptr [rsp + 32], rax",
          "mov qword ptr [rsp + 40], rcx",
          "mov qword ptr [rsp + 48], r8",
          "mov qword ptr [rsp + 56], r9",
          "mov qword ptr [rsp + 64], r10",
          "mov qword ptr [rsp + 72], r11",
          "movdqu xmmword ptr [rsp + 80], xmm0",
          "movdqu xmmword ptr [rsp + 96], xmm1",
          "movdqu xmmword ptr [rsp + 112], xmm2",
          "movdqu xmmword ptr [rsp + 128], xmm3",
          "movdqu xmmword ptr [rsp + 144], xmm4",
          "movdqu xmmword ptr [rsp + 160], xmm5",
          "call {hardware}",
          "mov rcx, qword ptr [rsp + 40]",
          "mov r8, qword ptr [rsp + 48]",
          "mov r9, qword ptr [rsp + 56]",
          "mov r10, qword ptr [rsp + 64]",
          "mov r11, qword ptr [rsp + 72]",
          "movdqu xmm0, xmmword ptr [rsp + 80]",
          "movdqu xmm1, xmmword ptr [rsp + 96]",
          "movdqu xmm2, xmmword ptr [rsp + 112]",
          "movdqu xmm3, xmmword ptr [rsp + 128]",
          "movdqu xmm4, xmmword ptr [rsp + 144]",
          "movdqu xmm5, xmmword ptr [rsp + 160]",
          "mov rsp, qword ptr [rsp + 32]",
          "jmp 3b",
          ".popsection",
          select = sym $select,
          fallback = sym $fallback,
          hardware = sym $hardware,
          lateout("rax") out,
          lateout("rdx") _,
        );
      }

      out
    }
  };
}

#[cfg(all(target_arch = "x86_64", any(target_os = "macos", windows)))]
x86_64_self_patching_ticks_fn!(
  x86_64_instant_self_patching_ticks,
  instant_select_and_patch_current,
  instant_direct_fallback,
  instant_direct_hardware,
);

#[cfg(all(target_arch = "x86_64", any(target_os = "macos", windows)))]
x86_64_self_patching_ticks_fn!(
  x86_64_cycle_self_patching_ticks,
  cycle_select_and_patch_current,
  cycle_direct_fallback,
  cycle_direct_hardware,
);

#[cfg(all(target_arch = "x86", not(any(target_os = "macos", windows))))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  x86_ticks_elf(ClockClass::Instant)
}

#[cfg(all(target_arch = "x86", not(any(target_os = "macos", windows))))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_ticks() -> u64 {
  x86_ticks_elf(ClockClass::Cycles)
}

#[cfg(all(target_arch = "x86", not(any(target_os = "macos", windows))))]
#[inline(always)]
#[allow(clippy::inline_always)]
fn x86_ticks_elf(class: ClockClass) -> u64 {
  match class {
    ClockClass::Instant => x86_instant_ticks_elf(),
    ClockClass::Cycles => x86_cycle_ticks_elf(),
  }
}

#[cfg(all(target_arch = "x86", not(any(target_os = "macos", windows))))]
#[inline(always)]
#[allow(clippy::inline_always)]
fn x86_instant_ticks_elf() -> u64 {
  let low: u32;
  let high: u32;

  // SAFETY: The gate is patched atomically; i686 returns the u64 tick in EDX:EAX.
  unsafe {
    asm!(
      instant_callsite_section_start!(),
      callsite_record!(),
      ".popsection",
      cold_section_start!(),
      ".p2align 4",
      "4:",
      "call {select}",
      "jmp 3f",
      ".p2align 4",
      "5:",
      "call {fallback}",
      "jmp 3f",
      ".p2align 4",
      "6:",
      "call {hardware}",
      "jmp 3f",
      ".popsection",
      ".p2align 3",
      "2:",
      "jmp 4b",
      ".rept 3",
      "nop",
      ".endr",
      "3:",
      select = sym instant_select_fallback,
      fallback = sym instant_direct_fallback,
      hardware = sym instant_direct_hardware,
      lateout("eax") low,
      lateout("edx") high,
      clobber_abi("C"),
    );
  }

  (u64::from(high) << 32) | u64::from(low)
}

#[cfg(all(target_arch = "x86", not(any(target_os = "macos", windows))))]
#[inline(always)]
#[allow(clippy::inline_always)]
fn x86_cycle_ticks_elf() -> u64 {
  let low: u32;
  let high: u32;

  // SAFETY: See `x86_instant_ticks_elf`.
  unsafe {
    asm!(
      cycle_callsite_section_start!(),
      callsite_record!(),
      ".popsection",
      cold_section_start!(),
      ".p2align 4",
      "4:",
      "call {select}",
      "jmp 3f",
      ".p2align 4",
      "5:",
      "call {fallback}",
      "jmp 3f",
      ".p2align 4",
      "6:",
      "call {hardware}",
      "jmp 3f",
      ".popsection",
      ".p2align 3",
      "2:",
      "jmp 4b",
      ".rept 3",
      "nop",
      ".endr",
      "3:",
      select = sym cycle_select_fallback,
      fallback = sym cycle_direct_fallback,
      hardware = sym cycle_direct_hardware,
      lateout("eax") low,
      lateout("edx") high,
      clobber_abi("C"),
    );
  }

  (u64::from(high) << 32) | u64::from(low)
}

#[cfg(all(target_arch = "x86", any(target_os = "macos", windows)))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  x86_instant_self_patching_ticks()
}

#[cfg(all(target_arch = "x86", any(target_os = "macos", windows)))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_ticks() -> u64 {
  x86_cycle_self_patching_ticks()
}

#[cfg(all(target_arch = "x86", any(target_os = "macos", windows)))]
macro_rules! x86_self_patching_ticks_fn {
  ($name:ident, $select:ident, $fallback:ident, $hardware:ident $(,)?) => {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    fn $name() -> u64 {
      let low: u32;
      let high: u32;

      // SAFETY: COFF/Mach-O self-patches the current callsite and falls back on patch failure.
      // Direct symbol calls avoid register-allocation conflicts with the pushed ABI arguments.
      unsafe {
        asm!(
          ".p2align 3",
          "2:",
          ".byte 0xE9",
          ".long 4f - . - 4",
          ".rept 3",
          "nop",
          ".endr",
          "jmp 3f",
          ".p2align 4",
          "4:",
          "push 6f",
          "push 5f",
          "push 4b",
          "push 2b",
          "call {select}",
          "add esp, 16",
          "jmp 3f",
          ".p2align 4",
          "5:",
          "call {fallback}",
          "jmp 3f",
          ".p2align 4",
          "6:",
          "call {hardware}",
          "jmp 3f",
          "3:",
          select = sym $select,
          fallback = sym $fallback,
          hardware = sym $hardware,
          lateout("eax") low,
          lateout("edx") high,
          clobber_abi("C"),
        );
      }

      (u64::from(high) << 32) | u64::from(low)
    }
  };
}

#[cfg(all(target_arch = "x86", any(target_os = "macos", windows)))]
x86_self_patching_ticks_fn!(
  x86_instant_self_patching_ticks,
  instant_select_and_patch_current,
  instant_direct_fallback,
  instant_direct_hardware,
);

#[cfg(all(target_arch = "x86", any(target_os = "macos", windows)))]
x86_self_patching_ticks_fn!(
  x86_cycle_self_patching_ticks,
  cycle_select_and_patch_current,
  cycle_direct_fallback,
  cycle_direct_hardware,
);

#[cfg(all(target_arch = "aarch64", not(windows)))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  aarch64_instant_ticks_elf()
}

#[cfg(all(target_arch = "aarch64", not(windows)))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_ticks() -> u64 {
  aarch64_cycle_ticks_elf()
}

#[cfg(all(target_arch = "aarch64", not(windows)))]
macro_rules! aarch64_ticks_elf_fn {
  ($name:ident, $section:ident, $select:ident, $fallback:ident, $hardware:ident $(,)?) => {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    fn $name() -> u64 {
      let out: u64;

      // SAFETY: The gate is patched atomically from one branch instruction to either a direct
      // counter read or a direct fallback branch.
      unsafe {
        asm!(
          $section!(),
          callsite_record!(),
          ".popsection",
          cold_section_start!(),
          ".p2align 2",
          "4:",
          "bl {select}",
          "b 3f",
          ".p2align 2",
          "5:",
          "bl {fallback}",
          "b 3f",
          ".p2align 2",
          "6:",
          "bl {hardware}",
          "b 3f",
          ".popsection",
          ".p2align 2",
          "2:",
          "b 4b",
          "3:",
          select = sym $select,
          fallback = sym $fallback,
          hardware = sym $hardware,
          lateout("x0") out,
          clobber_abi("C"),
        );
      }

      out
    }
  };
}

#[cfg(all(target_arch = "aarch64", not(windows)))]
aarch64_ticks_elf_fn!(
  aarch64_instant_ticks_elf,
  instant_callsite_section_start,
  instant_select_fallback,
  instant_direct_fallback,
  instant_direct_hardware,
);

#[cfg(all(target_arch = "aarch64", not(windows)))]
aarch64_ticks_elf_fn!(
  aarch64_cycle_ticks_elf,
  cycle_callsite_section_start,
  cycle_select_fallback,
  cycle_direct_fallback,
  cycle_direct_hardware,
);

#[cfg(all(target_arch = "aarch64", windows))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  aarch64_windows_instant_ticks()
}

#[cfg(all(target_arch = "aarch64", windows))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_ticks() -> u64 {
  aarch64_windows_cycle_ticks()
}

#[cfg(all(target_arch = "aarch64", windows))]
macro_rules! aarch64_windows_ticks_fn {
  ($name:ident, $select:ident, $fallback:ident, $hardware:ident $(,)?) => {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    fn $name() -> u64 {
      let out: u64;

      // SAFETY: COFF keeps trampolines in the same text section. Direct symbol calls avoid
      // register-allocation conflicts with the x0-x3 ABI argument setup.
      unsafe {
        asm!(
          "2:",
          "b 4f",
          "b 3f",
          "4:",
          "adr x0, 2b",
          "adr x1, 4b",
          "adr x2, 5f",
          "adr x3, 6f",
          "bl {select}",
          "b 3f",
          "5:",
          "bl {fallback}",
          "b 3f",
          "6:",
          "bl {hardware}",
          "b 3f",
          "3:",
          select = sym $select,
          fallback = sym $fallback,
          hardware = sym $hardware,
          lateout("x0") out,
          clobber_abi("C"),
        );
      }

      out
    }
  };
}

#[cfg(all(target_arch = "aarch64", windows))]
aarch64_windows_ticks_fn!(
  aarch64_windows_instant_ticks,
  instant_select_and_patch_current,
  instant_direct_fallback,
  instant_direct_hardware,
);

#[cfg(all(target_arch = "aarch64", windows))]
aarch64_windows_ticks_fn!(
  aarch64_windows_cycle_ticks,
  cycle_select_and_patch_current,
  cycle_direct_fallback,
  cycle_direct_hardware,
);

#[cfg(target_arch = "riscv64")]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  riscv64_instant_ticks()
}

#[cfg(target_arch = "riscv64")]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_ticks() -> u64 {
  riscv64_cycle_ticks()
}

#[cfg(target_arch = "riscv64")]
macro_rules! riscv64_ticks_fn {
  ($name:ident, $section:ident, $select:ident, $fallback:ident, $hardware:ident $(,)?) => {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    fn $name() -> u64 {
      let out: u64;

      // SAFETY: The gate is patched atomically from one jump instruction to the selected path.
      unsafe {
        asm!(
          $section!(),
          callsite_record!(),
          ".popsection",
          cold_section_start!(),
          ".p2align 2",
          "4:",
          "call {select}",
          "j 3f",
          ".p2align 2",
          "5:",
          "call {fallback}",
          "j 3f",
          ".p2align 2",
          "6:",
          "call {hardware}",
          "j 3f",
          ".popsection",
          ".p2align 2",
          "2:",
          "j 4b",
          "3:",
          select = sym $select,
          fallback = sym $fallback,
          hardware = sym $hardware,
          lateout("a0") out,
          clobber_abi("C"),
        );
      }

      out
    }
  };
}

#[cfg(target_arch = "riscv64")]
riscv64_ticks_fn!(
  riscv64_instant_ticks,
  instant_callsite_section_start,
  instant_select_fallback,
  instant_direct_fallback,
  instant_direct_hardware,
);

#[cfg(target_arch = "riscv64")]
riscv64_ticks_fn!(
  riscv64_cycle_ticks,
  cycle_callsite_section_start,
  cycle_select_fallback,
  cycle_direct_fallback,
  cycle_direct_hardware,
);

#[cfg(target_arch = "powerpc64")]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  powerpc64_instant_ticks()
}

#[cfg(target_arch = "powerpc64")]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_ticks() -> u64 {
  powerpc64_cycle_ticks()
}

#[cfg(target_arch = "powerpc64")]
macro_rules! powerpc64_ticks_fn {
  ($name:ident, $section:ident, $select:ident, $fallback:ident, $hardware:ident $(,)?) => {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    fn $name() -> u64 {
      let out: u64;

      // SAFETY: The gate is patched atomically from one branch instruction to the selected path.
      unsafe {
        asm!(
          $section!(),
          callsite_record!(),
          ".popsection",
          cold_section_start!(),
          ".p2align 2",
          "4:",
          "bl {select}",
          "b 3f",
          ".p2align 2",
          "5:",
          "bl {fallback}",
          "b 3f",
          ".p2align 2",
          "6:",
          "bl {hardware}",
          "b 3f",
          ".popsection",
          ".p2align 2",
          "2:",
          "b 4b",
          "3:",
          select = sym $select,
          fallback = sym $fallback,
          hardware = sym $hardware,
          lateout("r3") out,
          clobber_abi("C"),
        );
      }

      out
    }
  };
}

#[cfg(target_arch = "powerpc64")]
powerpc64_ticks_fn!(
  powerpc64_instant_ticks,
  instant_callsite_section_start,
  instant_select_fallback,
  instant_direct_fallback,
  instant_direct_hardware,
);

#[cfg(target_arch = "powerpc64")]
powerpc64_ticks_fn!(
  powerpc64_cycle_ticks,
  cycle_callsite_section_start,
  cycle_select_fallback,
  cycle_direct_fallback,
  cycle_direct_hardware,
);

#[cfg(target_arch = "s390x")]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  s390x_instant_ticks()
}

#[cfg(target_arch = "s390x")]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_ticks() -> u64 {
  s390x_cycle_ticks()
}

#[cfg(target_arch = "s390x")]
macro_rules! s390x_ticks_fn {
  ($name:ident, $section:ident, $select:ident, $fallback:ident, $hardware:ident $(,)?) => {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    fn $name() -> u64 {
      let out: u64;

      // SAFETY: s390x uses direct trampolines because STCKF writes through memory.
      unsafe {
        asm!(
          $section!(),
          callsite_record!(),
          ".popsection",
          cold_section_start!(),
          ".p2align 3",
          "4:",
          "brasl %r14, {select}",
          "jg 3f",
          ".p2align 3",
          "5:",
          "brasl %r14, {fallback}",
          "jg 3f",
          ".p2align 3",
          "6:",
          "brasl %r14, {hardware}",
          "jg 3f",
          ".popsection",
          ".p2align 3",
          "2:",
          "jg 4b",
          "nopr",
          "3:",
          select = sym $select,
          fallback = sym $fallback,
          hardware = sym $hardware,
          lateout("r2") out,
          clobber_abi("C"),
        );
      }

      out
    }
  };
}

#[cfg(target_arch = "s390x")]
s390x_ticks_fn!(
  s390x_instant_ticks,
  instant_callsite_section_start,
  instant_select_fallback,
  instant_direct_fallback,
  instant_direct_hardware,
);

#[cfg(target_arch = "s390x")]
s390x_ticks_fn!(
  s390x_cycle_ticks,
  cycle_callsite_section_start,
  cycle_select_fallback,
  cycle_direct_fallback,
  cycle_direct_hardware,
);

#[cfg(target_arch = "loongarch64")]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  loongarch64_instant_ticks()
}

#[cfg(target_arch = "loongarch64")]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_ticks() -> u64 {
  loongarch64_cycle_ticks()
}

#[cfg(target_arch = "loongarch64")]
macro_rules! loongarch64_ticks_fn {
  ($name:ident, $section:ident, $select:ident, $fallback:ident, $hardware:ident $(,)?) => {
    #[inline(always)]
    #[allow(clippy::inline_always)]
    fn $name() -> u64 {
      let out: u64;

      // SAFETY: The gate is patched atomically from one branch instruction to the selected path.
      unsafe {
        asm!(
          $section!(),
          callsite_record!(),
          ".popsection",
          cold_section_start!(),
          ".p2align 2",
          "4:",
          "bl {select}",
          "b 3f",
          ".p2align 2",
          "5:",
          "bl {fallback}",
          "b 3f",
          ".p2align 2",
          "6:",
          "bl {hardware}",
          "b 3f",
          ".popsection",
          ".p2align 2",
          "2:",
          "b 4b",
          "3:",
          select = sym $select,
          fallback = sym $fallback,
          hardware = sym $hardware,
          lateout("$a0") out,
          clobber_abi("C"),
        );
      }

      out
    }
  };
}

#[cfg(target_arch = "loongarch64")]
loongarch64_ticks_fn!(
  loongarch64_instant_ticks,
  instant_callsite_section_start,
  instant_select_fallback,
  instant_direct_fallback,
  instant_direct_hardware,
);

#[cfg(target_arch = "loongarch64")]
loongarch64_ticks_fn!(
  loongarch64_cycle_ticks,
  cycle_callsite_section_start,
  cycle_select_fallback,
  cycle_direct_fallback,
  cycle_direct_hardware,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PatchOutcome {
  Patched,
  AlreadyPatched,
  Failed,
}

#[cfg(not(any(target_os = "macos", windows)))]
fn patch_callsites(class: ClockClass, selected: u8) -> patch::PatchStats {
  let mut stats = patch::PatchStats::default();

  for record in callsite_records(class) {
    let Some(patch_address) = NonNull::new(record.patch_address as *mut u8) else {
      continue;
    };
    stats.registered += 1;
    match patch_callsite(class, patch_address, record, selected) {
      PatchOutcome::Patched => stats.patched += 1,
      PatchOutcome::AlreadyPatched => stats.already_patched += 1,
      PatchOutcome::Failed => stats.failed += 1,
    }
  }

  stats
}

fn patch_callsite(
  class: ClockClass,
  patch_address: NonNull<u8>,
  record: &CallsiteRecord,
  selected: u8,
) -> PatchOutcome {
  let ptr = patch_address.as_ptr();
  let Some(selected_bytes) = selected_gate_bytes(class, record, selected) else {
    return PatchOutcome::Failed;
  };
  let Some(unselected_bytes) = branch_gate_bytes(record.patch_address, record.cold_address) else {
    return PatchOutcome::Failed;
  };

  let current = read_gate_bytes(ptr);
  if current == selected_bytes {
    return PatchOutcome::AlreadyPatched;
  }
  if current != unselected_bytes {
    return PatchOutcome::Failed;
  }

  #[cfg(any(
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "powerpc64",
    target_arch = "loongarch64",
  ))]
  let ok = patch::patch_u32(ptr, u32::from_ne_bytes(selected_bytes));

  #[cfg(all(target_arch = "x86_64", windows))]
  let ok = patch::patch_bytes_with_breakpoint_commit(ptr, &selected_bytes);

  #[cfg(any(
    target_arch = "x86",
    target_arch = "s390x",
    all(target_arch = "x86_64", not(windows)),
  ))]
  let ok = patch::patch_bytes_with_u64_commit(ptr, &selected_bytes, COMMIT_LEN);

  if ok { PatchOutcome::Patched } else { PatchOutcome::Failed }
}

fn read_gate_bytes(ptr: *mut u8) -> [u8; GATE_LEN] {
  let mut bytes = [0u8; GATE_LEN];
  // SAFETY: Callsite records point at crate-emitted gates with exactly `GATE_LEN` bytes.
  unsafe {
    core::ptr::copy_nonoverlapping(ptr, bytes.as_mut_ptr(), GATE_LEN);
  }
  bytes
}

fn selected_gate_bytes(
  class: ClockClass,
  record: &CallsiteRecord,
  selected: u8,
) -> Option<[u8; GATE_LEN]> {
  match class {
    ClockClass::Instant => instant_gate_bytes(record, selected),
    ClockClass::Cycles => cycle_gate_bytes(record, selected),
  }
}

fn instant_gate_bytes(record: &CallsiteRecord, selected: u8) -> Option<[u8; GATE_LEN]> {
  #[cfg(target_arch = "x86_64")]
  {
    return match selected {
      indices::RDTSC => Some([0x0F, 0x31, 0x48, 0xC1, 0xE2, 0x20, 0x48, 0x09, 0xD0]),
      _ => branch_gate_bytes(record.patch_address, record.fallback_address),
    };
  }

  #[cfg(target_arch = "x86")]
  {
    return match selected {
      indices::RDTSC => Some([0x0F, 0x31, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90]),
      _ => branch_gate_bytes(record.patch_address, record.fallback_address),
    };
  }

  #[cfg(target_arch = "aarch64")]
  {
    return match selected {
      indices::CNTVCT => Some([0x40, 0xE0, 0x3B, 0xD5]),
      _ => branch_gate_bytes(record.patch_address, record.fallback_address),
    };
  }

  #[cfg(target_arch = "riscv64")]
  {
    return match selected {
      indices::RDTIME => Some([0x73, 0x25, 0x10, 0xC0]),
      _ => branch_gate_bytes(record.patch_address, record.fallback_address),
    };
  }

  #[cfg(target_arch = "powerpc64")]
  {
    return match selected {
      indices::MFTB => Some([0x7C, 0x6C, 0x42, 0xE6]),
      _ => branch_gate_bytes(record.patch_address, record.fallback_address),
    };
  }

  #[cfg(target_arch = "s390x")]
  {
    return match selected {
      indices::STCKF => branch_gate_bytes(record.patch_address, record.hardware_address),
      _ => branch_gate_bytes(record.patch_address, record.fallback_address),
    };
  }

  #[cfg(target_arch = "loongarch64")]
  {
    match selected {
      indices::RDTIME => Some([0x04, 0x68, 0x00, 0x00]),
      _ => branch_gate_bytes(record.patch_address, record.fallback_address),
    }
  }
}

fn cycle_gate_bytes(record: &CallsiteRecord, selected: u8) -> Option<[u8; GATE_LEN]> {
  #[cfg(target_arch = "x86_64")]
  {
    return match selected {
      indices::RDTSC => Some([0x0F, 0x31, 0x48, 0xC1, 0xE2, 0x20, 0x48, 0x09, 0xD0]),
      _ => branch_gate_bytes(record.patch_address, record.fallback_address),
    };
  }

  #[cfg(target_arch = "x86")]
  {
    return match selected {
      indices::RDTSC => Some([0x0F, 0x31, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90]),
      #[cfg(target_os = "linux")]
      indices::DIRECT_RDPMC | indices::PERF_RDPMC => {
        branch_gate_bytes(record.patch_address, record.hardware_address)
      }
      _ => branch_gate_bytes(record.patch_address, record.fallback_address),
    };
  }

  #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
  {
    return match selected {
      indices::PMCCNTR => Some([0x00, 0x9D, 0x3B, 0xD5]),
      indices::CNTVCT => Some([0x40, 0xE0, 0x3B, 0xD5]),
      _ => branch_gate_bytes(record.patch_address, record.fallback_address),
    };
  }

  #[cfg(all(target_arch = "aarch64", not(target_os = "linux")))]
  {
    return instant_gate_bytes(record, selected);
  }

  #[cfg(target_arch = "riscv64")]
  {
    return match selected {
      indices::RDCYCLE => Some([0x73, 0x25, 0x00, 0xC0]),
      indices::RDTIME => Some([0x73, 0x25, 0x10, 0xC0]),
      _ => branch_gate_bytes(record.patch_address, record.fallback_address),
    };
  }

  #[cfg(any(target_arch = "powerpc64", target_arch = "s390x", target_arch = "loongarch64"))]
  {
    instant_gate_bytes(record, selected)
  }
}

fn branch_gate_bytes(from: usize, to: usize) -> Option<[u8; GATE_LEN]> {
  #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
  {
    let rel = patch::branch_i32(from, to, 5)?;
    let mut bytes = [0x90; GATE_LEN];
    bytes[0] = 0xE9;
    bytes[1..5].copy_from_slice(&rel.to_le_bytes());
    return Some(bytes);
  }

  #[cfg(target_arch = "aarch64")]
  {
    let imm = patch::branch_words(from, to, 2, 26)? as u32;
    let word = 0x1400_0000 | (imm & 0x03ff_ffff);
    return Some(word.to_ne_bytes());
  }

  #[cfg(target_arch = "riscv64")]
  {
    let offset = patch::branch_words(from, to, 1, 21)?;
    let imm = (offset << 1) as u32;
    let word = 0x0000_006F
      | ((imm & 0x0010_0000) << 11)
      | ((imm & 0x0000_07FE) << 20)
      | ((imm & 0x0000_0800) << 9)
      | (imm & 0x000F_F000);
    return Some(word.to_ne_bytes());
  }

  #[cfg(target_arch = "powerpc64")]
  {
    let offset = patch::branch_words(from, to, 2, 24)? << 2;
    let word = 0x4800_0000 | ((offset as u32) & 0x03ff_fffc);
    return Some(word.to_ne_bytes());
  }

  #[cfg(target_arch = "s390x")]
  {
    let offset = patch::branch_words(from, to, 1, 32)?;
    let mut bytes = [0u8; GATE_LEN];
    bytes[0] = 0xC0;
    bytes[1] = 0xF4;
    bytes[2..6].copy_from_slice(&offset.to_be_bytes());
    bytes[6] = 0x07;
    bytes[7] = 0x00;
    return Some(bytes);
  }

  #[cfg(target_arch = "loongarch64")]
  {
    let offset = patch::branch_words(from, to, 2, 26)?;
    let imm = offset as u32;
    let word = 0x5000_0000 | ((imm & 0xffff) << 10) | ((imm >> 16) & 0x03ff);
    Some(word.to_ne_bytes())
  }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn callsite_records(class: ClockClass) -> &'static [CallsiteRecord] {
  match class {
    ClockClass::Instant => {
      let start = core::ptr::addr_of!(INSTANT_CALLSITE_START) as usize;
      let stop = core::ptr::addr_of!(INSTANT_CALLSITE_STOP) as usize;
      records_from_bounds(start, stop)
    }
    ClockClass::Cycles => {
      let start = core::ptr::addr_of!(CYCLE_CALLSITE_START) as usize;
      let stop = core::ptr::addr_of!(CYCLE_CALLSITE_STOP) as usize;
      records_from_bounds(start, stop)
    }
  }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn records_from_bounds(start: usize, stop: usize) -> &'static [CallsiteRecord] {
  if stop < start {
    return &[];
  }

  let byte_len = stop - start;
  if byte_len % core::mem::size_of::<CallsiteRecord>() != 0 {
    return &[];
  }

  // SAFETY: The linker-provided bounds cover the contiguous metadata section populated by this
  // module's inline assembly plus the sentinel record.
  unsafe {
    core::slice::from_raw_parts(
      start as *const CallsiteRecord,
      byte_len / core::mem::size_of::<CallsiteRecord>(),
    )
  }
}

fn store_patch_stats(class: ClockClass, stats: patch::PatchStats) {
  let (registered, patched, already_patched, failed) = match class {
    ClockClass::Instant => (
      &INSTANT_PATCH_REGISTERED,
      &INSTANT_PATCH_PATCHED,
      &INSTANT_PATCH_ALREADY_PATCHED,
      &INSTANT_PATCH_FAILED,
    ),
    ClockClass::Cycles => (
      &CYCLE_PATCH_REGISTERED,
      &CYCLE_PATCH_PATCHED,
      &CYCLE_PATCH_ALREADY_PATCHED,
      &CYCLE_PATCH_FAILED,
    ),
  };

  registered.store(stats.registered, Ordering::Relaxed);
  patched.store(stats.patched, Ordering::Relaxed);
  already_patched.store(stats.already_patched, Ordering::Relaxed);
  failed.store(stats.failed, Ordering::Relaxed);
}

#[cfg(test)]
fn last_patch_stats(class: ClockClass) -> patch::PatchStats {
  let (registered, patched, already_patched, failed) = match class {
    ClockClass::Instant => (
      &INSTANT_PATCH_REGISTERED,
      &INSTANT_PATCH_PATCHED,
      &INSTANT_PATCH_ALREADY_PATCHED,
      &INSTANT_PATCH_FAILED,
    ),
    ClockClass::Cycles => (
      &CYCLE_PATCH_REGISTERED,
      &CYCLE_PATCH_PATCHED,
      &CYCLE_PATCH_ALREADY_PATCHED,
      &CYCLE_PATCH_FAILED,
    ),
  };

  patch::PatchStats {
    registered: registered.load(Ordering::Relaxed),
    patched: patched.load(Ordering::Relaxed),
    already_patched: already_patched.load(Ordering::Relaxed),
    failed: failed.load(Ordering::Relaxed),
  }
}

#[cfg(test)]
mod tests {
  use super::{ClockClass, GATE_LEN};

  #[test]
  fn selected_architecture_patches_registered_instant_callsites() {
    let _ = super::ticks();
    let stats = super::last_patch_stats(ClockClass::Instant);
    assert_eq!(stats.failed, 0, "{stats:?}");
    assert_eq!(stats.patched + stats.already_patched, stats.registered, "{stats:?}");
  }

  #[test]
  fn selected_architecture_patches_registered_cycle_callsites() {
    let _ = super::cycle_ticks();
    let stats = super::last_patch_stats(ClockClass::Cycles);
    assert_eq!(stats.failed, 0, "{stats:?}");
    assert_eq!(stats.patched + stats.already_patched, stats.registered, "{stats:?}");
  }

  #[test]
  fn fallback_branch_gate_has_expected_width() {
    let bytes = super::branch_gate_bytes(0x1000, 0x1040).expect("branch encodes");
    assert_eq!(bytes.len(), GATE_LEN);
  }
}
