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

// Calibrate by spinning for `CALIBRATION_TIME_NS` and observing how many
// architectural ticks elapse against the platform monotonic clock. A 100 ms
// window is long enough that per-tick wall-clock noise (~50 ns on bare metal,
// ~500 ns on Nitro VMs) doesn't dominate the rate estimate but short enough
// that the worst-case startup cost (7 × 100 ms = 700 ms when no preemption
// occurs) is acceptable for a one-shot lazy init.
//
// On virtualized hosts the spin loop can be preempted by the hypervisor. When
// that happens, `wall_elapsed` overshoots `CALIBRATION_TIME_NS` by orders of
// magnitude and the per-sample rate estimate is contaminated by the wall
// clock advancing while the vCPU was descheduled (counters keep ticking, but
// at the wrong relative rate to wall time over that interval). Any sample
// that overran by more than 50 % is dropped as preempted; the median of the
// survivors is returned. The 1.5× threshold catches preemption that's
// distinguishable from ordinary noise without flagging samples that merely
// ran for the full window plus normal scheduling jitter.
//
// If every sample gets discarded — e.g. on a host that consistently preempts
// every 100 ms window — the function falls back to one un-filtered sample
// rather than returning 0. Something within a few percent of the right
// answer is preferable to zero; the background-recal thread (when enabled)
// gets another chance every 60 s.
pub fn calibrate_frequency() -> u64 {
  const CALIBRATION_TIME_NS: u64 = 100_000_000;
  const MAX_OVERRUN_NS: u64 = CALIBRATION_TIME_NS * 3 / 2;
  const NUM_SAMPLES: usize = 7;

  let mut survivors = [0u64; NUM_SAMPLES];
  let mut n = 0usize;

  for _ in 0..NUM_SAMPLES {
    let wall_start = ref_ns();
    let t0 = crate::arch::ticks();

    while ref_ns().wrapping_sub(wall_start) < CALIBRATION_TIME_NS {
      spin_loop();
    }

    let t1 = crate::arch::ticks();
    let wall_elapsed = ref_ns().wrapping_sub(wall_start);

    if wall_elapsed > MAX_OVERRUN_NS {
      continue;
    }

    let ticks = t1.wrapping_sub(t0);
    if let Some(hz) = (u128::from(ticks) * 1_000_000_000).checked_div(u128::from(wall_elapsed)) {
      survivors[n] = u64::try_from(hz).unwrap_or(u64::MAX);
      n += 1;
    }
  }

  if n == 0 {
    return single_unfiltered_sample(CALIBRATION_TIME_NS);
  }

  survivors[..n].sort_unstable();
  survivors[n / 2]
}

// Fallback for the pathological case where every bracketed sample overran.
// Better to return something within a few percent than zero — recal-bg gets
// another shot every 60 s if it's enabled.
#[inline]
fn single_unfiltered_sample(window_ns: u64) -> u64 {
  let wall_start = ref_ns();
  let t0 = crate::arch::ticks();
  while ref_ns().wrapping_sub(wall_start) < window_ns {
    spin_loop();
  }
  let t1 = crate::arch::ticks();
  let wall_elapsed = ref_ns().wrapping_sub(wall_start);
  let ticks = t1.wrapping_sub(t0);
  match (u128::from(ticks) * 1_000_000_000).checked_div(u128::from(wall_elapsed)) {
    Some(hz) => u64::try_from(hz).unwrap_or(u64::MAX),
    None => 0,
  }
}
