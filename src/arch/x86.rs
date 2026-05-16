use core::arch::asm;
use core::arch::x86::_rdtsc;

#[inline(always)]
pub fn rdtsc() -> u64 {
  // SAFETY: `_rdtsc` emits the CPU counter read instruction and has no Rust memory safety
  // preconditions.
  unsafe { _rdtsc() }
}

/// Ordered RDTSC: `lfence` serializes prior loads so the timestamp cannot be
/// sampled before an `Acquire`-or-stronger observation that precedes it.
/// See `x86_64::rdtsc_ordered` for the AMD caveat and the `nomem` rationale.
#[inline(always)]
pub fn rdtsc_ordered() -> u64 {
  let lo: u32;
  let hi: u32;
  // SAFETY: `lfence; rdtsc` only writes EDX:EAX. Compiler must treat as
  // memory-touching so surrounding loads aren't reordered across it.
  unsafe {
    asm!(
      "lfence",
      "rdtsc",
      out("eax") lo,
      out("edx") hi,
      options(nostack, preserves_flags),
    );
  }
  (u64::from(hi) << 32) | u64::from(lo)
}
