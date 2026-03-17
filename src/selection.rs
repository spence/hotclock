use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::arch::{self, fallback};

#[cfg(target_arch = "aarch64")]
use crate::arch::aarch64;
#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
use crate::arch::aarch64_linux;
#[cfg(target_arch = "loongarch64")]
use crate::arch::loongarch64;
#[cfg(target_arch = "powerpc64")]
use crate::arch::powerpc64;
#[cfg(target_arch = "riscv64")]
use crate::arch::riscv64;
#[cfg(target_arch = "s390x")]
use crate::arch::s390x;
#[cfg(target_arch = "x86")]
use crate::arch::x86;
#[cfg(target_arch = "x86_64")]
use crate::arch::x86_64;

#[derive(Clone, Copy)]
struct Candidate {
  name: &'static str,
  index: u8,
  counter: fn() -> u64,
}

#[cfg(target_arch = "x86_64")]
fn candidates() -> &'static [Candidate] {
  #[cfg(target_os = "macos")]
  {
    &[
      Candidate { name: "x86_64-rdtsc", index: arch::indices::RDTSC, counter: x86_64::rdtsc },
      Candidate {
        name: "macos-mach",
        index: arch::indices::MACH_TIME,
        counter: fallback::mach_time,
      },
    ]
  }
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    &[
      Candidate { name: "x86_64-rdtsc", index: arch::indices::RDTSC, counter: x86_64::rdtsc },
      Candidate {
        name: "unix-monotonic",
        index: arch::indices::CLOCK_MONOTONIC,
        counter: fallback::clock_monotonic,
      },
    ]
  }
  #[cfg(not(unix))]
  {
    &[
      Candidate { name: "x86_64-rdtsc", index: arch::indices::RDTSC, counter: x86_64::rdtsc },
      Candidate {
        name: "std-instant",
        index: arch::indices::STD_INSTANT,
        counter: fallback::instant_elapsed,
      },
    ]
  }
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn candidates() -> &'static [Candidate] {
  &[
    Candidate {
      name: "aarch64-pmccntr",
      index: arch::indices::PMCCNTR,
      counter: aarch64_linux::pmccntr,
    },
    Candidate { name: "aarch64-cntvct", index: arch::indices::CNTVCT, counter: aarch64::cntvct },
    Candidate {
      name: "unix-monotonic",
      index: arch::indices::CLOCK_MONOTONIC,
      counter: fallback::clock_monotonic,
    },
  ]
}

#[cfg(all(target_arch = "aarch64", not(target_os = "linux")))]
fn candidates() -> &'static [Candidate] {
  #[cfg(target_os = "macos")]
  {
    &[
      Candidate { name: "aarch64-cntvct", index: arch::indices::CNTVCT, counter: aarch64::cntvct },
      Candidate {
        name: "macos-mach",
        index: arch::indices::MACH_TIME,
        counter: fallback::mach_time,
      },
    ]
  }
  #[cfg(not(target_os = "macos"))]
  {
    &[
      Candidate { name: "aarch64-cntvct", index: arch::indices::CNTVCT, counter: aarch64::cntvct },
      Candidate {
        name: "std-instant",
        index: arch::indices::STD_INSTANT,
        counter: fallback::instant_elapsed,
      },
    ]
  }
}

#[cfg(target_arch = "x86")]
fn candidates() -> &'static [Candidate] {
  #[cfg(target_os = "macos")]
  {
    &[
      Candidate { name: "x86-rdtsc", index: arch::indices::RDTSC, counter: x86::rdtsc },
      Candidate {
        name: "macos-mach",
        index: arch::indices::MACH_TIME,
        counter: fallback::mach_time,
      },
    ]
  }
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    &[
      Candidate { name: "x86-rdtsc", index: arch::indices::RDTSC, counter: x86::rdtsc },
      Candidate {
        name: "unix-monotonic",
        index: arch::indices::CLOCK_MONOTONIC,
        counter: fallback::clock_monotonic,
      },
    ]
  }
  #[cfg(not(unix))]
  {
    &[
      Candidate { name: "x86-rdtsc", index: arch::indices::RDTSC, counter: x86::rdtsc },
      Candidate {
        name: "std-instant",
        index: arch::indices::STD_INSTANT,
        counter: fallback::instant_elapsed,
      },
    ]
  }
}

#[cfg(target_arch = "riscv64")]
fn candidates() -> &'static [Candidate] {
  #[cfg(unix)]
  {
    &[
      Candidate {
        name: "riscv64-rdcycle",
        index: arch::indices::RDCYCLE,
        counter: riscv64::rdcycle,
      },
      Candidate {
        name: "unix-monotonic",
        index: arch::indices::CLOCK_MONOTONIC,
        counter: fallback::clock_monotonic,
      },
    ]
  }
  #[cfg(not(unix))]
  {
    &[
      Candidate {
        name: "riscv64-rdcycle",
        index: arch::indices::RDCYCLE,
        counter: riscv64::rdcycle,
      },
      Candidate {
        name: "std-instant",
        index: arch::indices::STD_INSTANT,
        counter: fallback::instant_elapsed,
      },
    ]
  }
}

#[cfg(target_arch = "powerpc64")]
fn candidates() -> &'static [Candidate] {
  #[cfg(unix)]
  {
    &[
      Candidate { name: "powerpc64-mftb", index: arch::indices::MFTB, counter: powerpc64::mftb },
      Candidate {
        name: "unix-monotonic",
        index: arch::indices::CLOCK_MONOTONIC,
        counter: fallback::clock_monotonic,
      },
    ]
  }
  #[cfg(not(unix))]
  {
    &[
      Candidate { name: "powerpc64-mftb", index: arch::indices::MFTB, counter: powerpc64::mftb },
      Candidate {
        name: "std-instant",
        index: arch::indices::STD_INSTANT,
        counter: fallback::instant_elapsed,
      },
    ]
  }
}

