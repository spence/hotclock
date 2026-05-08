use core::arch::asm;
use core::ptr::NonNull;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use super::patch;

const PATCH_LEN: usize = RDTSC_BYTES.len();
const COMMIT_LEN: usize = 8;
const UNSELECTED: u8 = u8::MAX;
const SELECTING: u8 = u8::MAX - 1;
const RDTSC_BYTES: [u8; 9] = [0x0F, 0x31, 0x48, 0xC1, 0xE2, 0x20, 0x48, 0x09, 0xD0];

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

pub mod indices {
  pub const RDTSC: u8 = 0;
  pub const CLOCK_MONOTONIC: u8 = 1;
  pub const DIRECT_RDPMC: u8 = 2;
  pub const PERF_RDPMC: u8 = 3;
}

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
  direct_rdpmc_address: usize,
  perf_rdpmc_address: usize,
}

#[used]
#[unsafe(link_section = "hotclock_x86_64_instant_callsite")]
static INSTANT_CALLSITE_SENTINEL: CallsiteRecord = CallsiteRecord {
  patch_address: 0,
  cold_address: 0,
  fallback_address: 0,
  direct_rdpmc_address: 0,
  perf_rdpmc_address: 0,
};

#[used]
#[unsafe(link_section = "hotclock_x86_64_cycle_callsite")]
static CYCLE_CALLSITE_SENTINEL: CallsiteRecord = CallsiteRecord {
  patch_address: 0,
  cold_address: 0,
  fallback_address: 0,
  direct_rdpmc_address: 0,
  perf_rdpmc_address: 0,
};

unsafe extern "C" {
  #[link_name = "__start_hotclock_x86_64_instant_callsite"]
  static INSTANT_CALLSITE_START: CallsiteRecord;

  #[link_name = "__stop_hotclock_x86_64_instant_callsite"]
  static INSTANT_CALLSITE_STOP: CallsiteRecord;

  #[link_name = "__start_hotclock_x86_64_cycle_callsite"]
  static CYCLE_CALLSITE_START: CallsiteRecord;

  #[link_name = "__stop_hotclock_x86_64_cycle_callsite"]
  static CYCLE_CALLSITE_STOP: CallsiteRecord;
}

