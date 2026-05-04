use core::arch::asm;

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn cntvct() -> u64 {
  let cnt: u64;
  // SAFETY: `mrs cntvct_el0` only reads the architectural virtual counter register and does
  // not touch memory or the stack. Targets that cannot execute this instruction must not use
  // the direct path; runtime-selected targets validate the read before installing it.
  unsafe {
    asm!(
        "mrs {}, cntvct_el0",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}

#[inline(always)]
#[cfg(all(feature = "bench-internals", target_os = "linux"))]
#[allow(clippy::inline_always)]
pub fn pmccntr_el0() -> u64 {
  let cnt: u64;
  // SAFETY: this only reads the architectural PMU cycle counter register. Kernels that do not
  // enable userspace PMU access will trap; benchmark callers must run this in a child process.
  unsafe {
    asm!(
        "mrs {}, pmccntr_el0",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}
