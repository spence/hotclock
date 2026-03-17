use core::arch::asm;

#[inline(always)]
pub fn mftb() -> u64 {
  let cnt: u64;
  unsafe {
    asm!(
        "mftb {}",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}
