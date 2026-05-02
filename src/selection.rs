use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use crate::arch::{self, fallback};

#[cfg(target_arch = "aarch64")]
use crate::arch::aarch64;
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

      let before = counter();
      published.store(before, Ordering::Release);
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

  for i in 0..CALLS {
    if times[i] > times[i + 1] {
      return None;
    }
  }

  if !test_cross_thread_ordering(counter) {
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
    None => panic!("hotclock: no working counter found"),
  }
}
