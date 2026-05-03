use std::hint::black_box;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Instant as StdInstant;

const PRECISION_CALLS: usize = 2_000;
const CROSS_THREAD_CALLS: usize = 10_000;
const VALIDATION_PASSES: usize = 5;
const LATENCY_WARMUP_ITERS: usize = 5_000;
const LATENCY_MEASURE_ITERS: usize = 50_000;
const LATENCY_SAMPLES: usize = 7;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CounterValidation {
  pub precision_ticks: u64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CounterLatency {
  pub best_total_nanos: u128,
  pub median_total_nanos: u128,
  pub worst_total_nanos: u128,
  pub iterations: u64,
}

impl CounterLatency {
  #[cfg(feature = "bench-internals")]
  pub(crate) fn best_ns_per_call(self) -> f64 {
    self.best_total_nanos as f64 / self.iterations as f64
  }

  #[cfg(feature = "bench-internals")]
  pub(crate) fn median_ns_per_call(self) -> f64 {
    self.median_total_nanos as f64 / self.iterations as f64
  }

  #[cfg(feature = "bench-internals")]
  pub(crate) fn worst_ns_per_call(self) -> f64 {
    self.worst_total_nanos as f64 / self.iterations as f64
  }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CounterScore {
  pub validation: CounterValidation,
  pub latency: CounterLatency,
}

pub(crate) fn score_counter(counter: fn() -> u64) -> Option<CounterScore> {
  let validation = validate_counter_stable(counter)?;
  let latency = measure_counter_latency(counter);
  Some(CounterScore { validation, latency })
}

#[allow(dead_code)]
pub(crate) fn score_is_better(candidate: CounterScore, current: CounterScore) -> bool {
  (
    candidate.latency.median_total_nanos,
    candidate.latency.best_total_nanos,
    candidate.validation.precision_ticks,
  ) < (
    current.latency.median_total_nanos,
    current.latency.best_total_nanos,
    current.validation.precision_ticks,
  )
}

fn validate_counter_stable(counter: fn() -> u64) -> Option<CounterValidation> {
  let mut precision_ticks = 0;
  for _ in 0..VALIDATION_PASSES {
    let validation = validate_counter(counter)?;
    precision_ticks = precision_ticks.max(validation.precision_ticks);
  }
  Some(CounterValidation { precision_ticks })
}

fn validate_counter(counter: fn() -> u64) -> Option<CounterValidation> {
  if !test_works(counter) {
    return None;
  }

  let mut times = [0u64; PRECISION_CALLS + 1];
  for t in &mut times {
    *t = counter();
  }

  if times[0] == times[PRECISION_CALLS] {
    return None;
  }

  for i in 0..PRECISION_CALLS {
    if times[i] > times[i + 1] {
      return None;
    }
  }

  if !test_cross_thread_ordering(counter) {
    return None;
  }

  let mut smallest = u64::MAX;
  for i in 0..PRECISION_CALLS {
    let diff = times[i + 1].wrapping_sub(times[i]);
    if diff > 0 && diff < smallest && diff < 1_000_000 {
      smallest = diff;
    }
  }

  if smallest == u64::MAX { None } else { Some(CounterValidation { precision_ticks: smallest }) }
}

pub(crate) fn measure_counter_latency(counter: fn() -> u64) -> CounterLatency {
  for _ in 0..LATENCY_WARMUP_ITERS {
    black_box(counter());
  }

  let mut samples = [0u128; LATENCY_SAMPLES];
  for sample in &mut samples {
    let started = StdInstant::now();
    for _ in 0..LATENCY_MEASURE_ITERS {
      black_box(counter());
    }
    *sample = started.elapsed().as_nanos();
  }

  samples.sort_unstable();
  CounterLatency {
    best_total_nanos: samples[0],
    median_total_nanos: samples[samples.len() / 2],
    worst_total_nanos: samples[samples.len() - 1],
    iterations: LATENCY_MEASURE_ITERS as u64,
  }
}

fn test_works(counter: fn() -> u64) -> bool {
  catch_unwind(AssertUnwindSafe(|| {
    let _ = counter();
    let _ = counter();
  }))
  .is_ok()
}

fn test_cross_thread_ordering(counter: fn() -> u64) -> bool {
  catch_unwind(AssertUnwindSafe(|| {
    let published = Arc::new(AtomicU64::new(0));
    let sequence = Arc::new(AtomicUsize::new(0));
    let acknowledged = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(AtomicBool::new(false));
    let start = Arc::new(Barrier::new(2));

    let reader = {
      let published = Arc::clone(&published);
      let sequence = Arc::clone(&sequence);
      let acknowledged = Arc::clone(&acknowledged);
      let failed = Arc::clone(&failed);
      let start = Arc::clone(&start);

      std::thread::spawn(move || {
        start.wait();

        let mut seen = 0;
        while seen < CROSS_THREAD_CALLS {
          let next = sequence.load(Ordering::Acquire);
          if next == seen {
            std::hint::spin_loop();
            continue;
          }

          let before = published.load(Ordering::Acquire);
          let after = counter();
          seen = next;
          if after < before {
            failed.store(true, Ordering::Relaxed);
            acknowledged.store(seen, Ordering::Release);
            break;
          }

          acknowledged.store(seen, Ordering::Release);
        }
      })
    };

    start.wait();

    for i in 1..=CROSS_THREAD_CALLS {
      if failed.load(Ordering::Relaxed) {
        break;
      }

      published.store(counter(), Ordering::Release);
      sequence.store(i, Ordering::Release);

      while acknowledged.load(Ordering::Acquire) != i {
        if failed.load(Ordering::Relaxed) {
          break;
        }
        std::hint::spin_loop();
      }
    }

    reader.join().is_ok() && !failed.load(Ordering::Relaxed)
  }))
  .unwrap_or(false)
}
