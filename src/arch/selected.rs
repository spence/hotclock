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

#[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "s390x"))]
const COMMIT_LEN: usize = 8;
static SELECTED: AtomicU8 = AtomicU8::new(UNSELECTED);
static SELECTED_NAME: OnceLock<&'static str> = OnceLock::new();
static LAST_PATCH_REGISTERED: AtomicUsize = AtomicUsize::new(0);
static LAST_PATCH_PATCHED: AtomicUsize = AtomicUsize::new(0);
static LAST_PATCH_ALREADY_PATCHED: AtomicUsize = AtomicUsize::new(0);
static LAST_PATCH_FAILED: AtomicUsize = AtomicUsize::new(0);

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
#[unsafe(link_section = "hotclock_cs")]
static CALLSITE_SENTINEL: CallsiteRecord =
  CallsiteRecord { patch_address: 0, cold_address: 0, fallback_address: 0, hardware_address: 0 };

#[cfg(not(any(target_os = "macos", windows)))]
unsafe extern "C" {
  #[link_name = "__start_hotclock_cs"]
  static CALLSITE_START: CallsiteRecord;

  #[link_name = "__stop_hotclock_cs"]
  static CALLSITE_STOP: CallsiteRecord;
}

#[cfg(not(any(target_os = "macos", windows)))]
macro_rules! callsite_section_start {
  () => {
    ".pushsection hotclock_cs,\"awR\",@progbits"
  };
}

#[cfg(not(any(target_os = "macos", windows)))]
#[allow(unused_macros)]
macro_rules! cold_section_start {
  () => {
    ".pushsection .text.hotclock_cold,\"ax\",@progbits"
  };
}

#[cfg(target_os = "macos")]
#[allow(unused_macros)]
macro_rules! cold_section_start {
  () => {
    ".pushsection __TEXT,__hotclock_cold,regular,pure_instructions"
  };
}

#[cfg(windows)]
#[allow(unused_macros)]
macro_rules! cold_section_start {
  () => {
    ".pushsection .text$hotclock_cold,\"xr\""
  };
}

#[cfg(target_pointer_width = "64")]
#[allow(unused_macros)]
macro_rules! callsite_record_without_hardware_trampoline {
  () => {
    ".balign 8\n.quad 2f\n.quad 4f\n.quad 5f\n.quad 0"
  };
}

#[cfg(target_pointer_width = "32")]
#[allow(unused_macros)]
macro_rules! callsite_record_without_hardware_trampoline {
  () => {
    ".balign 4\n.long 2f\n.long 4f\n.long 5f\n.long 0"
  };
}

#[cfg(target_pointer_width = "64")]
#[allow(unused_macros)]
macro_rules! callsite_record_with_hardware_trampoline {
  () => {
    ".balign 8\n.quad 2f\n.quad 4f\n.quad 5f\n.quad 6f"
  };
}

