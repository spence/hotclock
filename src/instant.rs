use core::time::Duration;

use crate::arch;

/// A sampled point in the process-wide counter timeline.
///
/// Drop-in replacement for [`std::time::Instant`] backed by the architectural
/// wall-clock counter (RDTSC, CNTVCT_EL0, rdtime).
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
  /// Drop-in equivalent for [`std::time::Instant::elapsed()`].
  #[inline]
  #[must_use]
  pub fn elapsed(&self) -> Duration {
    let delta = arch::ticks().wrapping_sub(self.0);
    ticks_to_duration(delta)
  }

  #[inline(always)]
  pub(crate) fn from_raw_ticks(ticks: u64) -> Self {
    Self(ticks)
  }
}

/// An [`Instant`] sampled with an instruction-ordering barrier so the
/// timestamp cannot be reordered before any prior `Acquire`-or-stronger
/// observation.
///
/// Use this when correlating a timestamp with synchronization state — e.g.
/// reading a deadline or yielding signal from another thread and needing
/// the timestamp to reflect time *after* the observation:
///
/// ```ignore
/// let deadline = scheduler_state.load(Ordering::Acquire);
/// let now = OrderedInstant::now();
/// // `now` is guaranteed to be sampled after `deadline` was observed.
/// ```
///
/// With plain [`Instant`] the counter read can be hoisted earlier than the
/// acquire-load completes (on aarch64 `mrs cntvct_el0` is a system-register
/// access that memory fences do not constrain; on x86 `rdtsc` is not a
/// serializing instruction). [`OrderedInstant::now()`] emits the
/// arch-appropriate barrier (`isb sy` on aarch64, `lfence` on x86,
/// `fence ir, ir` on riscv64, `dbar 0` on loongarch64) before the counter
/// read so reordering is prevented.
///
/// # Cost
///
/// Roughly 5–20 ns more than [`Instant::now()`] depending on architecture;
/// still substantially faster than [`std::time::Instant::now()`] on Linux
/// and macOS (which call into the vDSO / libsystem path but do not
/// themselves guarantee this ordering against atomics).
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct OrderedInstant(u64);

impl OrderedInstant {
  /// Reads the counter with an instruction-ordering barrier so the
  /// timestamp is sampled *after* any prior `Acquire`-or-stronger
  /// observation.
  #[inline(always)]
  #[allow(clippy::inline_always)]
  pub fn now() -> Self {
    Self(arch::ticks_ordered())
  }

  /// Returns the duration that has elapsed since `self` was sampled, with
  /// the end read also ordered. Use this when the elapsed end must come
  /// after some downstream synchronization point.
  #[inline]
  #[must_use]
  pub fn elapsed(&self) -> Duration {
    let delta = arch::ticks_ordered().wrapping_sub(self.0);
    ticks_to_duration(delta)
  }

  /// Returns the elapsed duration with an *unordered* end read. Use this
  /// only when the start of the measurement needed ordering (e.g. anchored
  /// to a published deadline) but the end is only for logging or coarse
  /// reporting where pre-acquire drift is harmless.
  #[inline]
  #[must_use]
  pub fn elapsed_unordered(&self) -> Duration {
    let delta = arch::ticks().wrapping_sub(self.0);
    ticks_to_duration(delta)
  }

  /// Discards the ordering guarantee and returns a plain [`Instant`] with
  /// the same tick value. Useful when storing the timestamp in a struct
  /// field typed as [`Instant`]. There is no inverse — an unordered
  /// [`Instant`] cannot be promoted because the original read was not
  /// ordered.
  #[inline]
  pub fn as_unordered(&self) -> Instant {
    Instant::from_raw_ticks(self.0)
  }
}

// Q32 fixed-point conversion: nanos = (ticks * scale) >> 32 where
// scale = (1e9 << 32) / frequency. Avoids the per-call u128 division
// which is slow on virtualized x86 (Nitro burst VMs, Firecracker on
// Lambda) — typical savings on those targets is 15-25 ns/call.
#[inline]
fn ticks_to_duration(ticks: u64) -> Duration {
  let product = u128::from(ticks) * u128::from(arch::nanos_per_tick_q32());
  let nanos = u64::try_from(product >> 32).unwrap_or(u64::MAX);
  // Common case for elapsed (< 1 second): build Duration directly from
  // secs=0 + subsec_nanos. The compiler can prove `nanos_u32 < 1e9` from
  // the branch and elide the internal divide in Duration::new. Avoids
  // a divide by 1e9 on the hot path (~10 ns on virtualized x86).
  if nanos < 1_000_000_000 { Duration::new(0, nanos as u32) } else { Duration::from_nanos(nanos) }
}
