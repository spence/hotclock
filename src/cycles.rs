use std::time::Duration;

use crate::{CycleTicks, arch, convert};

/// A sampled point in the selected cycle-counter timeline.
///
/// `Cycles` is the lower-level sibling of [`crate::Instant`]. It keeps the same value shape and
/// elapsed-time convenience methods, but it can select PMU/core-cycle sources that are faster
/// than the `Instant` clock on some machines. Use it for same-thread hot-loop timing,
/// profiling, and microbenchmarks where counter-read cost dominates. Runtime-selected targets
/// patch warmed `Cycles::now()` call sites independently from [`crate::Instant`].
///
/// Do not use `Cycles` for cross-thread ordering or measurements that must survive OS thread
/// migration, descheduling, suspend/resume, or hypervisor migration with `Instant` semantics.
///
/// ```
/// use tach::Cycles;
///
/// let start = Cycles::now();
/// // ... hot-loop work ...
/// let ticks = start.elapsed_ticks();
/// println!("{} cycle ticks", ticks.as_raw());
/// ```
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct Cycles(u64);

impl Cycles {
  /// Reads the current value of the selected cycle counter.
  #[inline(always)]
  #[allow(clippy::inline_always)]
  pub fn now() -> Self {
    Self(arch::cycle_ticks())
  }

  /// Returns the duration that has elapsed since `self` was sampled.
  #[inline]
  #[must_use]
  pub fn elapsed(&self) -> Duration {
    Self::now().duration_since(*self)
  }

  /// Returns the raw cycle-counter delta elapsed since `self` was sampled.
  #[inline(always)]
  #[allow(clippy::inline_always)]
  pub fn elapsed_ticks(&self) -> CycleTicks {
    Self::now().wrapping_ticks_since(*self)
  }

  /// Returns the selected cycle-counter frequency in Hz.
  #[inline]
  #[must_use]
  pub fn frequency() -> u64 {
    arch::cycle_frequency()
  }

  /// Returns the name of the compiled-in or selected cycle-counter implementation.
  #[inline]
  #[must_use]
  pub fn implementation() -> &'static str {
    arch::cycle_implementation()
  }

  /// Creates a `Cycles` value from a raw `u64` counter reading.
  #[inline]
  pub const fn from_raw(ticks: u64) -> Self {
    Self(ticks)
  }

  /// Returns the underlying raw counter reading.
  #[inline]
  #[must_use]
  pub const fn as_raw(&self) -> u64 {
    self.0
  }

  /// Returns `true` if the counter reading is zero.
  #[inline]
  #[must_use]
  pub const fn is_zero(&self) -> bool {
    self.0 == 0
  }

  /// Returns the elapsed duration since `earlier`, saturating to zero on underflow.
  #[inline]
  #[must_use]
  pub fn duration_since(&self, earlier: Self) -> Duration {
    self.saturating_duration_since(earlier)
  }

  /// Returns the elapsed duration since `earlier`.
  #[inline]
  #[must_use]
  pub fn checked_duration_since(&self, earlier: Self) -> Option<Duration> {
    self.checked_ticks_since(earlier)?.checked_duration()
  }

  /// Returns the elapsed duration since `earlier`, saturating to zero on underflow.
  #[inline]
  #[must_use]
  pub fn saturating_duration_since(&self, earlier: Self) -> Duration {
    match self.checked_ticks_since(earlier) {
      Some(ticks) => ticks.as_duration(),
      None => Duration::ZERO,
    }
  }

  /// Returns the elapsed raw counter delta since `earlier`, saturating to zero on underflow.
  #[inline]
  pub const fn ticks_since(&self, earlier: Self) -> CycleTicks {
    self.saturating_ticks_since(earlier)
  }

  /// Returns the elapsed raw counter delta since `earlier`, saturating to zero on underflow.
  #[inline]
  pub const fn saturating_ticks_since(&self, earlier: Self) -> CycleTicks {
    CycleTicks::from_raw(self.0.saturating_sub(earlier.0))
  }

  /// Returns the elapsed raw counter delta since `earlier`, wrapping on underflow.
  #[inline]
  pub const fn wrapping_ticks_since(&self, earlier: Self) -> CycleTicks {
    CycleTicks::from_raw(self.0.wrapping_sub(earlier.0))
  }

  /// Returns the elapsed raw counter delta since `earlier`, or [`None`] on underflow.
  #[inline]
  #[must_use]
  pub const fn checked_ticks_since(&self, earlier: Self) -> Option<CycleTicks> {
    match self.0.checked_sub(earlier.0) {
      Some(ticks) => Some(CycleTicks::from_raw(ticks)),
      None => None,
    }
  }

  /// Returns `self + duration`, or [`None`] if the result overflows.
  #[inline]
  #[must_use]
  pub fn checked_add(&self, duration: Duration) -> Option<Self> {
    let ticks = convert::duration_to_ticks(duration, Self::frequency())?;
    self.checked_add_ticks(CycleTicks::from_raw(ticks))
  }

  /// Returns `self - duration`, or [`None`] if the result underflows.
  #[inline]
  #[must_use]
  pub fn checked_sub(&self, duration: Duration) -> Option<Self> {
    let ticks = convert::duration_to_ticks(duration, Self::frequency())?;
    self.checked_sub_ticks(CycleTicks::from_raw(ticks))
  }

  /// Returns `self + ticks`, or [`None`] if the result overflows.
  #[inline]
  #[must_use]
  pub const fn checked_add_ticks(&self, ticks: CycleTicks) -> Option<Self> {
    match self.0.checked_add(ticks.as_raw()) {
      Some(value) => Some(Self(value)),
      None => None,
    }
  }

  /// Returns `self - ticks`, or [`None`] if the result underflows.
  #[inline]
  #[must_use]
  pub const fn checked_sub_ticks(&self, ticks: CycleTicks) -> Option<Self> {
    match self.0.checked_sub(ticks.as_raw()) {
      Some(value) => Some(Self(value)),
      None => None,
    }
  }

  /// Saturating addition of raw counter deltas.
  #[inline]
  pub const fn saturating_add_ticks(&self, ticks: CycleTicks) -> Self {
    Self(self.0.saturating_add(ticks.as_raw()))
  }

  /// Saturating subtraction of raw counter deltas.
  #[inline]
  pub const fn saturating_sub_ticks(&self, ticks: CycleTicks) -> Self {
    Self(self.0.saturating_sub(ticks.as_raw()))
  }

  /// Resets this value to the current cycle counter.
  #[inline]
  pub fn reset(&mut self) {
    self.0 = arch::cycle_ticks();
  }
}

