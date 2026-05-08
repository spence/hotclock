use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering, fence};
use std::time::Instant as StdInstant;

use crate::arch::{self, fallback};

#[cfg(target_arch = "aarch64")]
use crate::arch::aarch64;
#[cfg(target_arch = "loongarch64")]
use crate::arch::loongarch64;
#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_os = "linux"))]
use crate::arch::perf_rdpmc_linux;
#[cfg(target_arch = "powerpc64")]
use crate::arch::powerpc64;
#[cfg(target_arch = "riscv64")]
use crate::arch::riscv64;
#[cfg(all(target_arch = "s390x", not(unix)))]
use crate::arch::s390x;
#[cfg(target_arch = "x86")]
use crate::arch::x86;
#[cfg(target_arch = "x86_64")]
use crate::arch::x86_64;

const VALIDATION_ATTEMPTS: usize = 3;
const TRACE_SELECTION_ENV: &str = "TACH_SELECTOR_TRACE";

#[derive(Clone, Copy)]
struct Candidate {
  name: &'static str,
  index: u8,
  prepare: fn() -> bool,
  counter: fn() -> u64,
}

#[derive(Clone, Copy)]
struct ValidationResult {
  works: bool,
  local_monotonic: bool,
  cross_thread_ordering: Option<bool>,
}

#[derive(Clone, Copy)]
struct ValidationReport {
  attempts: usize,
  result: ValidationResult,
}

impl Candidate {
  const fn new(name: &'static str, index: u8, counter: fn() -> u64) -> Self {
    Self { name, index, prepare: always, counter }
  }

  #[allow(dead_code)]
  const fn prepared(
    name: &'static str,
    index: u8,
    prepare: fn() -> bool,
    counter: fn() -> u64,
  ) -> Self {
    Self { name, index, prepare, counter }
  }
}

macro_rules! candidates {
  ($($candidate:expr),+ $(,)?) => {{
    const CANDIDATES: &[Candidate] = &[$($candidate),+];
    CANDIDATES
  }};
}

fn always() -> bool {
  true
}

#[cfg(target_arch = "x86_64")]
fn candidates() -> &'static [Candidate] {
  #[cfg(target_os = "macos")]
  {
    candidates![
      Candidate::new("x86_64-rdtsc", arch::indices::RDTSC, x86_64::rdtsc),
      Candidate::new("macos-mach", arch::indices::MACH_TIME, fallback::mach_time),
    ]
  }
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    candidates![
      Candidate::new("x86_64-rdtsc", arch::indices::RDTSC, x86_64::rdtsc),
      Candidate::new("unix-monotonic", arch::indices::CLOCK_MONOTONIC, fallback::clock_monotonic),
    ]
  }
  #[cfg(not(unix))]
  {
    candidates![
      Candidate::new("x86_64-rdtsc", arch::indices::RDTSC, x86_64::rdtsc),
      Candidate::new("std-instant", arch::indices::STD_INSTANT, fallback::instant_elapsed),
    ]
  }
}

