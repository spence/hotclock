use core::arch::asm;

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

#[inline(always)]
pub fn rdtime() -> u64 {
  let cnt: u64;
  // SAFETY: `rdtime` reads a timer CSR into a general-purpose register and does not access
  // Rust memory.
  unsafe {
    asm!(
        "rdtime {}",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}