impl core::fmt::Display for Cycles {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "{} cycles", self.0)
  }
}

impl From<u64> for Cycles {
  #[inline]
  fn from(ticks: u64) -> Self {
    Self(ticks)
  }
}

impl From<Cycles> for u64 {
  #[inline]
  fn from(cycles: Cycles) -> Self {
    cycles.0
  }
}

impl core::ops::Add<CycleTicks> for Cycles {
  type Output = Self;

  #[inline]
  fn add(self, ticks: CycleTicks) -> Self {
    Self(self.0 + ticks.as_raw())
  }
}

impl core::ops::Add<Duration> for Cycles {
  type Output = Self;

  #[inline]
  fn add(self, duration: Duration) -> Self {
    self.checked_add(duration).expect("overflow when adding duration to cycles")
  }
}

impl core::ops::AddAssign<CycleTicks> for Cycles {
  #[inline]
  fn add_assign(&mut self, ticks: CycleTicks) {
    self.0 += ticks.as_raw();
  }
}

impl core::ops::AddAssign<Duration> for Cycles {
  #[inline]
  fn add_assign(&mut self, duration: Duration) {
    *self = *self + duration;
  }
}

impl core::ops::Sub for Cycles {
  type Output = Duration;

  #[inline]
  fn sub(self, earlier: Self) -> Duration {
    self.duration_since(earlier)
  }
}

impl core::ops::Sub<CycleTicks> for Cycles {
  type Output = Self;

  #[inline]
  fn sub(self, ticks: CycleTicks) -> Self {
    Self(self.0 - ticks.as_raw())
  }
}

impl core::ops::Sub<Duration> for Cycles {
  type Output = Self;

  #[inline]
  fn sub(self, duration: Duration) -> Self {
    self
      .checked_sub(duration)
      .expect("overflow when subtracting duration from cycles")
  }
}

impl core::ops::SubAssign<CycleTicks> for Cycles {
  #[inline]
  fn sub_assign(&mut self, ticks: CycleTicks) {
    self.0 -= ticks.as_raw();
  }
}

impl core::ops::SubAssign<Duration> for Cycles {
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
    let a = Cycles::from_raw(100);
    let b = Cycles::from_raw(50);
    let ticks = CycleTicks::from_raw(50);
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
    assert_eq!(b.saturating_ticks_since(a), CycleTicks::from_raw(0));
    assert_eq!(b.wrapping_ticks_since(a), CycleTicks::from_raw(u64::MAX - 49));
    assert_eq!(a.checked_add_ticks(CycleTicks::from_raw(u64::MAX)), None);
    assert_eq!(b.checked_sub_ticks(CycleTicks::from_raw(51)), None);
    assert_eq!(b.saturating_sub_ticks(CycleTicks::from_raw(51)), Cycles::from_raw(0));
  }

  #[test]
  fn std_duration_arithmetic() {
    let one_second = Duration::from_secs(1);
    let one_second_ticks = CycleTicks::from_raw(Cycles::frequency());
    let start = Cycles::from_raw(0);
    let later = Cycles::from_raw(one_second_ticks.as_raw());

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
