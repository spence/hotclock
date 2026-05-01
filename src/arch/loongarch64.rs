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