macro_rules! callsite_record {
  () => {
    ".balign 8\n.quad 2f\n.quad 4f\n.quad 5f\n.quad 6f\n.quad 7f"
  };
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  let out: u64;

  // SAFETY: This emits a crate-owned 9-byte gate plus cold and selected trampolines. The first
  // call selects and patches all registered gates. Warmed RDTSC gates become raw RDTSC bytes;
  // fallback gates become direct jumps to the monotonic trampoline.
  unsafe {
    asm!(
      ".pushsection hotclock_x86_64_instant_callsite,\"awR\",@progbits",
      callsite_record!(),
      ".popsection",
      ".pushsection .text.hotclock_x86_64_cold,\"ax\",@progbits",
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
      "call {direct_rdpmc}",
      "mov rsp, qword ptr [rsp + 32]",
      "jmp 3f",
      ".p2align 4",
      "7:",
      "mov r11, rsp",
      "and rsp, -16",
      "sub rsp, 48",
      "mov qword ptr [rsp + 32], r11",
      "call {perf_rdpmc}",
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
      fallback = sym direct_clock_monotonic,
      direct_rdpmc = sym direct_rdpmc,
      perf_rdpmc = sym perf_rdpmc,
      lateout("rax") out,
      clobber_abi("C"),
    );
  }

  out
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_ticks() -> u64 {
  let out: u64;

  // SAFETY: Same patchpoint shape as `ticks`, with independent cycle selection and cycle
  // trampolines for RDPMC-backed sources.
  unsafe {
    asm!(
      ".pushsection hotclock_x86_64_cycle_callsite,\"awR\",@progbits",
      callsite_record!(),
      ".popsection",
      ".pushsection .text.hotclock_x86_64_cold,\"ax\",@progbits",
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
      "call {direct_rdpmc}",
      "mov rsp, qword ptr [rsp + 32]",
      "jmp 3f",
      ".p2align 4",
      "7:",
      "mov r11, rsp",
      "and rsp, -16",
      "sub rsp, 48",
      "mov qword ptr [rsp + 32], r11",
      "call {perf_rdpmc}",
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
      fallback = sym direct_clock_monotonic,
      direct_rdpmc = sym direct_rdpmc,
      perf_rdpmc = sym perf_rdpmc,
      lateout("rax") out,
      clobber_abi("C"),
    );
  }

  out
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

extern "C" fn instant_select_fallback() -> u64 {
  read_selected(ensure_selected())
}

extern "C" fn cycle_select_fallback() -> u64 {
  read_cycle_selected(ensure_cycle_selected())
}

extern "C" fn direct_clock_monotonic() -> u64 {
  super::fallback::clock_monotonic()
}

extern "C" fn direct_rdpmc() -> u64 {
  super::perf_rdpmc_linux::direct_rdpmc_fixed_core_cycles()
}

extern "C" fn perf_rdpmc() -> u64 {
  super::perf_rdpmc_linux::perf_rdpmc_cpu_cycles().unwrap_or_else(super::x86_64::rdtsc)
}

#[inline(always)]
fn read_selected(sel: u8) -> u64 {
  match sel {
    indices::RDTSC => super::x86_64::rdtsc(),
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    _ => unreachable!("invalid selected x86_64 Linux counter"),
  }
}

#[inline(always)]
fn read_cycle_selected(sel: u8) -> u64 {
  match sel {
    indices::DIRECT_RDPMC => super::perf_rdpmc_linux::direct_rdpmc_fixed_core_cycles(),
    indices::PERF_RDPMC => {
      super::perf_rdpmc_linux::perf_rdpmc_cpu_cycles().unwrap_or_else(super::x86_64::rdtsc)
    }
    indices::RDTSC => super::x86_64::rdtsc(),
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    _ => unreachable!("invalid selected x86_64 Linux cycle counter"),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PatchOutcome {
  Patched,
  AlreadyPatched,
  Failed,
}

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
  let Some(unselected_bytes) = branch_bytes(record.patch_address, record.cold_address) else {
    return PatchOutcome::Failed;
  };

  let current = read_patch_bytes(ptr);
  if current == selected_bytes {
    return PatchOutcome::AlreadyPatched;
  }
  if current != unselected_bytes {
    return PatchOutcome::Failed;
  }

  if patch::patch_bytes_with_u64_commit(ptr, &selected_bytes, COMMIT_LEN) {
    PatchOutcome::Patched
  } else {
    PatchOutcome::Failed
  }
}

fn selected_gate_bytes(
  class: ClockClass,
  record: &CallsiteRecord,
  selected: u8,
) -> Option<[u8; PATCH_LEN]> {
  match class {
    ClockClass::Instant => match selected {
      indices::RDTSC => Some(RDTSC_BYTES),
      indices::CLOCK_MONOTONIC => branch_bytes(record.patch_address, record.fallback_address),
      _ => None,
    },
    ClockClass::Cycles => match selected {
      indices::RDTSC => Some(RDTSC_BYTES),
      indices::CLOCK_MONOTONIC => branch_bytes(record.patch_address, record.fallback_address),
      indices::DIRECT_RDPMC => branch_bytes(record.patch_address, record.direct_rdpmc_address),
      indices::PERF_RDPMC => branch_bytes(record.patch_address, record.perf_rdpmc_address),
      _ => None,
    },
  }
}

fn branch_bytes(from: usize, to: usize) -> Option<[u8; PATCH_LEN]> {
  let rel = patch::branch_i32(from, to, 5)?;
  let mut bytes = [0x90; PATCH_LEN];
  bytes[0] = 0xE9;
  bytes[1..5].copy_from_slice(&rel.to_le_bytes());
  Some(bytes)
}

fn callsite_records(class: ClockClass) -> &'static [CallsiteRecord] {
  let (start, stop) = match class {
    ClockClass::Instant => (
      core::ptr::addr_of!(INSTANT_CALLSITE_START) as usize,
      core::ptr::addr_of!(INSTANT_CALLSITE_STOP) as usize,
    ),
    ClockClass::Cycles => (
      core::ptr::addr_of!(CYCLE_CALLSITE_START) as usize,
      core::ptr::addr_of!(CYCLE_CALLSITE_STOP) as usize,
    ),
  };

  if stop < start {
    return &[];
  }

  let byte_len = stop - start;
  if byte_len % core::mem::size_of::<CallsiteRecord>() != 0 {
    return &[];
  }

  // SAFETY: The linker start/stop symbols bound a metadata section populated by this module's
  // inline assembly plus the sentinel record.
  unsafe {
    core::slice::from_raw_parts(
      start as *const CallsiteRecord,
      byte_len / core::mem::size_of::<CallsiteRecord>(),
    )
  }
}

fn read_patch_bytes(ptr: *mut u8) -> [u8; PATCH_LEN] {
  let mut bytes = [0_u8; PATCH_LEN];
  // SAFETY: `ptr` is a call-site patch address emitted by this crate and points at a readable
  // fixed-size patch region.
  unsafe {
    core::ptr::copy_nonoverlapping(ptr.cast_const(), bytes.as_mut_ptr(), PATCH_LEN);
  }
  bytes
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
  use super::{ClockClass, RDTSC_BYTES};

  #[test]
  fn rdtsc_selection_patches_registered_instant_callsites_when_selected() {
    let _ = super::ticks();
    if super::implementation() != "x86_64-rdtsc" {
      return;
    }

    let callsites: Vec<_> = super::callsite_records(ClockClass::Instant)
      .iter()
      .filter_map(|record| core::ptr::NonNull::new(record.patch_address as *mut u8))
      .map(|address| super::read_patch_bytes(address.as_ptr()))
      .collect();
    assert!(!callsites.is_empty());
    assert!(callsites.iter().all(|bytes| bytes == &RDTSC_BYTES), "{callsites:02x?}");

    let stats = super::last_patch_stats(ClockClass::Instant);
    assert_eq!(stats.failed, 0, "{stats:?}");
    assert_eq!(stats.patched + stats.already_patched, stats.registered, "{stats:?}");
  }

  #[test]
  fn cycle_selection_patches_registered_callsites() {
    let _ = super::cycle_ticks();
    let stats = super::last_patch_stats(ClockClass::Cycles);
    assert_eq!(stats.failed, 0, "{stats:?}");
    assert_eq!(stats.patched + stats.already_patched, stats.registered, "{stats:?}");
  }

  #[test]
  fn rdtsc_patch_bytes_fill_the_entire_hot_region() {
    assert_eq!(RDTSC_BYTES.len(), super::PATCH_LEN);
  }
}
