use core::arch::asm;

#[inline(always)]
pub fn mftb() -> u64 {
  let cnt: u64;
  // SAFETY: `mftb` copies the time-base register into a general-purpose register and does
  // not access Rust memory.
  unsafe {
    asm!(
        "mftb {}",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}
