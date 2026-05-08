#![warn(clippy::undocumented_unsafe_blocks)]
#![warn(rustdoc::broken_intra_doc_links)]

//! Cross-platform CPU cycle/tick counter with direct, runtime-selected, and patched-selected
//! paths.
//!
//! A Rust port of [libcpucycles](https://cpucycles.cr.yp.to/) that provides sub-nanosecond
//! timing by directly reading hardware counters (RDTSC, CNTVCT\_EL0, etc.). Deterministic
//! targets use a compiled-in counter path; targets with meaningful runtime variation select the
//! best available counter lazily on first use and cache it for the lifetime of the process.
//! Runtime-selected targets patch crate-owned warmed call sites where supported so hot reads do
//! not keep selected-index dispatch on the hot path.
//!
//! Roughly **~30x faster** than [`std::time::Instant`] on typical hardware.
//!
//! # Quick start
//!
//! ```
//! use hotclock::Instant;
//!
//! let start = Instant::now();
//! // ... do some work ...
//! let elapsed = start.elapsed();
//! println!("Elapsed: {:?}", elapsed);
//! ```
//!
//! # Platform support
//!
//! | Architecture        | Primary        | Fallback       |
//! |---------------------|----------------|----------------|
//! | aarch64 macOS       | CNTVCT\_EL0    | none           |
//! | x86\_64             | RDTSC          | OS timer       |
//! | x86                 | RDTSC          | OS timer       |
//! | aarch64 non-macOS   | CNTVCT\_EL0    | OS timer       |
//! | riscv64             | rdtime         | OS timer       |
//! | powerpc64           | OS timer       | none           |
//! | s390x               | OS timer       | none           |
//! | loongarch64         | rdtime.d       | OS timer       |
//! | other               | OS timer       | none           |
//!
//! OS timers: `mach_absolute_time` (macOS), `clock_gettime` (Unix), [`Instant`](std::time::Instant) (other).
//!
//! # Timing contract
//!
//! `hotclock::Instant` is a `Copy + Send + Sync` opaque `u64` wrapper around a counter
//! sample. Its raw value is not a civil-time or OS timestamp. Use it as a point in the
//! process-wide counter timeline.
//!
//! [`Instant::now()`] reads the process-wide counter. Runtime-selected targets require the
//! selected counter to be nondecreasing for repeated reads on one thread and for reads ordered
//! across threads by a release/acquire handoff. Selection falls back to the OS timer when the
//! preferred hardware counter does not satisfy that contract during validation.
//!
//! Standard `Instant` methods return [`std::time::Duration`] to match the familiar Rust timing
//! API. `hotclock::Ticks` is the explicit raw counter-delta type for hot paths that want hardware
//! tick units directly. Use [`Instant::elapsed_ticks()`] or [`Instant::ticks_since()`] when raw
//! counter deltas are required.
//!
//! `Cycles` is the separate hot-loop clock contract: an Instant-shaped counter that can use
//! faster PMU or core-cycle sources such as RDPMC, PMCCNTR\_EL0, or rdcycle. It is for
//! same-thread microbenchmarks, profilers, and polling loops, not for cross-thread ordering or
//! measurements that must survive OS thread migration, descheduling, suspend/resume, or
//! hypervisor migration.
//!
//! # Frequency calibration
//!
//! [`Instant::frequency()`] is lazily calibrated on first call by measuring tick rate against
//! the system clock. Calibration is thread-safe and the result is cached for the lifetime of
//! the process. Calling it pre-warms calibration and, on runtime-selected targets, counter
//! selection. Time-conversion methods ([`Ticks::as_nanos`], [`Ticks::as_duration`], etc.) call
//! `frequency()` internally and therefore trigger calibration on first use. Wall-unit
//! conversions return `u128`; [`Ticks::as_duration()`] saturates at [`std::time::Duration::MAX`],
//! and [`Ticks::checked_duration()`] reports overflow with [`None`].

mod arch;
mod calibration;
mod convert;
mod cycle_ticks;
mod cycles;
mod instant;
#[cfg(not(any(
  all(target_arch = "aarch64", target_os = "macos"),
  not(any(
    target_arch = "x86_64",
    target_arch = "x86",
    target_arch = "aarch64",
    target_arch = "riscv64",
    target_arch = "loongarch64",
  )),
)))]
mod selection;
mod ticks;

pub use cycle_ticks::CycleTicks;
pub use cycles::Cycles;
pub use instant::Instant;
pub use ticks::Ticks;

