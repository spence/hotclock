use std::time::Duration;

use crate::{Ticks, arch, convert};

/// A sampled point in the process-wide counter timeline.
///
/// `Instant` is a `#[repr(transparent)]` newtype over [`u64`]. The value wrapper is zero-cost;
/// [`Instant::now()`] compiles to a single architectural counter read on supported targets and
/// to a platform monotonic-clock read otherwise. `Instant` is `Send` and `Sync`, comparable, and
/// hashable. It is not a civil-time or OS timestamp.
///
/// # Obtaining elapsed time
///
/// Use [`Instant::now()`] to sample the current counter. Standard elapsed-time methods return
/// [`Duration`]. Use [`elapsed_ticks`](Instant::elapsed_ticks) when raw counter deltas are needed:
///
/// ```
/// use tach::Instant;
///
/// let start = Instant::now();
/// // ... work ...
/// let duration = start.elapsed();
/// let ticks = start.elapsed_ticks();
/// println!("{duration:?} ({} raw ticks)", ticks.as_raw());
/// ```
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct Instant(u64);

impl Instant {
  /// Reads the current value of the process-wide tick counter.
  ///
  /// Compiles to a single architectural counter read on every supported target.
  ///
  /// # Example
  ///
  /// ```
  /// use tach::Instant;
  /// let t = Instant::now();
  /// assert!(t.as_raw() > 0);
  /// ```
  #[inline(always)]
  #[allow(clippy::inline_always)]
  pub fn now() -> Self {
    Self(arch::ticks())
  }

  /// Returns the duration that has elapsed since `self` was sampled.
  ///
  /// This is the familiar [`std::time::Instant`]-style API. Use
  /// [`elapsed_ticks`](Instant::elapsed_ticks) for the raw counter delta.
  ///
  /// # Example
  ///
  /// ```
  /// use tach::Instant;
  /// let start = Instant::now();
  /// // ... work ...
  /// let elapsed = start.elapsed();
  /// println!("{elapsed:?}");
  /// ```
  #[inline]
  #[must_use]
  pub fn elapsed(&self) -> Duration {
    Self::now().duration_since(*self)
  }

  /// Returns the raw counter ticks that have elapsed since `self` was sampled.
  ///
  /// Uses wrapping subtraction so extremely long-running counters can roll over. Use
  /// [`Instant::elapsed()`] when a [`Duration`] is preferred.
  #[inline(always)]
  #[allow(clippy::inline_always)]
  pub fn elapsed_ticks(&self) -> Ticks {
    Self::now().wrapping_ticks_since(*self)
  }

  /// Returns the tick counter frequency in Hz.
  ///
  /// Calibrated lazily and thread-safely on first call by measuring the tick rate against
  /// the system clock over a short spin-wait. The result is cached for the lifetime of
  /// the process.
  ///
  /// # Example
  ///
  /// ```
  /// use tach::Instant;
  /// let freq = Instant::frequency();
  /// println!("{:.2} MHz", freq as f64 / 1e6);
  /// ```
  #[inline]
  #[must_use]
  pub fn frequency() -> u64 {
    arch::frequency()
  }

