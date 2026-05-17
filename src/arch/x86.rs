use core::arch::asm;
use core::arch::x86::{__cpuid, _rdtsc};

#[inline(always)]
pub fn rdtsc() -> u64 {
  // SAFETY: `_rdtsc` emits the CPU counter read instruction and has no Rust memory safety
  // preconditions.
  unsafe { _rdtsc() }
}

/// Read the architectural TSC frequency from CPUID leaf 15h. See
/// `x86_64::cpuid_tsc_hz` for the formula and supported-CPU notes.
#[allow(dead_code)] // unused on macOS/Windows where the OS API is authoritative
pub fn cpuid_tsc_hz() -> Option<u64> {
  let basic = __cpuid(0);
  if basic.eax < 0x15 {
    return None;
  }
  let leaf = __cpuid(0x15);
  if leaf.eax == 0 || leaf.ebx == 0 || leaf.ecx == 0 {
    return None;
  }
  Some(u64::from(leaf.ecx) * u64::from(leaf.ebx) / u64::from(leaf.eax))
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
