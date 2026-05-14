//! Cycles patchpoint mechanism for Linux aarch64.
//!
//! Provides `cycle_ticks()` whose 4-byte callsite is rewritten on first call to
//! inline the winning Cycles candidate's bytes (raw `mrs x0, cntvct_el0` for the
//! wall-clock fallback, or a `bl` to a trampoline that calls into
//! `perf_pmccntr_linux` for the perf-PMCCNTR path, or a `bl` to a clock_monotonic
//! trampoline for the last-resort fallback). After patching, subsequent reads have
//! zero dispatch overhead on the CNTVCT path — the call site IS the raw `mrs`.
//!
//! Instant does not use this mechanism. Instant compiles directly to
//! `mrs x0, cntvct_el0` via `super::direct::ticks()` — every benchmarked
//! (target × env) cell picked CNTVCT_EL0, so no runtime selection is needed
//! for Instant.

use core::arch::asm;
use core::ptr::NonNull;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use super::patch;

const PATCH_LEN: usize = 4;
const UNSELECTED: u8 = u8::MAX;
const SELECTING: u8 = u8::MAX - 1;

// `mrs x0, cntvct_el0` little-endian bytes. Op0=3, Op1=3, CRn=14, CRm=0, Op2=2, Rt=0.
const CNTVCT_BYTES: [u8; 4] = [0x40, 0xE0, 0x3B, 0xD5];

static CYCLE_SELECTED: AtomicU8 = AtomicU8::new(UNSELECTED);
static CYCLE_SELECTED_NAME: OnceLock<&'static str> = OnceLock::new();

static CYCLE_PATCH_REGISTERED: AtomicUsize = AtomicUsize::new(0);
static CYCLE_PATCH_PATCHED: AtomicUsize = AtomicUsize::new(0);
static CYCLE_PATCH_ALREADY_PATCHED: AtomicUsize = AtomicUsize::new(0);
static CYCLE_PATCH_FAILED: AtomicUsize = AtomicUsize::new(0);