#[cfg(target_arch = "x86_64")]
fn cycle_candidates() -> &'static [Candidate] {
  #[cfg(all(target_os = "linux"))]
  {
    candidates![
      Candidate::prepared(
        "x86_64-direct-rdpmc",
        arch::indices::DIRECT_RDPMC,
        perf_rdpmc_linux::direct_rdpmc_fixed_core_cycles_available,
        perf_rdpmc_linux::direct_rdpmc_fixed_core_cycles,
      ),
      Candidate::prepared(
        "x86_64-perf-rdpmc",
        arch::indices::PERF_RDPMC,
        perf_rdpmc_linux::perf_rdpmc_cpu_cycles_available,
        perf_rdpmc_linux::perf_rdpmc_cpu_cycles_checked,
      ),
      Candidate::new("x86_64-rdtsc", arch::indices::RDTSC, x86_64::rdtsc),
      Candidate::new("unix-monotonic", arch::indices::CLOCK_MONOTONIC, fallback::clock_monotonic),
    ]
  }
  #[cfg(not(target_os = "linux"))]
  {
    candidates()
  }
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn candidates() -> &'static [Candidate] {
  candidates![
    Candidate::new("aarch64-cntvct", arch::indices::CNTVCT, aarch64::cntvct),
    Candidate::new("unix-monotonic", arch::indices::CLOCK_MONOTONIC, fallback::clock_monotonic),
  ]
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn cycle_candidates() -> &'static [Candidate] {
  candidates![
    Candidate::prepared(
      "aarch64-pmccntr",
      arch::indices::PMCCNTR,
      aarch64_pmccntr_user_access_enabled,
      aarch64::pmccntr_el0,
    ),
    Candidate::new("aarch64-cntvct", arch::indices::CNTVCT, aarch64::cntvct),
    Candidate::new("unix-monotonic", arch::indices::CLOCK_MONOTONIC, fallback::clock_monotonic),
  ]
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn aarch64_pmccntr_user_access_enabled() -> bool {
  let enabled = std::fs::read_to_string("/proc/sys/kernel/perf_user_access")
    .ok()
    .and_then(|value| value.trim().parse::<u64>().ok())
    .is_some_and(|value| value > 0);

  enabled && test_works(aarch64::pmccntr_el0)
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
fn candidates() -> &'static [Candidate] {
  candidates![
    Candidate::new("aarch64-cntvct", arch::indices::CNTVCT, aarch64::cntvct),
    Candidate::new("macos-mach", arch::indices::MACH_TIME, fallback::mach_time),
  ]
}

#[cfg(all(target_arch = "aarch64", unix, not(any(target_os = "linux", target_os = "macos"))))]
fn candidates() -> &'static [Candidate] {
  candidates![
    Candidate::new("aarch64-cntvct", arch::indices::CNTVCT, aarch64::cntvct),
    Candidate::new("unix-monotonic", arch::indices::CLOCK_MONOTONIC, fallback::clock_monotonic),
  ]
}

#[cfg(all(target_arch = "aarch64", not(unix), not(target_os = "linux")))]
fn candidates() -> &'static [Candidate] {
  candidates![
    Candidate::new("aarch64-cntvct", arch::indices::CNTVCT, aarch64::cntvct),
    Candidate::new("std-instant", arch::indices::STD_INSTANT, fallback::instant_elapsed),
  ]
}

#[cfg(all(target_arch = "aarch64", not(target_os = "linux")))]
fn cycle_candidates() -> &'static [Candidate] {
  candidates()
}

#[cfg(target_arch = "x86")]
fn candidates() -> &'static [Candidate] {
  #[cfg(target_os = "macos")]
  {
    candidates![
      Candidate::new("x86-rdtsc", arch::indices::RDTSC, x86::rdtsc),
      Candidate::new("macos-mach", arch::indices::MACH_TIME, fallback::mach_time),
    ]
  }
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    candidates![
      Candidate::new("x86-rdtsc", arch::indices::RDTSC, x86::rdtsc),
      Candidate::new("unix-monotonic", arch::indices::CLOCK_MONOTONIC, fallback::clock_monotonic),
    ]
  }
  #[cfg(not(unix))]
  {
    candidates![
      Candidate::new("x86-rdtsc", arch::indices::RDTSC, x86::rdtsc),
      Candidate::new("std-instant", arch::indices::STD_INSTANT, fallback::instant_elapsed),
    ]
  }
}

#[cfg(target_arch = "x86")]
fn cycle_candidates() -> &'static [Candidate] {
  #[cfg(target_os = "linux")]
  {
    candidates![
      Candidate::prepared(
        "x86-direct-rdpmc",
        arch::indices::DIRECT_RDPMC,
        perf_rdpmc_linux::direct_rdpmc_fixed_core_cycles_available,
        perf_rdpmc_linux::direct_rdpmc_fixed_core_cycles,
      ),
      Candidate::prepared(
        "x86-perf-rdpmc",
        arch::indices::PERF_RDPMC,
        perf_rdpmc_linux::perf_rdpmc_cpu_cycles_available,
        perf_rdpmc_linux::perf_rdpmc_cpu_cycles_checked,
      ),
      Candidate::new("x86-rdtsc", arch::indices::RDTSC, x86::rdtsc),
      Candidate::new("unix-monotonic", arch::indices::CLOCK_MONOTONIC, fallback::clock_monotonic),
    ]
  }
  #[cfg(not(target_os = "linux"))]
  {
    candidates()
  }
}