  /// Returns the name of the compiled-in or selected counter implementation (e.g.
  /// `"aarch64-cntvct"`, `"x86_64-rdtsc"`, `"macos-mach"`).
  ///
  /// Useful for diagnostics and logging.
  #[inline]
  #[must_use]
  pub fn implementation() -> &'static str {
    arch::implementation()
  }

  /// Creates an `Instant` value from a raw `u64` counter reading.
  #[inline]
  pub const fn from_raw(ticks: u64) -> Self {
    Self(ticks)
  }

  /// Returns the underlying raw tick count.
  #[inline]
  #[must_use]
  pub const fn as_raw(&self) -> u64 {
    self.0
  }

  /// Returns `true` if the tick count is zero.
  #[inline]
  #[must_use]
  pub const fn is_zero(&self) -> bool {
    self.0 == 0
  }

  /// Returns the elapsed duration since `earlier`, saturating to zero on underflow.
  ///
  /// If the converted duration is too large for [`Duration`], the result saturates to
  /// [`Duration::MAX`].
  #[inline]
  #[must_use]
  pub fn duration_since(&self, earlier: Self) -> Duration {
    self.saturating_duration_since(earlier)
  }

  /// Returns the elapsed duration since `earlier`.
  ///
  /// Returns [`None`] when `earlier` is later than `self` or when the converted duration does not
  /// fit in [`Duration`].
  #[inline]
  #[must_use]
  pub fn checked_duration_since(&self, earlier: Self) -> Option<Duration> {
    self.checked_ticks_since(earlier)?.checked_duration()
  }

  /// Returns the elapsed duration since `earlier`, saturating to zero on underflow.
  ///
  /// If the converted duration is too large for [`Duration`], the result saturates to
  /// [`Duration::MAX`].
  #[inline]
  #[must_use]
  pub fn saturating_duration_since(&self, earlier: Self) -> Duration {
    match self.checked_ticks_since(earlier) {
      Some(ticks) => ticks.as_duration(),
      None => Duration::ZERO,
    }
  }

  /// Returns the elapsed raw ticks since `earlier`, saturating to zero on underflow.
  #[inline]
  pub const fn ticks_since(&self, earlier: Self) -> Ticks {
    self.saturating_ticks_since(earlier)
  }

  /// Returns the elapsed raw ticks since `earlier`, saturating to zero on underflow.
  #[inline]
  pub const fn saturating_ticks_since(&self, earlier: Self) -> Ticks {
    Ticks::from_raw(self.0.saturating_sub(earlier.0))
  }

  /// Returns the elapsed raw ticks since `earlier`, wrapping on underflow.
  ///
  /// This is useful when the underlying counter may have rolled over between samples.
  #[inline]
  pub const fn wrapping_ticks_since(&self, earlier: Self) -> Ticks {
    Ticks::from_raw(self.0.wrapping_sub(earlier.0))
  }

  /// Returns the elapsed raw ticks since `earlier`, or [`None`] on underflow.
  #[inline]
  #[must_use]
  pub const fn checked_ticks_since(&self, earlier: Self) -> Option<Ticks> {
    match self.0.checked_sub(earlier.0) {
      Some(ticks) => Some(Ticks::from_raw(ticks)),
      None => None,
    }
  }

  /// Returns `self + duration`, or [`None`] if the result overflows.
  ///
  /// Durations are rounded up to the next representable counter tick.
  #[inline]
  #[must_use]
  pub fn checked_add(&self, duration: Duration) -> Option<Self> {
    let ticks = convert::duration_to_ticks(duration, Self::frequency())?;
    self.checked_add_ticks(Ticks::from_raw(ticks))
  }

  /// Returns `self - duration`, or [`None`] if the result underflows.
  ///
  /// Durations are rounded up to the next representable counter tick.
  #[inline]
  #[must_use]
  pub fn checked_sub(&self, duration: Duration) -> Option<Self> {
    let ticks = convert::duration_to_ticks(duration, Self::frequency())?;
    self.checked_sub_ticks(Ticks::from_raw(ticks))
  }

  /// Returns `self + ticks`, or [`None`] if the result overflows.
  #[inline]
  #[must_use]
  pub const fn checked_add_ticks(&self, ticks: Ticks) -> Option<Self> {
    match self.0.checked_add(ticks.as_raw()) {
      Some(value) => Some(Self(value)),
      None => None,
    }
  }

  /// Returns `self - ticks`, or [`None`] if the result underflows.
  #[inline]
  #[must_use]
  pub const fn checked_sub_ticks(&self, ticks: Ticks) -> Option<Self> {
    match self.0.checked_sub(ticks.as_raw()) {
      Some(value) => Some(Self(value)),
      None => None,
    }
  }

  /// Saturating addition of raw ticks. Returns [`u64::MAX`] ticks on overflow.
  #[inline]
  pub const fn saturating_add_ticks(&self, ticks: Ticks) -> Self {
    Self(self.0.saturating_add(ticks.as_raw()))
  }

  /// Saturating subtraction of raw ticks. Returns zero ticks on underflow.
  #[inline]
  pub const fn saturating_sub_ticks(&self, ticks: Ticks) -> Self {
    Self(self.0.saturating_sub(ticks.as_raw()))
  }

  /// Resets this value to the current tick counter, equivalent to `*self = Instant::now()`.
  #[inline]
  pub fn reset(&mut self) {
    self.0 = arch::ticks();
  }
}

