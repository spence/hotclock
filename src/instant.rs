use std::time::Duration;

use crate::arch;

const NANOS_PER_SECOND: u128 = 1_000_000_000;

/// A sampled point in the process-wide counter timeline.
///
/// Drop-in replacement for [`std::time::Instant`] backed by the architectural
/// wall-clock counter (RDTSC, CNTVCT_EL0, rdtime). [`Instant::now()`] compiles
/// to a single counter read on supported targets.
///
/// `Instant` is wall-clock-rate: it keeps ticking through park, suspension, and
/// descheduling. The same source is used across every thread in the process,
/// but raw hardware counters can disagree across CPUs by sub-microsecond sync
/// slop on most hosts. For strict cross-thread monotonicity, use
/// [`std::time::Instant`].
///
/// # Example
///
/// ```
/// use tach::Instant;
///
/// let start = Instant::now();
/// // ... work ...
/// let elapsed = start.elapsed();
/// println!("{elapsed:?}");
/// ```
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Instant(u64);

impl Instant {
  /// Reads the current value of the process-wide tick counter.
  ///
  /// Compiles to a single architectural counter read on every supported target.
  #[inline(always)]
  #[allow(clippy::inline_always)]
  pub fn now() -> Self {
    Self(arch::ticks())
  }

  /// Returns the duration that has elapsed since `self` was sampled.
  ///
  /// Drop-in equivalent for [`std::time::Instant::elapsed()`]. Use
  /// [`elapsed_fast`](Instant::elapsed_fast) when integer nanoseconds suffice
  /// and you want to skip [`Duration`] construction.
  #[inline]
  #[must_use]
  pub fn elapsed(&self) -> Duration {
    let delta = arch::ticks().wrapping_sub(self.0);
    ticks_to_duration(delta)
  }

  /// Returns the elapsed nanoseconds since `self` was sampled.
  ///
  /// Faster than [`elapsed`](Instant::elapsed) when you only need an integer:
  /// skips the [`Duration`] `(secs, nanos)` construction. Saturates at
  /// [`u64::MAX`] (~584 years).
  #[inline]
  #[must_use]
  pub fn elapsed_fast(&self) -> u64 {
    let delta = arch::ticks().wrapping_sub(self.0);
    ticks_to_nanos(delta)
  }
}

#[inline]
fn ticks_to_nanos(ticks: u64) -> u64 {
  let nanos = u128::from(ticks) * NANOS_PER_SECOND / u128::from(arch::frequency());
  u64::try_from(nanos).unwrap_or(u64::MAX)
}

#[inline]
fn ticks_to_duration(ticks: u64) -> Duration {
  let nanos = u128::from(ticks) * NANOS_PER_SECOND / u128::from(arch::frequency());
  let secs = u64::try_from(nanos / NANOS_PER_SECOND).unwrap_or(u64::MAX);
  let subsec_nanos = u32::try_from(nanos % NANOS_PER_SECOND).unwrap_or(0);
  Duration::new(secs, subsec_nanos)
}
