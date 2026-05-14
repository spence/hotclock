//! Single-threaded Cycles candidate selection.
//!
//! This module is only compiled on Linux targets where `Cycles` has more than one
//! candidate counter (x86 / x86_64, aarch64). On every other target,
//! `arch::cycle_ticks()` compile-time-equals `arch::ticks()` so no selection runs.
//!
//! The selector measures each available candidate in a single-threaded tight loop
//! and picks the lowest measured latency. The wall-clock fallback (RDTSC on x86_64,
//! CNTVCT_EL0 on aarch64) is always one of the candidates, guaranteeing
//! `Cycles ≤ Instant` on read cost.
//!
//! No `thread::spawn`, no `Barrier`, no cross-thread validation — the cross-thread
//! property is provided by the patchpoint mechanism committing one choice for every
//! callsite in the process.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant as StdInstant;

use crate::arch::{self};
#[cfg(target_arch = "aarch64")]
use crate::arch::aarch64;
#[cfg(target_arch = "aarch64")]
use crate::arch::perf_pmccntr_linux;
#[cfg(target_arch = "x86_64")]
use crate::arch::perf_rdpmc_linux;
#[cfg(target_arch = "x86_64")]
use crate::arch::x86_64;

const TRACE_SELECTION_ENV: &str = "TACH_SELECTOR_TRACE";

#[derive(Clone, Copy)]
struct Candidate {
  name: &'static str,
  index: u8,
  prepare: fn() -> bool,
  counter: fn() -> u64,
}

impl Candidate {
  const fn new(name: &'static str, index: u8, counter: fn() -> u64) -> Self {
    Self { name, index, prepare: always, counter }
  }

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
fn cycle_candidates() -> &'static [Candidate] {
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
    Candidate::new(
      "unix-monotonic",
      arch::indices::CLOCK_MONOTONIC,
      arch::fallback::clock_monotonic,
    ),
  ]
}

#[cfg(target_arch = "aarch64")]
fn cycle_candidates() -> &'static [Candidate] {
  candidates![
    Candidate::prepared(
      "aarch64-perf-pmccntr",
      arch::indices::PERF_PMCCNTR,
      perf_pmccntr_linux::perf_pmccntr_cpu_cycles_available,
      perf_pmccntr_linux::perf_pmccntr_cpu_cycles_checked,
    ),
    Candidate::new("aarch64-cntvct", arch::indices::CNTVCT, aarch64::cntvct),
    Candidate::new(
      "unix-monotonic",
      arch::indices::CLOCK_MONOTONIC,
      arch::fallback::clock_monotonic,
    ),
  ]
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

/// Selects the fastest available Cycles candidate.
///
/// Detection is single-threaded and side-effect-free at the OS level (sysfs/sysctl
/// reads plus `perf_event_open` + mmap probes). Latency measurement is a tight
/// single-thread loop. The wall-clock fallback (last candidate) is always available,
/// so this function always returns a valid `(index, name)` pair.
pub fn select_best_cycles() -> (u8, &'static str) {
  let trace = trace_selection();
  let candidates = cycle_candidates();
  let mut best: Option<(&Candidate, u128)> = None;

  for candidate in candidates {
    let prepared = catch_unwind(AssertUnwindSafe(|| (candidate.prepare)())).unwrap_or(false);
    if !prepared {
      trace_candidate(candidate, prepared, false, None, false, trace);
      continue;
    }

    let works = test_works(candidate.counter);
    if !works {
      trace_candidate(candidate, prepared, false, None, false, trace);
      continue;
    }

    let local_monotonic = test_local_monotonic(candidate.counter);
    if !local_monotonic {
      trace_candidate(candidate, prepared, false, None, false, trace);
      continue;
    }

    let Some(latency) = measure_latency(candidate.counter) else {
      trace_candidate(candidate, prepared, true, None, false, trace);
      continue;
    };

    match best {
      None => best = Some((candidate, latency)),
      Some((_, best_latency)) if latency < best_latency => best = Some((candidate, latency)),
      _ => {}
    }

    trace_candidate(candidate, prepared, true, Some(latency), false, trace);
  }

  match best {
    Some((candidate, latency)) => {
      trace_candidate(candidate, true, true, Some(latency), true, trace);
      (candidate.index, candidate.name)
    }
    None => {
      let fallback = candidates
        .last()
        .expect("tach Cycles selector must have at least one fallback candidate");
      trace_candidate(fallback, true, false, None, true, trace);
      (fallback.index, fallback.name)
    }
  }
}

fn trace_selection() -> bool {
  std::env::var(TRACE_SELECTION_ENV).as_deref() == Ok("1")
}

fn trace_candidate(
  candidate: &Candidate,
  prepared: bool,
  valid: bool,
  latency: Option<u128>,
  selected: bool,
  trace: bool,
) {
  if !trace {
    return;
  }
  let latency = latency.map_or_else(|| "none".to_owned(), |value| format!("{value} ns"));
  eprintln!(
    "tach selector class=cycles candidate={} prepared={prepared} valid={valid} \
     latency={latency} selected={selected}",
    candidate.name,
  );
}
