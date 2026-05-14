#![warn(clippy::undocumented_unsafe_blocks)]
#![warn(rustdoc::broken_intra_doc_links)]

//! Ultra-fast drop-in replacement for [`std::time::Instant`], plus a
//! [`Cycles`] API for the fastest possible read on each target/env.
//!
//! Each supported target compiles [`Instant::now()`] directly to the fastest
//! wall-clock-rate hardware counter for that architecture — RDTSC on x86/x86_64,
//! CNTVCT_EL0 on aarch64, rdtime on riscv64/loongarch64 — and falls back to a
//! platform monotonic clock everywhere else. No runtime dispatch on the hot path.
//!
//! [`Cycles::now()`] returns the fastest counter available for this target/env.
//! On Linux x86/x86_64, the first call detects PMU permission (RDPMC fixed,
//! perf-RDPMC) and patches every callsite to inline the winning counter's
//! bytes; on hosts without PMU access, Cycles uses the same wall-clock counter
//! Instant uses, so `Cycles ≤ Instant` on read cost, always.
//!
//! Roughly **~30x faster** than [`std::time::Instant`] on typical hardware.
//!
//! # Quick start
//!
//! ```
//! use tach::Instant;
//!
//! let start = Instant::now();
//! // ... do some work ...
//! let elapsed = start.elapsed();
//! println!("Elapsed: {:?}", elapsed);
//! ```
//!
//! # Platform support
//!
//! | Architecture        | Instant clock  | Cycles selection | Fallback       |
//! |---------------------|----------------|------------------|----------------|
//! | x86_64 / x86 Linux  | RDTSC          | RDPMC variants   | clock_gettime  |
//! | aarch64 macOS       | CNTVCT_EL0     | = Instant        | none           |
//! | aarch64 Linux       | CNTVCT_EL0     | = Instant        | clock_gettime  |
//! | aarch64 Windows     | CNTVCT_EL0     | = Instant        | QPC            |
//! | x86_64 macOS        | RDTSC          | = Instant        | mach           |
//! | x86_64 Windows      | RDTSC          | = Instant        | QPC            |
//! | riscv64             | rdtime         | = Instant        | clock_gettime  |
//! | loongarch64         | rdtime.d       | = Instant        | clock_gettime  |
//! | other               | OS timer       | = Instant        | none           |
//!
//! `Cycles selection = Instant` means the target has no faster cycle counter
//! exposed to user mode, so `Cycles::now()` compile-time-resolves to the same
//! tick reader as `Instant::now()`.
//!
//! # Timing contract
//!
//! `Instant` is wall-clock-rate: keeps ticking through park, suspension, and
//! descheduling. Same source across every thread. **Not strictly cross-thread
//! monotonic** — raw hardware counters can disagree across CPUs by sync slop on
//! some hosts. For strict cross-thread monotonicity, use [`std::time::Instant`].
//!
//! `Cycles` is the fastest read available. When backed by a CPU cycle counter,
//! it has cycle-counter semantics: per-core, stops during idle, not safe across
//! thread migration. When backed by the wall-clock fallback (most targets), it
//! has Instant's semantics. `Cycles::now()` always returns a value.
//!
//! # Frequency calibration
//!
//! [`Instant::frequency()`] and [`Cycles::frequency()`] are lazily calibrated
//! on first call by measuring tick rate against the system clock. The result is
//! cached for the lifetime of the process.

#[doc(hidden)]
pub mod arch;
mod calibration;
mod convert;
mod cycle_ticks;
mod cycles;
mod instant;
#[cfg(all(
  target_os = "linux",
  any(target_arch = "x86_64", target_arch = "aarch64")
))]
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
