use core::arch::asm;

#[inline(always)]
pub fn rdtime() -> u64 {
  let cnt: u64;
  unsafe {
    asm!(
        "rdtime.d {}, $zero",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}