impl core::fmt::Display for Instant {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "{} ticks", self.0)
  }
}

impl From<u64> for Instant {
  #[inline]
  fn from(ticks: u64) -> Self {
    Self(ticks)
  }
}

impl From<Instant> for u64 {
  #[inline]
  fn from(instant: Instant) -> Self {
    instant.0
  }
}

impl core::ops::Add<Ticks> for Instant {
  type Output = Self;

  #[inline]
  fn add(self, ticks: Ticks) -> Self {
    Self(self.0 + ticks.as_raw())
  }
}

impl core::ops::Add<Duration> for Instant {
  type Output = Self;

  #[inline]
  fn add(self, duration: Duration) -> Self {
    self.checked_add(duration).expect("overflow when adding duration to instant")
  }
}

impl core::ops::AddAssign<Ticks> for Instant {
  #[inline]
  fn add_assign(&mut self, ticks: Ticks) {
    self.0 += ticks.as_raw();
  }
}

impl core::ops::AddAssign<Duration> for Instant {
  #[inline]
  fn add_assign(&mut self, duration: Duration) {
    *self = *self + duration;
  }
}

impl core::ops::Sub for Instant {
  type Output = Duration;

  #[inline]
  fn sub(self, earlier: Self) -> Duration {
    self.duration_since(earlier)
  }
}

impl core::ops::Sub<Ticks> for Instant {
  type Output = Self;

  #[inline]
  fn sub(self, ticks: Ticks) -> Self {
    Self(self.0 - ticks.as_raw())
  }
}

impl core::ops::Sub<Duration> for Instant {
  type Output = Self;

  #[inline]
  fn sub(self, duration: Duration) -> Self {
    self
      .checked_sub(duration)
      .expect("overflow when subtracting duration from instant")
  }
}

impl core::ops::SubAssign<Ticks> for Instant {
  #[inline]
  fn sub_assign(&mut self, ticks: Ticks) {
    self.0 -= ticks.as_raw();
  }
}

impl core::ops::SubAssign<Duration> for Instant {
  #[inline]
  fn sub_assign(&mut self, duration: Duration) {
    *self = *self - duration;
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn duration_arithmetic() {
    let a = Instant::from_raw(100);
    let b = Instant::from_raw(50);
    let ticks = Ticks::from_raw(50);
    let duration = ticks.as_duration();

    assert_eq!(a - b, duration);
    assert_eq!(a.duration_since(b), duration);
    assert_eq!(b + ticks, a);
    assert_eq!(a - ticks, b);
    assert_eq!(a.checked_duration_since(b), Some(duration));
    assert_eq!(b.checked_duration_since(a), None);
    assert_eq!(b.saturating_duration_since(a), Duration::ZERO);
    assert_eq!(a.ticks_since(b), ticks);
    assert_eq!(a.checked_ticks_since(b), Some(ticks));
    assert_eq!(b.checked_ticks_since(a), None);
    assert_eq!(b.saturating_ticks_since(a), Ticks::from_raw(0));
    assert_eq!(b.wrapping_ticks_since(a), Ticks::from_raw(u64::MAX - 49));
    assert_eq!(a.checked_add_ticks(Ticks::from_raw(u64::MAX)), None);
    assert_eq!(b.checked_sub_ticks(Ticks::from_raw(51)), None);
    assert_eq!(b.saturating_sub_ticks(Ticks::from_raw(51)), Instant::from_raw(0));
  }

  #[test]
  fn std_duration_arithmetic() {
    let one_second = Duration::from_secs(1);
    let one_second_ticks = Ticks::from_raw(Instant::frequency());
    let start = Instant::from_raw(0);
    let later = Instant::from_raw(one_second_ticks.as_raw());

    assert_eq!(start.checked_add(one_second), Some(later));
    assert_eq!(later.checked_sub(one_second), Some(start));
    assert_eq!(start + one_second, later);
    assert_eq!(later - one_second, start);

    let mut assign = start;
    assign += one_second;
    assert_eq!(assign, later);
    assign -= one_second;
    assert_eq!(assign, start);
  }
}
