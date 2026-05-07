use core::arch::asm;

/// Reads the architectural time counter.
#[inline(always)]
pub fn rdtime() -> u64 {
  let cnt: u64;
  // SAFETY: `rdtime` reads a timer CSR into a general-purpose register and does not access
  // Rust memory. Platforms that disable user-mode access are rejected by runtime selection.
  unsafe {
    asm!(
        "rdtime {}",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}

/// Reads the architectural cycle counter. This is a Cycles-class source, not an Instant-class
/// elapsed-time source.
#[allow(dead_code)]
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
