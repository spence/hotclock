use core::arch::asm;

/// Reads the architectural time counter.
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

/// Ordered `rdtime`: `fence ir, ir` orders prior instructions+reads vs
/// subsequent instructions+reads so the CSR read cannot be hoisted above a
/// preceding `Acquire`-or-stronger observation.
#[inline(always)]
pub fn rdtime_ordered() -> u64 {
  let cnt: u64;
  // SAFETY: `fence ir, ir; rdtime` only sequences execution and reads a CSR; no memory access.
  unsafe {
    asm!(
        "fence ir, ir",
        "rdtime {}",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}
