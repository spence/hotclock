use crate::arch::{self, fallback};
use crate::counter_eval::{CounterScore, score_counter, score_is_better};

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

pub fn select_best() -> (u8, &'static str) {
  let candidates = candidates();
  let mut best: Option<(&Candidate, CounterScore)> = None;

  for candidate in candidates {
    if let Some(score) = score_counter(candidate.counter) {
      match best {
        None => best = Some((candidate, score)),
        Some((_, best_score)) if score_is_better(score, best_score) => {
          best = Some((candidate, score))
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
