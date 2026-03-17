use core::arch::x86::{__rdtscp, _rdtsc};

#[inline(always)]
pub fn rdtsc() -> u64 {
  unsafe { _rdtsc() as u64 }
}

#[inline(always)]
pub fn rdtscp() -> u64 {
  let mut aux: u32 = 0;
  unsafe { __rdtscp(&mut aux) }
}
