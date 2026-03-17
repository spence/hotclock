use core::arch::asm;

/// May trap (SIGILL) if not enabled by kernel.
#[inline(always)]
pub fn pmccntr() -> u64 {
  let cnt: u64;
  unsafe {
    asm!(
        "mrs {}, pmccntr_el0",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}