#[cfg(target_arch = "riscv64")]
fn candidates() -> &'static [Candidate] {
  #[cfg(unix)]
  {
    candidates![
      Candidate::new("riscv64-rdtime", arch::indices::RDTIME, riscv64::rdtime),
      Candidate::new("unix-monotonic", arch::indices::CLOCK_MONOTONIC, fallback::clock_monotonic),
    ]
  }
  #[cfg(not(unix))]
  {
    candidates![
      Candidate::new("riscv64-rdtime", arch::indices::RDTIME, riscv64::rdtime),
      Candidate::new("std-instant", arch::indices::STD_INSTANT, fallback::instant_elapsed),
    ]
  }
}

#[cfg(target_arch = "riscv64")]
fn cycle_candidates() -> &'static [Candidate] {
  #[cfg(unix)]
  {
    candidates![
      Candidate::new("riscv64-rdcycle", arch::indices::RDCYCLE, riscv64::rdcycle),
      Candidate::new("riscv64-rdtime", arch::indices::RDTIME, riscv64::rdtime),
      Candidate::new("unix-monotonic", arch::indices::CLOCK_MONOTONIC, fallback::clock_monotonic),
    ]
  }
  #[cfg(not(unix))]
  {
    candidates![
      Candidate::new("riscv64-rdcycle", arch::indices::RDCYCLE, riscv64::rdcycle),
      Candidate::new("riscv64-rdtime", arch::indices::RDTIME, riscv64::rdtime),
      Candidate::new("std-instant", arch::indices::STD_INSTANT, fallback::instant_elapsed),
    ]
  }
}

#[cfg(target_arch = "powerpc64")]
fn candidates() -> &'static [Candidate] {
  #[cfg(unix)]
  {
    candidates![
      Candidate::new("powerpc64-mftb", arch::indices::MFTB, powerpc64::mftb),
      Candidate::new("unix-monotonic", arch::indices::CLOCK_MONOTONIC, fallback::clock_monotonic),
    ]
  }
  #[cfg(not(unix))]
  {
    candidates![
      Candidate::new("powerpc64-mftb", arch::indices::MFTB, powerpc64::mftb),
      Candidate::new("std-instant", arch::indices::STD_INSTANT, fallback::instant_elapsed),
    ]
  }
}

#[cfg(target_arch = "powerpc64")]
fn cycle_candidates() -> &'static [Candidate] {
  candidates()
}

#[cfg(target_arch = "s390x")]
fn candidates() -> &'static [Candidate] {
  #[cfg(unix)]
  {
    candidates![Candidate::new(
      "unix-monotonic",
      arch::indices::CLOCK_MONOTONIC,
      fallback::clock_monotonic,
    )]
  }
  #[cfg(not(unix))]
  {
    candidates![
      Candidate::new("s390x-stckf", arch::indices::STCKF, s390x::stckf),
      Candidate::new("std-instant", arch::indices::STD_INSTANT, fallback::instant_elapsed),
    ]
  }
}

#[cfg(target_arch = "s390x")]
fn cycle_candidates() -> &'static [Candidate] {
  candidates()
}

#[cfg(target_arch = "loongarch64")]
fn candidates() -> &'static [Candidate] {
  #[cfg(unix)]
  {
    candidates![
      Candidate::new("loongarch64-rdtime", arch::indices::RDTIME, loongarch64::rdtime),
      Candidate::new("unix-monotonic", arch::indices::CLOCK_MONOTONIC, fallback::clock_monotonic),
    ]
  }
  #[cfg(not(unix))]
  {
    candidates![
      Candidate::new("loongarch64-rdtime", arch::indices::RDTIME, loongarch64::rdtime),
      Candidate::new("std-instant", arch::indices::STD_INSTANT, fallback::instant_elapsed),
    ]
  }
}

#[cfg(target_arch = "loongarch64")]
fn cycle_candidates() -> &'static [Candidate] {
  candidates()
}

