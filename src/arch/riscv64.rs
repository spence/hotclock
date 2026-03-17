use core::arch::asm;

/// May be disabled by kernel for security.
#[inline(always)]
pub fn rdcycle() -> u64 {
  let cnt: u64;
  unsafe {
    asm!(
        "rdcycle {}",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}

#[inline(always)]
pub fn rdtime() -> u64 {
  let cnt: u64;
  unsafe {
    asm!(
        "rdtime {}",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}
