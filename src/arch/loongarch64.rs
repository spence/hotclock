use core::arch::asm;

#[inline(always)]
pub fn rdtime() -> u64 {
  let cnt: u64;
  // SAFETY: `rdtime.d` reads the architectural timer into a general-purpose register and does
  // not access Rust memory.
  unsafe {
    asm!(
        "rdtime.d {}, $zero",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}

/// Ordered `rdtime.d`: `dbar 0` is a full memory + execution barrier on
/// LoongArch; the subsequent CSR read cannot be reordered before any
/// preceding `Acquire`-or-stronger observation.
#[inline(always)]
pub fn rdtime_ordered() -> u64 {
  let cnt: u64;
  // SAFETY: `dbar 0; rdtime.d` orders prior accesses and reads the architectural timer;
  // neither touches Rust memory.
  unsafe {
    asm!(
        "dbar 0",
        "rdtime.d {}, $zero",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}