pub mod indices {
  pub const CNTVCT: u8 = 0;
  pub const CLOCK_MONOTONIC: u8 = 1;
  pub const PERF_PMCCNTR: u8 = 2;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CallsiteRecord {
  patch_address: usize,
  cold_address: usize,
  fallback_address: usize,
  perf_pmccntr_address: usize,
}

#[used]
#[unsafe(link_section = "tach_aarch64_cycle_callsite")]
static CYCLE_CALLSITE_SENTINEL: CallsiteRecord = CallsiteRecord {
  patch_address: 0,
  cold_address: 0,
  fallback_address: 0,
  perf_pmccntr_address: 0,
};

unsafe extern "C" {
  #[link_name = "__start_tach_aarch64_cycle_callsite"]
  static CYCLE_CALLSITE_START: CallsiteRecord;

  #[link_name = "__stop_tach_aarch64_cycle_callsite"]
  static CYCLE_CALLSITE_STOP: CallsiteRecord;
}

macro_rules! callsite_record {
  () => {
    ".balign 8\n.quad 2f\n.quad 4f\n.quad 5f\n.quad 6f"
  };
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_ticks() -> u64 {
  let out: u64;

  // SAFETY: Emits a crate-owned 4-byte gate plus cold and trampoline paths in a
  // dedicated text section. The first call selects the fastest Cycles candidate
  // and patches all registered gates. The selected gate is either inline
  // `mrs x0, cntvct_el0` (wall-clock fallback — zero dispatch overhead) or a
  // `bl` to one of the trampolines below.
  unsafe {
    asm!(
      ".pushsection tach_aarch64_cycle_callsite,\"awR\",@progbits",
      callsite_record!(),
      ".popsection",
      ".pushsection .text.tach_aarch64_cold,\"ax\",@progbits",
      ".balign 4",
      "4:",
      "stp x29, x30, [sp, -16]!",
      "mov x29, sp",
      "bl {select}",
      "ldp x29, x30, [sp], 16",
      "ret",
      ".balign 4",
      "5:",
      "stp x29, x30, [sp, -16]!",
      "mov x29, sp",
      "bl {fallback}",
      "ldp x29, x30, [sp], 16",
      "ret",
      ".balign 4",
      "6:",
      "stp x29, x30, [sp, -16]!",
      "mov x29, sp",
      "bl {perf_pmccntr}",
      "ldp x29, x30, [sp], 16",
      "ret",
      ".popsection",
      ".balign 4",
      "2:",
      "bl 4b",
      select = sym cycle_select_fallback,
      fallback = sym direct_clock_monotonic,
      perf_pmccntr = sym perf_pmccntr,
      lateout("x0") out,
      clobber_abi("C"),
    );
  }

  out
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_implementation() -> &'static str {
  ensure_cycle_selected();
  CYCLE_SELECTED_NAME.get().copied().unwrap_or("unknown")
}

extern "C" fn cycle_select_fallback() -> u64 {
  read_cycle_selected(ensure_cycle_selected())
}

extern "C" fn direct_clock_monotonic() -> u64 {
  super::fallback::clock_monotonic()
}

extern "C" fn perf_pmccntr() -> u64 {
  super::perf_pmccntr_linux::perf_pmccntr_cpu_cycles().unwrap_or_else(super::aarch64::cntvct)
}

#[inline(always)]
fn read_cycle_selected(sel: u8) -> u64 {
  match sel {
    indices::PERF_PMCCNTR => {
      super::perf_pmccntr_linux::perf_pmccntr_cpu_cycles().unwrap_or_else(super::aarch64::cntvct)
    }
    indices::CNTVCT => super::aarch64::cntvct(),
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    _ => unreachable!("invalid selected aarch64 Cycles counter"),
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
          store_patch_stats(patch_callsites(idx));
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
  let Some(unselected_bytes) = bl_bytes(record.patch_address, record.cold_address) else {
    return PatchOutcome::Failed;
  };

  let current = read_patch_bytes(ptr);
  if current == selected_bytes {
    return PatchOutcome::AlreadyPatched;
  }
  if current != unselected_bytes {
    return PatchOutcome::Failed;
  }

  let word = u32::from_le_bytes(selected_bytes);
  if patch::patch_u32(ptr, word) {
    PatchOutcome::Patched
  } else {
    PatchOutcome::Failed
  }
}

fn selected_gate_bytes(record: &CallsiteRecord, selected: u8) -> Option<[u8; PATCH_LEN]> {
  match selected {
    indices::CNTVCT => Some(CNTVCT_BYTES),
    indices::CLOCK_MONOTONIC => bl_bytes(record.patch_address, record.fallback_address),
    indices::PERF_PMCCNTR => bl_bytes(record.patch_address, record.perf_pmccntr_address),
    _ => None,
  }
}

fn bl_bytes(from: usize, to: usize) -> Option<[u8; PATCH_LEN]> {
  let offset_words = patch::branch_words(from, to, 2, 26)?;
  // `BL <imm26>`: opcode `100101` (bit pattern 0x94000000) | (imm26 & 0x03FFFFFF).
  let imm26 = (offset_words as u32) & 0x03FF_FFFF;
  let word = 0x9400_0000_u32 | imm26;
  Some(word.to_le_bytes())
}

fn callsite_records() -> &'static [CallsiteRecord] {
  let start = core::ptr::addr_of!(CYCLE_CALLSITE_START) as usize;
  let stop = core::ptr::addr_of!(CYCLE_CALLSITE_STOP) as usize;

  if stop < start {
    return &[];
  }

  let byte_len = stop - start;
  if byte_len % core::mem::size_of::<CallsiteRecord>() != 0 {
    return &[];
  }

  // SAFETY: The linker start/stop symbols bound a metadata section populated by this
  // module's inline assembly plus the sentinel record.
  unsafe {
    core::slice::from_raw_parts(
      start as *const CallsiteRecord,
      byte_len / core::mem::size_of::<CallsiteRecord>(),
    )
  }
}

fn read_patch_bytes(ptr: *mut u8) -> [u8; PATCH_LEN] {
  let mut bytes = [0_u8; PATCH_LEN];
  // SAFETY: `ptr` is a call-site patch address emitted by this crate and points at a
  // readable 4-byte aligned patch region.
  unsafe {
    core::ptr::copy_nonoverlapping(ptr.cast_const(), bytes.as_mut_ptr(), PATCH_LEN);
  }
  bytes
}

fn store_patch_stats(stats: patch::PatchStats) {
  CYCLE_PATCH_REGISTERED.store(stats.registered, Ordering::Relaxed);
  CYCLE_PATCH_PATCHED.store(stats.patched, Ordering::Relaxed);
  CYCLE_PATCH_ALREADY_PATCHED.store(stats.already_patched, Ordering::Relaxed);
  CYCLE_PATCH_FAILED.store(stats.failed, Ordering::Relaxed);
}

#[cfg(test)]
fn last_patch_stats() -> patch::PatchStats {
  patch::PatchStats {
    registered: CYCLE_PATCH_REGISTERED.load(Ordering::Relaxed),
    patched: CYCLE_PATCH_PATCHED.load(Ordering::Relaxed),
    already_patched: CYCLE_PATCH_ALREADY_PATCHED.load(Ordering::Relaxed),
    failed: CYCLE_PATCH_FAILED.load(Ordering::Relaxed),
  }
}

#[cfg(test)]
mod tests {
  use super::CNTVCT_BYTES;

  #[test]
  fn cntvct_bytes_encode_mrs_cntvct_el0() {
    // `mrs x0, cntvct_el0` standard encoding: 0xD53BE040.
    assert_eq!(u32::from_le_bytes(CNTVCT_BYTES), 0xD53B_E040);
  }

  #[test]
  fn cycle_selection_patches_registered_callsites() {
    let _ = super::cycle_ticks();
    let stats = super::last_patch_stats();
    assert_eq!(stats.failed, 0, "{stats:?}");
    assert_eq!(stats.patched + stats.already_patched, stats.registered, "{stats:?}");
  }
}
