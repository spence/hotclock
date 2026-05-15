use std::time::Duration;

use crate::arch;

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

// Q32 fixed-point conversion: nanos = (ticks * scale) >> 32 where
// scale = (1e9 << 32) / frequency. Avoids the per-call u128 division
// which is slow on virtualized x86 (Nitro burst VMs, Firecracker on
// Lambda) — typical savings on those targets is 15-25 ns/call.
#[inline]
fn ticks_to_nanos(ticks: u64) -> u64 {
  let product = u128::from(ticks) * u128::from(arch::nanos_per_tick_q32());
  u64::try_from(product >> 32).unwrap_or(u64::MAX)
}

#[inline]
fn ticks_to_duration(ticks: u64) -> Duration {
  let nanos = ticks_to_nanos(ticks);
  // Common case for elapsed (< 1 second): build Duration directly from
  // secs=0 + subsec_nanos. The compiler can prove `nanos_u32 < 1e9` from
  // the branch and elide the internal divide in Duration::new. Avoids
  // a divide by 1e9 on the hot path (~10 ns on virtualized x86).
  if nanos < 1_000_000_000 {
    Duration::new(0, nanos as u32)
  } else {
    Duration::from_nanos(nanos)
  }
}
