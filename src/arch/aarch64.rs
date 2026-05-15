use core::arch::asm;

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cntvct() -> u64 {
  let cnt: u64;
  // SAFETY: `mrs cntvct_el0` only reads the architectural virtual counter register and does
  // not touch memory or the stack.
  unsafe {
    asm!(
        "mrs {}, cntvct_el0",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}

