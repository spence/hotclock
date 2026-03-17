use core::arch::asm;

#[inline(always)]
pub fn stckf() -> u64 {
  let mut cnt: u64 = 0;
  unsafe {
    asm!(
        "stckf 0({})",
        in(reg) &mut cnt,
        options(nostack, preserves_flags)
    );
  }
  cnt
}
