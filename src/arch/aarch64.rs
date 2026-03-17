use core::arch::asm;

#[inline(always)]
pub fn cntvct() -> u64 {
  let cnt: u64;
  unsafe {
    asm!(
        "mrs {}, cntvct_el0",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}
