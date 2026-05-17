//! Calibration runs on x86 / x86_64 (Linux, other Unixes, Windows) and on
//! riscv64 / loongarch64 — every target whose architectural counter doesn't
//! self-report its frequency. It measures the counter's tick rate against a
//! platform monotonic clock: `clock_gettime(CLOCK_MONOTONIC)` on Unix,
//! `QueryPerformanceCounter` on Windows. aarch64 reads `cntfrq_el0`, macOS
//! reads `mach_timebase_info`, wasm32-host / WASI are fixed at 1 GHz.

use core::hint::spin_loop;

#[inline]
fn ref_ns() -> u64 {
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    crate::arch::fallback::clock_monotonic()
  }
  #[cfg(target_os = "windows")]
  {
    crate::arch::fallback::qpc_now_ns()
  }
}

pub fn calibrate_frequency() -> u64 {
  const CALIBRATION_TIME_NS: u64 = 10_000_000;
  const NUM_SAMPLES: usize = 5;

  let mut estimates = [0u64; NUM_SAMPLES];

  for estimate in &mut estimates {
    let wall_start = ref_ns();
    let t0 = crate::arch::ticks();

    while ref_ns().wrapping_sub(wall_start) < CALIBRATION_TIME_NS {
      spin_loop();
    }

    let t1 = crate::arch::ticks();
    let wall_elapsed = ref_ns().wrapping_sub(wall_start);

    let ticks = t1.wrapping_sub(t0);
    if let Some(hz) = (u128::from(ticks) * 1_000_000_000).checked_div(u128::from(wall_elapsed)) {
      *estimate = u64::try_from(hz).unwrap_or(u64::MAX);
    }
  }

  estimates.sort_unstable();
  estimates[NUM_SAMPLES / 2]
}
