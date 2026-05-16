#![no_std]
#![warn(clippy::undocumented_unsafe_blocks)]
#![warn(rustdoc::broken_intra_doc_links)]

//! Ultra-fast drop-in replacement for [`std::time::Instant`].
//!
//! Each supported target compiles [`Instant::now()`] to a single architectural
//! counter read — RDTSC on x86 / x86_64, CNTVCT_EL0 on aarch64, rdtime on
//! riscv64 / loongarch64 — and falls back to the platform monotonic clock
//! everywhere else. No runtime dispatch on the hot path.
//!
//! # Quick start
//!
//! ```
//! use tach::Instant;
//!
//! let start = Instant::now();
//! // ... work ...
//! let elapsed = start.elapsed();
//! println!("{elapsed:?}");
//! ```
//!
//! # Timing contract
//!
//! `Instant` is wall-clock-rate: keeps ticking through park, suspension, and
//! descheduling. Same source across every thread in the process. **Not strictly
//! cross-thread monotonic** — raw hardware counters can disagree across CPUs by
//! sub-microsecond sync slop on most hosts. For strict cross-thread monotonicity,
//! use [`std::time::Instant`].

mod arch;
#[cfg(not(any(
  target_arch = "aarch64",
  target_os = "macos",
  target_os = "windows",
  target_os = "wasi",
  all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
)))]
mod calibration;
mod instant;

pub use instant::{Instant, OrderedInstant};

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::Duration;

  #[test]
  fn instant_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Instant>();
    assert_send_sync::<OrderedInstant>();
  }

  #[test]
  fn now_advances() {
    let mut previous = Instant::now();
    for _ in 0..10_000 {
      let current = Instant::now();
      assert!(current >= previous, "counter moved backward");
      previous = current;
    }
  }

  #[test]
  fn elapsed_after_sleep() {
    let start = Instant::now();
    std::thread::sleep(Duration::from_millis(10));
    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() >= 9, "elapsed too short: {elapsed:?}");
    assert!(elapsed.as_millis() < 200, "elapsed too long: {elapsed:?}");
  }

  #[test]
  fn ordered_now_advances() {
    let mut previous = OrderedInstant::now();
    for _ in 0..10_000 {
      let current = OrderedInstant::now();
      assert!(current >= previous, "ordered counter moved backward");
      previous = current;
    }
  }

  #[test]
  fn ordered_elapsed_after_sleep() {
    let start = OrderedInstant::now();
    std::thread::sleep(Duration::from_millis(10));
    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() >= 9, "ordered elapsed too short: {elapsed:?}");
    assert!(elapsed.as_millis() < 200, "ordered elapsed too long: {elapsed:?}");
  }

  // `as_unordered()` shares the same underlying tick value, so an elapsed
  // measurement from the converted unordered handle should match an elapsed
  // measurement from the original within bench-runtime noise.
  #[test]
  fn ordered_as_unordered_preserves_tick_value() {
    let ordered = OrderedInstant::now();
    let unordered = ordered.as_unordered();
    let elapsed_from_ordered = ordered.elapsed_unordered();
    let elapsed_from_unordered = unordered.elapsed();
    let diff = elapsed_from_ordered.abs_diff(elapsed_from_unordered);
    // The two .elapsed*() calls happen back-to-back; diff is whatever a
    // single counter read costs. 1ms is generous noise budget.
    assert!(diff.as_millis() < 1, "elapsed diverged after as_unordered: {diff:?}");
  }

  // Pairing OrderedInstant start with elapsed_unordered() end: end timestamp
  // is unordered but should still come after the ordered start (sleep is well
  // longer than any reordering window).
  #[test]
  fn ordered_elapsed_unordered_after_sleep() {
    let start = OrderedInstant::now();
    std::thread::sleep(Duration::from_millis(10));
    let elapsed = start.elapsed_unordered();
    assert!(elapsed.as_millis() >= 9, "elapsed_unordered too short: {elapsed:?}");
    assert!(elapsed.as_millis() < 200, "elapsed_unordered too long: {elapsed:?}");
  }
}