#[cfg(not(any(
  target_arch = "x86_64",
  target_arch = "x86",
  target_arch = "aarch64",
  target_arch = "riscv64",
  target_arch = "powerpc64",
  target_arch = "s390x",
  target_arch = "loongarch64",
)))]
fn candidates() -> &'static [Candidate] {
  #[cfg(target_os = "macos")]
  {
    candidates![Candidate::new("macos-mach", arch::indices::MACH_TIME, fallback::mach_time)]
  }
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    candidates![Candidate::new(
      "unix-monotonic",
      arch::indices::CLOCK_MONOTONIC,
      fallback::clock_monotonic,
    )]
  }
  #[cfg(not(unix))]
  {
    candidates![Candidate::new(
      "std-instant",
      arch::indices::STD_INSTANT,
      fallback::instant_elapsed
    )]
  }
}

#[cfg(not(any(
  target_arch = "x86_64",
  target_arch = "x86",
  target_arch = "aarch64",
  target_arch = "riscv64",
  target_arch = "powerpc64",
  target_arch = "s390x",
  target_arch = "loongarch64",
)))]
fn cycle_candidates() -> &'static [Candidate] {
  candidates()
}

fn test_works(counter: fn() -> u64) -> bool {
  catch_unwind(AssertUnwindSafe(|| {
    let _ = counter();
    let _ = counter();
  }))
  .is_ok()
}

fn test_local_monotonic(counter: fn() -> u64) -> bool {
  const CALLS: usize = 1000;

  catch_unwind(AssertUnwindSafe(|| {
    let mut previous = counter();
    let mut advanced = false;

    for _ in 0..CALLS {
      let current = counter();
      if current < previous {
        return false;
      }
      advanced |= current > previous;
      previous = current;
    }

    advanced
  }))
  .unwrap_or(false)
}

fn test_cross_thread_ordering(counter: fn() -> u64) -> bool {
  const CALLS: usize = 1000;

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
        while seen < CALLS {
          let next = sequence.load(Ordering::Acquire);
          if next == seen {
            std::hint::spin_loop();
            continue;
          }

          let before = published.load(Ordering::Acquire);
          fence(Ordering::SeqCst);
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

    for i in 1..=CALLS {
      if failed.load(Ordering::Relaxed) {
        break;
      }

      published.store(counter(), Ordering::Release);
      sequence.store(i, Ordering::Release);

      while acknowledged.load(Ordering::Acquire) != i {
        if failed.load(Ordering::Relaxed) {
          break;
        }
        if reader.is_finished() {
          failed.store(true, Ordering::Relaxed);
          break;
        }
        std::hint::spin_loop();
      }
    }

    reader.join().is_ok() && !failed.load(Ordering::Relaxed)
  }))
  .unwrap_or(false)
}

fn validate_once(counter: fn() -> u64, cross_thread: bool) -> ValidationResult {
  let works = test_works(counter);
  let local_monotonic = works && test_local_monotonic(counter);
  let cross_thread_ordering =
    if cross_thread && local_monotonic { Some(test_cross_thread_ordering(counter)) } else { None };

  ValidationResult { works, local_monotonic, cross_thread_ordering }
}

fn validate_with_retries(counter: fn() -> u64, cross_thread: bool) -> ValidationReport {
  let mut last = None;

  for attempts in 1..=VALIDATION_ATTEMPTS {
    let result = validate_once(counter, cross_thread);
    last = Some(result);
    if result.passed() {
      return ValidationReport { attempts, result };
    }
  }

  ValidationReport {
    attempts: VALIDATION_ATTEMPTS,
    result: last.expect("validation must run at least one attempt"),
  }
}

impl ValidationReport {
  fn passed(self) -> bool {
    self.result.passed()
  }
}

impl ValidationResult {
  fn passed(self) -> bool {
    self.works && self.local_monotonic && self.cross_thread_ordering.unwrap_or(true)
  }

  fn failed_stage(self) -> &'static str {
    if !self.works {
      "works"
    } else if !self.local_monotonic {
      "local-monotonic"
    } else if self.cross_thread_ordering == Some(false) {
      "cross-thread-ordering"
    } else {
      "none"
    }
  }
}

