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

pub use instant::Instant;

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
}