#[cfg(target_pointer_width = "32")]
#[allow(unused_macros)]
macro_rules! callsite_record_with_hardware_trampoline {
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

#[cfg(not(any(target_os = "macos", windows)))]
extern "C" fn select_fallback() -> u64 {
  read_selected(ensure_selected())
}

#[cfg(any(target_os = "macos", windows))]
extern "C" fn select_and_patch_current(
  patch_address: usize,
  cold_address: usize,
  fallback_address: usize,
  hardware_address: usize,
) -> u64 {
  let selected = ensure_selected();
  let record = CallsiteRecord { patch_address, cold_address, fallback_address, hardware_address };
  let Some(patch_address) = NonNull::new(patch_address as *mut u8) else {
    std::process::abort();
  };

  let mut stats = patch::PatchStats { registered: 1, ..patch::PatchStats::default() };
  match patch_callsite(patch_address, &record, selected) {
    PatchOutcome::Patched => stats.patched = 1,
    PatchOutcome::AlreadyPatched => stats.already_patched = 1,
    PatchOutcome::Failed => {
      stats.failed = 1;
      LAST_PATCH_REGISTERED.store(stats.registered, Ordering::Relaxed);
      LAST_PATCH_FAILED.store(stats.failed, Ordering::Relaxed);
      std::process::abort();
    }
  }

  LAST_PATCH_REGISTERED.store(stats.registered, Ordering::Relaxed);
  LAST_PATCH_PATCHED.store(stats.patched, Ordering::Relaxed);
  LAST_PATCH_ALREADY_PATCHED.store(stats.already_patched, Ordering::Relaxed);
  LAST_PATCH_FAILED.store(stats.failed, Ordering::Relaxed);

  read_selected(selected)
}

extern "C" fn direct_fallback() -> u64 {
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

  #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
  {
    return super::fallback::clock_monotonic();
  }
  #[cfg(all(target_arch = "aarch64", not(any(target_os = "linux", target_os = "macos"))))]
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

#[cfg(target_arch = "s390x")]
extern "C" fn direct_hardware() -> u64 {
  super::s390x::stckf()
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
          {
            let stats = patch_callsites(idx);
            if stats.failed != 0 || stats.registered == 0 {
              panic!("hotclock: failed to patch selected callsites: {stats:?}");
            }
          }
          SELECTED.store(idx, Ordering::Release);
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

#[cfg(all(target_arch = "x86_64", not(any(target_os = "macos", windows))))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  let out: u64;

  // SAFETY: The assembly emits a patchable gate plus direct cold/fallback trampolines. The
  // clobber ABI tells LLVM not to keep caller-saved values live across the inline region.
  unsafe {
    asm!(
      callsite_section_start!(),
      callsite_record_without_hardware_trampoline!(),
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
      ".popsection",
      ".p2align 3",
      "2:",
      "jmp 4b",
      ".rept 4",
      "nop",
      ".endr",
      "3:",
      select = sym select_fallback,
      fallback = sym direct_fallback,
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
  let out: u64;

  // SAFETY: Mach-O and COFF keep trampolines in the same text section to avoid unsupported
  // cross-section branch relocations. The patched hot gate still removes selected-index dispatch.
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
      #[cfg(target_os = "macos")]
      "lea rdi, [rip + 2b]",
      #[cfg(target_os = "macos")]
      "lea rsi, [rip + 4b]",
      #[cfg(target_os = "macos")]
      "lea rdx, [rip + 5f]",
      #[cfg(target_os = "macos")]
      "xor ecx, ecx",
      #[cfg(windows)]
      "lea rcx, [rip + 2b]",
      #[cfg(windows)]
      "lea rdx, [rip + 4b]",
      #[cfg(windows)]
      "lea r8, [rip + 5f]",
      #[cfg(windows)]
      "xor r9d, r9d",
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
      "3:",
      select = sym select_and_patch_current,
      fallback = sym direct_fallback,
      lateout("rax") out,
      clobber_abi("C"),
    );
  }

  out
}

#[cfg(all(target_arch = "x86", not(any(target_os = "macos", windows))))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  let low: u32;
  let high: u32;

  // SAFETY: See the x86_64 backend; i686 returns the u64 tick in EDX:EAX.
  unsafe {
    asm!(
      callsite_section_start!(),
      callsite_record_without_hardware_trampoline!(),
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
      ".popsection",
      ".p2align 3",
      "2:",
      "jmp 4b",
      ".rept 3",
      "nop",
      ".endr",
      "3:",
      select = sym select_fallback,
      fallback = sym direct_fallback,
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
  let low: u32;
  let high: u32;

  // SAFETY: See the portable x86_64 backend; i686 returns the u64 tick in EDX:EAX.
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
      "push 0",
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
      "3:",
      select = sym select_and_patch_current,
      fallback = sym direct_fallback,
      lateout("eax") low,
      lateout("edx") high,
      clobber_abi("C"),
    );
  }

  (u64::from(high) << 32) | u64::from(low)
}

#[cfg(all(target_arch = "aarch64", not(windows)))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  let out: u64;

  // SAFETY: The gate is patched atomically from one branch instruction to either CNTVCT or a
  // direct fallback branch. The C ABI clobber describes the trampoline calls to LLVM.
  unsafe {
    asm!(
      callsite_section_start!(),
      callsite_record_without_hardware_trampoline!(),
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
      ".popsection",
      ".p2align 2",
      "2:",
      "b 4b",
      "3:",
      select = sym select_fallback,
      fallback = sym direct_fallback,
      lateout("x0") out,
      clobber_abi("C"),
    );
  }

  out
}

#[cfg(all(target_arch = "aarch64", windows))]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  let out: u64;

  // SAFETY: COFF keeps trampolines in the same text section to avoid unsupported cross-section
  // branch relocations. The patched hot gate still removes selected-index dispatch.
  unsafe {
    asm!(
      ".p2align 2",
      "2:",
      "b 4f",
      "b 3f",
      ".p2align 2",
      "4:",
      "adr x0, 2b",
      "adr x1, 4b",
      "adr x2, 5f",
      "mov x3, xzr",
      "bl {select}",
      "b 3f",
      ".p2align 2",
      "5:",
      "bl {fallback}",
      "b 3f",
      "3:",
      select = sym select_and_patch_current,
      fallback = sym direct_fallback,
      lateout("x0") out,
      clobber_abi("C"),
    );
  }

  out
}

#[cfg(target_arch = "riscv64")]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  let out: u64;

  // SAFETY: The gate is patched atomically from one jump instruction to either RDCYCLE or a
  // direct fallback jump. The C ABI clobber describes the trampoline calls to LLVM.
  unsafe {
    asm!(
      callsite_section_start!(),
      callsite_record_without_hardware_trampoline!(),
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
      ".popsection",
      ".p2align 2",
      "2:",
      "j 4b",
      "3:",
      select = sym select_fallback,
      fallback = sym direct_fallback,
      lateout("a0") out,
      clobber_abi("C"),
    );
  }

  out
}

