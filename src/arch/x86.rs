use core::arch::x86::_rdtsc;

#[cfg(all(feature = "bench-internals", target_os = "linux"))]
const FIXED_CORE_CYCLES: u32 = (1 << 30) | 1;

#[inline(always)]
pub fn rdtsc() -> u64 {
  // SAFETY: `_rdtsc` emits the CPU counter read instruction and has no Rust memory safety
  // preconditions. Runtime selection validates monotonic behavior before installing it.
  unsafe { _rdtsc() }
}

#[inline(always)]
#[cfg(all(feature = "bench-internals", target_os = "linux"))]
#[allow(clippy::inline_always)]
pub fn rdpmc_fixed_core_cycles() -> u64 {
  let low: u32;
  let high: u32;
  // SAFETY: this is benchmark-only direct RDPMC. The caller runs it in a crash-isolated child
  // because kernels that do not enable userspace RDPMC will fault the process.
  unsafe {
    core::arch::asm!(
      "rdpmc",
      in("ecx") FIXED_CORE_CYCLES,
      out("eax") low,
      out("edx") high,
      options(nomem, nostack, preserves_flags)
    );
  }
  (u64::from(high) << 32) | u64::from(low)
}
