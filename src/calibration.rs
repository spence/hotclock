//! Calibration runs only on cfg(unix) non-macOS non-aarch64 — i.e. Linux x86,
//! x86_64, riscv64, loongarch64 — to measure the architectural counter's tick
//! rate against `clock_gettime(CLOCK_MONOTONIC)`. aarch64 reads `cntfrq_el0`,
//! macOS reads `mach_timebase_info`, Windows reads `QueryPerformanceFrequency`,
//! wasm32-host / WASI are fixed at 1 GHz.

use core::hint::spin_loop;

use crate::arch::fallback::clock_monotonic;

pub fn calibrate_frequency() -> u64 {
  const CALIBRATION_TIME_NS: u64 = 10_000_000;
  const NUM_SAMPLES: usize = 5;

  let mut estimates = [0u64; NUM_SAMPLES];

  for estimate in &mut estimates {
    let wall_start = clock_monotonic();
    let t0 = crate::arch::ticks();

    while clock_monotonic().wrapping_sub(wall_start) < CALIBRATION_TIME_NS {
      spin_loop();
    }

    let t1 = crate::arch::ticks();
    let wall_elapsed = clock_monotonic().wrapping_sub(wall_start);

    let ticks = t1.wrapping_sub(t0);
    if let Some(hz) = (u128::from(ticks) * 1_000_000_000).checked_div(u128::from(wall_elapsed)) {
      *estimate = u64::try_from(hz).unwrap_or(u64::MAX);
    }
  }

  estimates.sort_unstable();
  estimates[NUM_SAMPLES / 2]
}