fn measure_latency(counter: fn() -> u64) -> Option<u128> {
  const WARMUP_CALLS: usize = 1000;
  const MEASURE_CALLS: usize = 100_000;
  const SAMPLES: usize = 11;

  catch_unwind(AssertUnwindSafe(|| {
    for _ in 0..WARMUP_CALLS {
      std::hint::black_box(counter());
    }

    let mut best = u128::MAX;
    for _ in 0..SAMPLES {
      let start = StdInstant::now();
      for _ in 0..MEASURE_CALLS {
        std::hint::black_box(counter());
      }
      let elapsed = start.elapsed().as_nanos();
      best = best.min(elapsed);
    }
    best
  }))
  .ok()
}

fn select_fastest(
  class: &'static str,
  candidates: &'static [Candidate],
  cross_thread: bool,
) -> (u8, &'static str) {
  let mut best: Option<(&Candidate, u128, ValidationReport)> = None;
  let trace = trace_selection();

  for candidate in candidates {
    let prepared = catch_unwind(AssertUnwindSafe(|| (candidate.prepare)())).unwrap_or(false);
    if !prepared {
      trace_candidate(class, candidate, prepared, None, None, false, trace);
      continue;
    }

    let validation = validate_with_retries(candidate.counter, cross_thread);
    if !validation.passed() {
      trace_candidate(class, candidate, prepared, Some(validation), None, false, trace);
      continue;
    }

    let Some(latency) = measure_latency(candidate.counter) else {
      trace_candidate(class, candidate, prepared, Some(validation), None, false, trace);
      continue;
    };

    match best {
      None => best = Some((candidate, latency, validation)),
      Some((_, best_latency, _)) if latency < best_latency => {
        best = Some((candidate, latency, validation))
      }
      _ => {}
    }

    trace_candidate(class, candidate, prepared, Some(validation), Some(latency), false, trace);
  }

  match best {
    Some((candidate, latency, validation)) => {
      trace_candidate(class, candidate, true, Some(validation), Some(latency), true, trace);
      (candidate.index, candidate.name)
    }
    None => {
      let fallback = candidates
        .last()
        .expect("tach selector must have at least one fallback candidate");
      trace_candidate(class, fallback, true, None, None, true, trace);
      (fallback.index, fallback.name)
    }
  }
}

pub fn select_best() -> (u8, &'static str) {
  select_fastest("instant", candidates(), true)
}

pub fn select_best_cycles() -> (u8, &'static str) {
  select_fastest("cycles", cycle_candidates(), false)
}

fn trace_selection() -> bool {
  std::env::var(TRACE_SELECTION_ENV).as_deref() == Ok("1")
}

fn trace_candidate(
  class: &str,
  candidate: &Candidate,
  prepared: bool,
  validation: Option<ValidationReport>,
  latency: Option<u128>,
  selected: bool,
  trace: bool,
) {
  if !trace {
    return;
  }

  let (attempts, passed, failed_stage) = match validation {
    Some(report) => (report.attempts, report.result.passed(), report.result.failed_stage()),
    None => (0, false, if prepared { "not-measured" } else { "prepare" }),
  };
  let latency = latency.map_or_else(|| "none".to_owned(), |value| format!("{value} ns"));

  eprintln!(
    "tach selector class={class} candidate={} prepared={prepared} attempts={attempts} \
     valid={passed} failed_stage={failed_stage} latency={latency} selected={selected}",
    candidate.name,
  );
}

#[cfg(test)]
mod tests {
  use std::sync::atomic::{AtomicUsize, Ordering};

  use super::validate_with_retries;

  static FLAKY_COUNTER_CALLS: AtomicUsize = AtomicUsize::new(0);

  fn flaky_counter() -> u64 {
    let call = FLAKY_COUNTER_CALLS.fetch_add(1, Ordering::Relaxed);
    if call == 4 { 0 } else { call as u64 + 1 }
  }

  #[test]
  fn validation_retries_transient_local_monotonic_failure() {
    FLAKY_COUNTER_CALLS.store(0, Ordering::Relaxed);

    let report = validate_with_retries(flaky_counter, false);

    assert!(report.passed());
    assert_eq!(report.attempts, 2);
  }
}
