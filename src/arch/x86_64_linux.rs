use core::arch::asm;
use core::ptr::NonNull;
use std::ffi::{c_int, c_long, c_void};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering, compiler_fence};

const PATCH_LEN: usize = RDTSC_BYTES.len();
const COMMIT_LEN: usize = 8;
const UNSELECTED: u8 = u8::MAX;
const SELECTING: u8 = u8::MAX - 1;
const PROT_READ: c_int = 0x1;
const PROT_WRITE: c_int = 0x2;
const PROT_EXEC: c_int = 0x4;
const SC_PAGESIZE: c_int = 30;

const RDTSC_BYTES: [u8; 9] = [0x0F, 0x31, 0x48, 0xC1, 0xE2, 0x20, 0x48, 0x09, 0xD0];

static SELECTED: AtomicU8 = AtomicU8::new(UNSELECTED);
static SELECTED_NAME: OnceLock<&'static str> = OnceLock::new();
static CYCLE_SELECTED: AtomicU8 = AtomicU8::new(UNSELECTED);
static CYCLE_SELECTED_NAME: OnceLock<&'static str> = OnceLock::new();

pub mod indices {
  pub const RDTSC: u8 = 0;
  pub const CLOCK_MONOTONIC: u8 = 1;
  pub const DIRECT_RDPMC: u8 = 2;
  pub const PERF_RDPMC: u8 = 3;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CallsiteRecord {
  patch_address: usize,
}

#[used]
#[unsafe(link_section = "hotclock_x86_64_callsite")]
static CALLSITE_SENTINEL: CallsiteRecord = CallsiteRecord { patch_address: 0 };

unsafe extern "C" {
  #[link_name = "__start_hotclock_x86_64_callsite"]
  static CALLSITE_START: CallsiteRecord;

  #[link_name = "__stop_hotclock_x86_64_callsite"]
  static CALLSITE_STOP: CallsiteRecord;

  fn sysconf(name: c_int) -> c_long;
  fn mprotect(addr: *mut c_void, len: usize, prot: c_int) -> c_int;
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn ticks() -> u64 {
  let out: u64;

  // SAFETY: This assembly emits a crate-owned 9-byte gate and a cold trampoline for this exact
  // call site. Before selection, execution jumps to the cold trampoline, which preserves the
  // caller's registers around the Rust fallback call and then jumps back to the continuation.
  // After an RDTSC selection, the gate is atomically replaced with `RDTSC_BYTES`, which leaves
  // the tick value in RAX and falls through directly to the continuation.
  unsafe {
    asm!(
      ".pushsection hotclock_x86_64_callsite,\"awR\",@progbits",
      ".balign 8",
      ".quad 2f",
      ".popsection",
      ".pushsection .text.hotclock_x86_64_cold,\"ax\",@progbits",
      ".p2align 4",
      "4:",
      "push rcx",
      "push rsi",
      "push rdi",
      "push r8",
      "push r9",
      "push r10",
      "push r11",
      "mov r11, rsp",
      "and rsp, -16",
      "sub rsp, 272",
      "mov qword ptr [rsp + 256], r11",
      "movdqu xmmword ptr [rsp + 0], xmm0",
      "movdqu xmmword ptr [rsp + 16], xmm1",
      "movdqu xmmword ptr [rsp + 32], xmm2",
      "movdqu xmmword ptr [rsp + 48], xmm3",
      "movdqu xmmword ptr [rsp + 64], xmm4",
      "movdqu xmmword ptr [rsp + 80], xmm5",
      "movdqu xmmword ptr [rsp + 96], xmm6",
      "movdqu xmmword ptr [rsp + 112], xmm7",
      "movdqu xmmword ptr [rsp + 128], xmm8",
      "movdqu xmmword ptr [rsp + 144], xmm9",
      "movdqu xmmword ptr [rsp + 160], xmm10",
      "movdqu xmmword ptr [rsp + 176], xmm11",
      "movdqu xmmword ptr [rsp + 192], xmm12",
      "movdqu xmmword ptr [rsp + 208], xmm13",
      "movdqu xmmword ptr [rsp + 224], xmm14",
      "movdqu xmmword ptr [rsp + 240], xmm15",
      "call {fallback}",
      "movdqu xmm0, xmmword ptr [rsp + 0]",
      "movdqu xmm1, xmmword ptr [rsp + 16]",
      "movdqu xmm2, xmmword ptr [rsp + 32]",
      "movdqu xmm3, xmmword ptr [rsp + 48]",
      "movdqu xmm4, xmmword ptr [rsp + 64]",
      "movdqu xmm5, xmmword ptr [rsp + 80]",
      "movdqu xmm6, xmmword ptr [rsp + 96]",
      "movdqu xmm7, xmmword ptr [rsp + 112]",
      "movdqu xmm8, xmmword ptr [rsp + 128]",
      "movdqu xmm9, xmmword ptr [rsp + 144]",
      "movdqu xmm10, xmmword ptr [rsp + 160]",
      "movdqu xmm11, xmmword ptr [rsp + 176]",
      "movdqu xmm12, xmmword ptr [rsp + 192]",
      "movdqu xmm13, xmmword ptr [rsp + 208]",
      "movdqu xmm14, xmmword ptr [rsp + 224]",
      "movdqu xmm15, xmmword ptr [rsp + 240]",
      "mov r11, qword ptr [rsp + 256]",
      "mov rsp, r11",
      "pop r11",
      "pop r10",
      "pop r9",
      "pop r8",
      "pop rdi",
      "pop rsi",
      "pop rcx",
      "jmp 3f",
      ".popsection",
      ".p2align 3",
      "2:",
      "jmp 4b",
      ".rept 4",
      "nop",
      ".endr",
      "3:",
      fallback = sym select_fallback,
      lateout("rax") out,
      lateout("rdx") _,
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
pub fn cycle_ticks() -> u64 {
  match ensure_cycle_selected() {
    indices::DIRECT_RDPMC => super::perf_rdpmc_linux::direct_rdpmc_fixed_core_cycles(),
    indices::PERF_RDPMC => {
      super::perf_rdpmc_linux::perf_rdpmc_cpu_cycles().unwrap_or_else(super::x86_64::rdtsc)
    }
    indices::RDTSC => super::x86_64::rdtsc(),
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    _ => unreachable!("invalid selected x86_64 Linux cycle counter"),
  }
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cycle_implementation() -> &'static str {
  ensure_cycle_selected();
  CYCLE_SELECTED_NAME.get().copied().unwrap_or("unknown")
}

extern "C" fn select_fallback() -> u64 {
  match ensure_selected() {
    indices::RDTSC => super::x86_64::rdtsc(),
    indices::CLOCK_MONOTONIC => super::fallback::clock_monotonic(),
    _ => unreachable!("invalid selected x86_64 Linux counter"),
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
          CYCLE_SELECTED.store(idx, Ordering::Release);
          return idx;
        }
      }
      SELECTING => std::hint::spin_loop(),
      selected => return selected,
    }
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
          if idx == indices::RDTSC {
            patch_rdtsc_callsites();
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

fn patch_rdtsc_callsites() {
  for record in callsite_records() {
    let Some(patch_address) = NonNull::new(record.patch_address as *mut u8) else {
      continue;
    };
    let _ = patch_rdtsc_callsite(patch_address);
  }
}

fn callsite_records() -> &'static [CallsiteRecord] {
  let start = core::ptr::addr_of!(CALLSITE_START) as usize;
  let stop = core::ptr::addr_of!(CALLSITE_STOP) as usize;
  if stop < start {
    return &[];
  }

  let byte_len = stop - start;
  if byte_len % core::mem::size_of::<CallsiteRecord>() != 0 {
    return &[];
  }

  // SAFETY: The linker start/stop symbols bound the contiguous metadata section populated by
  // the inline assembly above plus the sentinel record in this module.
  unsafe {
    core::slice::from_raw_parts(
      core::ptr::addr_of!(CALLSITE_START),
      byte_len / core::mem::size_of::<CallsiteRecord>(),
    )
  }
}

fn patch_rdtsc_callsite(patch_address: NonNull<u8>) -> bool {
  let ptr = patch_address.as_ptr();
  let address = ptr as usize;
  if address % COMMIT_LEN != 0 {
    return false;
  }

  let current = read_patch_bytes(ptr);
  if current == RDTSC_BYTES {
    return true;
  }
  if !is_unselected_gate(&current) {
    return false;
  }

  let Ok(guard) = PageWriteGuard::enable(ptr, PATCH_LEN) else {
    return false;
  };

  // SAFETY: The page guard made the crate-owned call-site bytes writable. The old gate is an
  // out-of-line jump, so writing the final tail byte first cannot affect threads that already
  // entered the cold path. The aligned atomic store commits the first eight selected bytes in
  // one step.
  unsafe {
    ptr.add(COMMIT_LEN).write(RDTSC_BYTES[COMMIT_LEN]);
    compiler_fence(Ordering::SeqCst);
    AtomicU64::from_ptr(ptr.cast::<u64>())
      .store(u64::from_le_bytes(RDTSC_BYTES[..COMMIT_LEN].try_into().unwrap()), Ordering::SeqCst);
  }

  flush_instruction_cache();
  let _ = guard.restore();
  true
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

fn is_unselected_gate(bytes: &[u8; PATCH_LEN]) -> bool {
  bytes[0] == 0xE9 && bytes[5..] == [0x90, 0x90, 0x90, 0x90]
}

fn flush_instruction_cache() {
  compiler_fence(Ordering::SeqCst);
}

struct PageWriteGuard {
  start: *mut c_void,
  len: usize,
  active: bool,
}

impl PageWriteGuard {
  fn enable(ptr: *mut u8, len: usize) -> Result<Self, ()> {
    let (start, len) = page_span(ptr as usize, len)?;
    let mut guard = Self { start: start as *mut c_void, len, active: false };
    guard.mprotect(PROT_READ | PROT_WRITE | PROT_EXEC)?;
    guard.active = true;
    Ok(guard)
  }

  fn restore(mut self) -> Result<(), ()> {
    self.mprotect(PROT_READ | PROT_EXEC)?;
    self.active = false;
    Ok(())
  }

  fn mprotect(&self, protection: c_int) -> Result<(), ()> {
    // SAFETY: `start` and `len` are page-aligned values returned by `page_span`, and
    // `protection` is a valid Linux page-protection bitset.
    if unsafe { mprotect(self.start, self.len, protection) } == 0 { Ok(()) } else { Err(()) }
  }
}

impl Drop for PageWriteGuard {
  fn drop(&mut self) {
    if self.active {
      let _ = self.mprotect(PROT_READ | PROT_EXEC);
    }
  }
}

fn page_span(addr: usize, len: usize) -> Result<(usize, usize), ()> {
  let page_size = page_size()?;
  let page_mask = !(page_size - 1);
  let start = addr & page_mask;
  let end = addr.checked_add(len).ok_or(())?;
  let end = end.checked_add(page_size - 1).ok_or(())? & page_mask;
  let len = end.checked_sub(start).ok_or(())?;
  Ok((start, len))
}

fn page_size() -> Result<usize, ()> {
  // SAFETY: `sysconf(_SC_PAGESIZE)` has no Rust-side safety preconditions.
  let value = unsafe { sysconf(SC_PAGESIZE) };
  if value <= 0 {
    return Err(());
  }
  usize::try_from(value).map_err(|_| ())
}

#[cfg(test)]
mod tests {
  use super::{PATCH_LEN, RDTSC_BYTES};

  #[test]
  fn rdtsc_selection_patches_registered_callsites_when_selected() {
    let _ = super::ticks();
    if super::implementation() != "x86_64-rdtsc" {
      return;
    }

    let callsites: Vec<_> = super::callsite_records()
      .iter()
      .filter_map(|record| core::ptr::NonNull::new(record.patch_address as *mut u8))
      .map(|address| super::read_patch_bytes(address.as_ptr()))
      .collect();
    assert!(!callsites.is_empty());
    assert!(callsites.iter().all(|bytes| bytes == &RDTSC_BYTES), "{callsites:02x?}");
  }

  #[test]
  fn rdtsc_patch_bytes_fill_the_entire_hot_region() {
    assert_eq!(RDTSC_BYTES.len(), PATCH_LEN);
  }
}
