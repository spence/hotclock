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

#[inline]
pub fn cntfrq() -> u64 {
  let freq: u64;
  // SAFETY: `mrs cntfrq_el0` only reads the architectural counter frequency register and
  // does not touch memory or the stack. The low 32 bits hold the timer rate in Hz.
  unsafe {
    asm!(
        "mrs {}, cntfrq_el0",
        out(reg) freq,
        options(nostack, nomem, preserves_flags)
    );
  }
  freq
}
