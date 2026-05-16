use core::arch::asm;
use core::arch::x86_64::_rdtsc;

#[inline(always)]
pub fn rdtsc() -> u64 {
  // SAFETY: `_rdtsc` emits the CPU counter read instruction and has no Rust memory safety
  // preconditions.
  unsafe { _rdtsc() }
}

/// Ordered RDTSC: `lfence` serializes prior loads so the timestamp cannot be
/// sampled before an `Acquire`-or-stronger observation that precedes it.
///
/// On Intel CPUs `lfence` is fully serializing since SSE2. On AMD, `lfence`
/// is serializing when the OS sets `DE_CFG[1]` (Linux does so by default for
/// Spectre v1 mitigation). On older AMD without that bit, the fence may not
/// serialize — callers concerned about that path should use `mfence` manually.
///
/// `nomem` is intentionally omitted: the CPU barrier orders execution, but
/// the compiler also needs to keep surrounding memory operations in order
/// around the read. With `nomem` the optimizer would be free to hoist a
/// prior `Acquire` load past the asm, defeating the contract.
#[inline(always)]
pub fn rdtsc_ordered() -> u64 {
  let lo: u32;
  let hi: u32;
  // SAFETY: `lfence; rdtsc` only writes EDX:EAX. No stack access; flags
  // preserved. Compiler must treat as memory-touching so surrounding loads
  // aren't reordered across it.
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
