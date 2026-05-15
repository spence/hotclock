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
