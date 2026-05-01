use core::arch::x86::_rdtsc;

#[inline(always)]
pub fn rdtsc() -> u64 {
  // SAFETY: `_rdtsc` emits the CPU counter read instruction and has no Rust memory safety
  // preconditions. Runtime selection validates monotonic behavior before installing it.
  unsafe { _rdtsc() }
}