/// Returns the selected cycle-counter frequency in Hz.
///
/// This is the free-function form of [`Cycles::frequency()`]. See that method for details.
pub use arch::cycle_frequency;
/// Returns the name of the selected cycle-counter implementation.
///
/// This is the free-function form of [`Cycles::implementation()`]. See that method for details.
pub use arch::cycle_implementation;
/// Returns the tick counter frequency in Hz.
///
/// This is the free-function form of [`Instant::frequency()`]. See that method for details.
pub use arch::frequency;
/// Returns the name of the selected counter implementation.
///
/// This is the free-function form of [`Instant::implementation()`]. See that method for details.
pub use arch::implementation;

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::Duration;

  fn assert_send_sync<T: Send + Sync>() {}

  fn wait_for_instant_duration(start: Instant) -> Duration {
    for _ in 0..1_000_000 {
      let elapsed = start.elapsed();
      if elapsed > Duration::ZERO {
        return elapsed;
      }
      std::hint::spin_loop();
    }
    start.elapsed()
  }

  fn wait_for_instant_ticks(start: Instant) -> Ticks {
    for _ in 0..1_000_000 {
      let elapsed = start.elapsed_ticks();
      if elapsed.as_raw() > 0 {
        return elapsed;
      }
      std::hint::spin_loop();
    }
    start.elapsed_ticks()
  }

  fn wait_for_cycle_ticks(start: Cycles) -> CycleTicks {
    for _ in 0..1_000_000 {
      let elapsed = start.elapsed_ticks();
      if elapsed.as_raw() > 0 {
        return elapsed;
      }
      std::hint::spin_loop();
    }
    start.elapsed_ticks()
  }

  #[test]
  fn test_instant_now() {
    let c = Instant::now();
    assert!(c.as_raw() > 0);
  }

  #[test]
  fn test_instant_is_send_sync() {
    assert_send_sync::<Instant>();
    assert_send_sync::<Ticks>();
    assert_send_sync::<Cycles>();
    assert_send_sync::<CycleTicks>();
  }

  #[test]
  fn test_cycles_now() {
    let c = Cycles::now();
    assert!(c.as_raw() > 0);
  }

  #[test]
  fn test_instant_elapsed() {
    let start = Instant::now();
    let elapsed = wait_for_instant_duration(start);
    assert!(elapsed > Duration::ZERO);
  }

  #[test]
  fn test_instant_elapsed_ticks() {
    let start = Instant::now();
    let elapsed = wait_for_instant_ticks(start);
    assert!(elapsed.as_raw() > 0);
  }

  #[test]
  fn test_cycles_elapsed_ticks() {
    let start = Cycles::now();
    let elapsed = wait_for_cycle_ticks(start);
    assert!(elapsed.as_raw() > 0);
  }

  #[test]
  fn test_instant_monotonic() {
    let mut previous = Instant::now();
    for _ in 0..10_000 {
      let current = Instant::now();
      assert!(
        current.as_raw() >= previous.as_raw(),
        "tick counter moved backward: previous={} current={}",
        previous.as_raw(),
        current.as_raw()
      );
      previous = current;
    }
  }

  #[test]
  fn test_instant_cross_thread_ordering() {
    use std::sync::Arc;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

    const CALLS: usize = 1000;

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
          let after = Instant::now().as_raw();
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

      published.store(Instant::now().as_raw(), Ordering::Release);
      sequence.store(i, Ordering::Release);

      while acknowledged.load(Ordering::Acquire) != i {
        if failed.load(Ordering::Relaxed) {
          break;
        }
        std::hint::spin_loop();
      }
    }

    assert!(reader.join().is_ok());
    assert!(!failed.load(Ordering::Relaxed));
  }

  #[test]
  fn test_implementation() {
    let impl_name = Instant::implementation();
    assert!(!impl_name.is_empty());
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    assert_eq!(impl_name, "aarch64-cntvct");
    println!("Implementation: {impl_name}");
  }

  #[test]
  #[allow(clippy::cast_precision_loss)]
  fn test_frequency() {
    let freq = Instant::frequency();
    println!("Frequency: {freq} Hz ({:.2} MHz)", freq as f64 / 1e6);
    // Sanity check: between 1 MHz and 100 GHz
    assert!((1_000_000..=100_000_000_000).contains(&freq));
  }

  #[test]
  fn test_frequency_concurrent_initialization() {
    let threads: Vec<_> = (0..8).map(|_| std::thread::spawn(Instant::frequency)).collect();
    let expected = Instant::frequency();

    for thread in threads {
      assert_eq!(thread.join().expect("frequency thread panicked"), expected);
    }
  }

  #[test]
  fn test_cycle_frequency_concurrent_initialization() {
    let threads: Vec<_> = (0..8).map(|_| std::thread::spawn(Cycles::frequency)).collect();
    let expected = Cycles::frequency();

    for thread in threads {
      assert_eq!(thread.join().expect("cycle frequency thread panicked"), expected);
    }
  }

  #[test]
  fn test_as_duration() {
    let start = Instant::now();
    std::thread::sleep(Duration::from_millis(10));
    let duration = start.elapsed();
    let elapsed = start.elapsed_ticks();

    println!("Slept for {duration:?}");
    // Should be at least 10ms
    assert!(duration.as_millis() >= 9);
    // But not more than 100ms (allowing for scheduler delays)
    assert!(duration.as_millis() < 100);
    assert!(elapsed.as_duration().as_millis() >= 9);
  }
}