#[cfg(target_arch = "powerpc64")]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  let out: u64;

  // SAFETY: The gate is patched atomically from one branch instruction to either MFTB or a
  // direct fallback branch. The C ABI clobber describes the trampoline calls to LLVM.
  unsafe {
    asm!(
      callsite_section_start!(),
      callsite_record_without_hardware_trampoline!(),
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
      ".popsection",
      ".p2align 2",
      "2:",
      "b 4b",
      "3:",
      select = sym select_fallback,
      fallback = sym direct_fallback,
      lateout("r3") out,
      clobber_abi("C"),
    );
  }

  out
}

#[cfg(target_arch = "s390x")]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  let out: u64;

  // SAFETY: s390x uses direct trampolines for both selected hardware and fallback clocks
  // because STCKF writes through memory. The gate still removes selected-index dispatch.
  unsafe {
    asm!(
      callsite_section_start!(),
      callsite_record_with_hardware_trampoline!(),
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
      select = sym select_fallback,
      fallback = sym direct_fallback,
      hardware = sym direct_hardware,
      lateout("r2") out,
      clobber_abi("C"),
    );
  }

  out
}

#[cfg(target_arch = "loongarch64")]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  let out: u64;

  // SAFETY: The gate is patched atomically from one branch instruction to either RDTIME or a
  // direct fallback branch. The C ABI clobber describes the trampoline calls to LLVM.
  unsafe {
    asm!(
      callsite_section_start!(),
      callsite_record_without_hardware_trampoline!(),
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
      ".popsection",
      ".p2align 2",
      "2:",
      "b 4b",
      "3:",
      select = sym select_fallback,
      fallback = sym direct_fallback,
      lateout("$a0") out,
      clobber_abi("C"),
    );
  }

  out
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PatchOutcome {
  Patched,
  AlreadyPatched,
  Failed,
}

#[cfg(not(any(target_os = "macos", windows)))]
fn patch_callsites(selected: u8) -> patch::PatchStats {
  let mut stats = patch::PatchStats::default();

  for record in callsite_records() {
    let Some(patch_address) = NonNull::new(record.patch_address as *mut u8) else {
      continue;
    };
    stats.registered += 1;
    match patch_callsite(patch_address, record, selected) {
      PatchOutcome::Patched => stats.patched += 1,
      PatchOutcome::AlreadyPatched => stats.already_patched += 1,
      PatchOutcome::Failed => stats.failed += 1,
    }
  }

  LAST_PATCH_REGISTERED.store(stats.registered, Ordering::Relaxed);
  LAST_PATCH_PATCHED.store(stats.patched, Ordering::Relaxed);
  LAST_PATCH_ALREADY_PATCHED.store(stats.already_patched, Ordering::Relaxed);
  LAST_PATCH_FAILED.store(stats.failed, Ordering::Relaxed);

  stats
}

fn patch_callsite(
  patch_address: NonNull<u8>,
  record: &CallsiteRecord,
  selected: u8,
) -> PatchOutcome {
  let ptr = patch_address.as_ptr();
  let Some(selected_bytes) = selected_gate_bytes(record, selected) else {
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

  #[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "s390x"))]
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

fn selected_gate_bytes(record: &CallsiteRecord, selected: u8) -> Option<[u8; GATE_LEN]> {
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
      indices::RDCYCLE => Some([0x73, 0x25, 0x00, 0xC0]),
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
    let imm = offset << 1;
    let imm = imm as u32;
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
fn callsite_records() -> &'static [CallsiteRecord] {
  let start = core::ptr::addr_of!(CALLSITE_START) as usize;
  let stop = core::ptr::addr_of!(CALLSITE_STOP) as usize;
  records_from_bounds(start, stop)
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

  // SAFETY: The linker-provided bounds or COFF sentinels cover a contiguous metadata range.
  unsafe {
    core::slice::from_raw_parts(
      start as *const CallsiteRecord,
      byte_len / core::mem::size_of::<CallsiteRecord>(),
    )
  }
}

#[cfg(test)]
fn last_patch_stats() -> patch::PatchStats {
  patch::PatchStats {
    registered: LAST_PATCH_REGISTERED.load(Ordering::Relaxed),
    patched: LAST_PATCH_PATCHED.load(Ordering::Relaxed),
    already_patched: LAST_PATCH_ALREADY_PATCHED.load(Ordering::Relaxed),
    failed: LAST_PATCH_FAILED.load(Ordering::Relaxed),
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn selected_architecture_patches_registered_callsites() {
    let _ = super::ticks();
    let stats = super::last_patch_stats();
    assert_eq!(stats.failed, 0, "{stats:?}");
    assert_ne!(stats.registered, 0, "{stats:?}");
    assert_eq!(stats.patched + stats.already_patched, stats.registered, "{stats:?}");
  }

  #[test]
  fn fallback_branch_gate_has_expected_width() {
    let bytes = super::branch_gate_bytes(0x1000, 0x1040).expect("branch encodes");
    assert_eq!(bytes.len(), super::GATE_LEN);
  }
}
