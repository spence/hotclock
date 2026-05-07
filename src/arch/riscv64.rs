use core::arch::asm;

/// Architectural elapsed-time counter.
#[inline(always)]
pub fn rdtime() -> u64 {
  let cnt: u64;
  // SAFETY: `rdtime` reads the time CSR into a general-purpose register and does not access
  // Rust memory. Platforms that disable user-mode access are rejected by runtime validation.
  unsafe {
    asm!(
        "rdtime {}",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}

/// May be disabled by kernel for security.
#[inline(always)]
pub fn rdcycle() -> u64 {
  let cnt: u64;
  // SAFETY: `rdcycle` reads a counter CSR into a general-purpose register and does not access
  // Rust memory. Platforms that disable user-mode access are rejected by runtime selection.
  unsafe {
    asm!(
        "rdcycle {}",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}