#[cfg(target_arch = "s390x")]
fn candidates() -> &'static [Candidate] {
  #[cfg(unix)]
  {
    &[
      Candidate { name: "s390x-stckf", index: arch::indices::STCKF, counter: s390x::stckf },
      Candidate {
        name: "unix-monotonic",
        index: arch::indices::CLOCK_MONOTONIC,
        counter: fallback::clock_monotonic,
      },
    ]
  }
  #[cfg(not(unix))]
  {
    &[
      Candidate { name: "s390x-stckf", index: arch::indices::STCKF, counter: s390x::stckf },
      Candidate {
        name: "std-instant",
        index: arch::indices::STD_INSTANT,
        counter: fallback::instant_elapsed,
      },
    ]
  }
}

#[cfg(target_arch = "loongarch64")]
fn candidates() -> &'static [Candidate] {
  #[cfg(unix)]
  {
    &[
      Candidate {
        name: "loongarch64-rdtime",
        index: arch::indices::RDTIME,
        counter: loongarch64::rdtime,
      },
      Candidate {
        name: "unix-monotonic",
        index: arch::indices::CLOCK_MONOTONIC,
        counter: fallback::clock_monotonic,
      },
    ]
  }
  #[cfg(not(unix))]
  {
    &[
      Candidate {
        name: "loongarch64-rdtime",
        index: arch::indices::RDTIME,
        counter: loongarch64::rdtime,
      },
      Candidate {
        name: "std-instant",
        index: arch::indices::STD_INSTANT,
        counter: fallback::instant_elapsed,
      },
    ]
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
fn candidates() -> &'static [Candidate] {
  #[cfg(target_os = "macos")]
  {
    &[Candidate {
      name: "macos-mach",
      index: arch::indices::MACH_TIME,
      counter: fallback::mach_time,
    }]
  }
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    &[Candidate {
      name: "unix-monotonic",
      index: arch::indices::CLOCK_MONOTONIC,
      counter: fallback::clock_monotonic,
    }]
  }
  #[cfg(not(unix))]
  {
    &[Candidate {
      name: "std-instant",
      index: arch::indices::STD_INSTANT,
      counter: fallback::instant_elapsed,
    }]
  }
}

fn test_works(counter: fn() -> u64) -> bool {
  catch_unwind(AssertUnwindSafe(|| {
    let _ = counter();
    let _ = counter();
  }))
  .is_ok()
}

fn measure_precision(counter: fn() -> u64) -> Option<u64> {
  const CALLS: usize = 1000;

  if !test_works(counter) {
    return None;
  }

  let mut times = [0u64; CALLS + 1];
  for t in &mut times {
    *t = counter();
  }

  if times[0] == times[CALLS] {
    return None;
  }

  let mut non_monotonic = 0;
  for i in 0..CALLS {
    if times[i] > times[i + 1] {
      non_monotonic += 1;
    }
  }
  if non_monotonic > CALLS / 10 {
    return None;
  }

  let mut smallest = u64::MAX;
  for i in 0..CALLS {
    let diff = times[i + 1].wrapping_sub(times[i]);
    if diff > 0 && diff < smallest && diff < 1_000_000 {
      smallest = diff;
    }
  }

  if smallest == u64::MAX { None } else { Some(smallest) }
}

pub fn select_best() -> (u8, &'static str) {
  let candidates = candidates();
  let mut best: Option<(&Candidate, u64)> = None;

  for candidate in candidates {
    if let Some(precision) = measure_precision(candidate.counter) {
      match best {
        None => best = Some((candidate, precision)),
        Some((_, best_precision)) if precision < best_precision => {
          best = Some((candidate, precision));
        }
        _ => {}
      }
    }
  }

  match best {
    Some((candidate, _)) => (candidate.index, candidate.name),
    None => panic!("cputicks: no working counter found"),
  }
}

pub fn calibrate_frequency() -> u64 {
  use std::time::{Duration, Instant};

  const CALIBRATION_TIME_MS: u64 = 10;
  const NUM_SAMPLES: usize = 5;

  let mut estimates = [0u64; NUM_SAMPLES];

  for estimate in &mut estimates {
    let t0 = arch::ticks();
    let start = Instant::now();

    while start.elapsed() < Duration::from_millis(CALIBRATION_TIME_MS) {
      std::hint::spin_loop();
    }

    let t1 = arch::ticks();
    let elapsed = start.elapsed();

    let ticks = t1.wrapping_sub(t0);
    let nanos = elapsed.as_nanos() as u64;

    if nanos > 0 {
      *estimate = (ticks as u128 * 1_000_000_000 / nanos as u128) as u64;
    }
  }

  estimates.sort();
  estimates[NUM_SAMPLES / 2]
}

#[cfg(all(target_family = "unix", not(target_os = "macos"), not(target_os = "ios")))]
#[used]
#[unsafe(link_section = ".init_array")]
static INIT: extern "C" fn() = init;

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[used]
#[unsafe(link_section = "__DATA,__mod_init_func")]
static INIT: extern "C" fn() = init;

#[cfg(target_os = "windows")]
#[used]
#[unsafe(link_section = ".CRT$XCU")]
static INIT: extern "C" fn() = init;

extern "C" fn init() {
  let (idx, name) = select_best();
  arch::set_selected(idx, name);
}
