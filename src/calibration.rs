use std::time::{Duration, Instant};

pub fn calibrate_frequency() -> u64 {
  calibrate_frequency_for(crate::arch::ticks)
}

pub fn calibrate_frequency_for(counter: fn() -> u64) -> u64 {
  const CALIBRATION_TIME_MS: u64 = 10;
  const NUM_SAMPLES: usize = 5;

  let mut estimates = [0u64; NUM_SAMPLES];

  for estimate in &mut estimates {
    let t0 = counter();
    let start = Instant::now();

    while start.elapsed() < Duration::from_millis(CALIBRATION_TIME_MS) {
      std::hint::spin_loop();
    }

    let t1 = counter();
    let elapsed = start.elapsed();

    let ticks = t1.wrapping_sub(t0);
    let nanos = elapsed.as_nanos();

    if let Some(hz) = (u128::from(ticks) * 1_000_000_000).checked_div(nanos) {
      *estimate = u64::try_from(hz).unwrap_or(u64::MAX);
    }
  }

  estimates.sort_unstable();
  estimates[NUM_SAMPLES / 2]
}
