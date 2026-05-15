use core::arch::x86::_rdtsc;

#[inline(always)]
pub fn rdtsc() -> u64 {
  // SAFETY: `_rdtsc` emits the CPU counter read instruction and has no Rust memory safety
  // preconditions.
  unsafe { _rdtsc() }
}
