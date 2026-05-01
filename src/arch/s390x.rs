use core::arch::asm;

#[inline(always)]
pub fn stckf() -> u64 {
  let mut cnt: u64 = 0;
  // SAFETY: `cnt` is a valid writable 8-byte stack slot, and `stckf` stores exactly one
  // clock value through the provided address.
  unsafe {
    asm!(
        "stckf 0({})",
        in(reg) &mut cnt,
        options(nostack, preserves_flags)
    );
  }
  cnt
}
