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

#[cfg(target_os = "linux")]
#[inline(always)]
#[allow(clippy::inline_always)]
pub fn pmccntr_el0() -> u64 {
  let cnt: u64;
  // SAFETY: `mrs pmccntr_el0` reads the PMU cycle counter. Callers must only install this
  // path after Linux reports userspace PMU access is enabled.
  unsafe {
    asm!(
        "mrs {}, pmccntr_el0",
        out(reg) cnt,
        options(nostack, nomem, preserves_flags)
    );
  }
  cnt
}
