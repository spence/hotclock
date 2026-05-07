use core::arch::x86_64::{__rdtscp, _rdtsc};

#[inline(always)]
pub fn rdtsc() -> u64 {
  // SAFETY: `_rdtsc` emits the CPU counter read instruction and has no Rust memory safety
  // preconditions. Runtime selection validates monotonic behavior before installing it.
  unsafe { _rdtsc() }
}

#[inline(always)]
#[cfg(all(feature = "bench-internals", target_os = "linux"))]
pub fn rdtscp() -> u64 {
  let mut aux = 0;
  // SAFETY: `__rdtscp` reads the TSC plus the CPU auxiliary value and has no Rust memory safety
  // preconditions. This is benchmark-only because the extra ordering is a policy choice.
  unsafe { __rdtscp(&mut aux) }
}

#[inline(always)]
#[cfg(all(feature = "bench-internals", target_os = "linux"))]
#[allow(clippy::inline_always)]
pub fn lfence_rdtsc() -> u64 {
  let low: u32;
  let high: u32;
  // SAFETY: `lfence; rdtsc` is a benchmark-only ordered TSC read. It touches no memory and
  // returns the architectural TSC split across EDX:EAX.
  unsafe {
    core::arch::asm!(
      "lfence",
      "rdtsc",
      out("eax") low,
      out("edx") high,
      options(nomem, nostack, preserves_flags)
    );
  }
  (u64::from(high) << 32) | u64::from(low)
}
