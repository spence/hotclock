use std::time::Duration;

use crate::{arch, convert};

/// An elapsed raw tick count from the process-wide counter.
///
/// `Ticks` represents an amount of counter movement, not a sampled point in time. Conversion
/// methods such as [`as_nanos`](Ticks::as_nanos), [`as_micros`](Ticks::as_micros),
/// [`as_millis`](Ticks::as_millis), [`as_secs_f64`](Ticks::as_secs_f64), and
/// [`as_duration`](Ticks::as_duration) divide by the calibrated counter frequency. The
/// frequency is calibrated lazily on first conversion and cached thereafter.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct Ticks(u64);

impl Ticks {
  /// Returns the tick counter frequency in Hz.
  ///
  /// This is the same cached calibration used by [`crate::Instant::frequency`].
  #[inline]
  #[must_use]
  pub fn frequency() -> u64 {
    arch::frequency()
  }

  /// Creates a `Ticks` value from a raw `u64` counter delta.
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

  /// Converts this elapsed tick count to nanoseconds using the calibrated frequency.
  #[inline]
  #[must_use]
  pub fn as_nanos(&self) -> u128 {
    convert::ticks_to_unit(self.0, Self::frequency(), convert::NANOS_PER_SECOND)
  }

  /// Converts this elapsed tick count to microseconds using the calibrated frequency.
  #[inline]
  #[must_use]
  pub fn as_micros(&self) -> u128 {
    convert::ticks_to_unit(self.0, Self::frequency(), convert::MICROS_PER_SECOND)
  }

  /// Converts this elapsed tick count to milliseconds using the calibrated frequency.
  #[inline]
  #[must_use]
  pub fn as_millis(&self) -> u128 {
    convert::ticks_to_unit(self.0, Self::frequency(), convert::MILLIS_PER_SECOND)
  }

  /// Converts this elapsed tick count to seconds as a floating-point value.
  #[inline]
  #[must_use]
  #[allow(clippy::cast_precision_loss)]
  pub fn as_secs_f64(&self) -> f64 {
    let freq = Self::frequency();
    self.0 as f64 / freq as f64
  }

  /// Converts this elapsed tick count to a [`Duration`], or [`None`] if it does not fit.
  #[inline]
  #[must_use]
  pub fn checked_duration(&self) -> Option<Duration> {
    convert::ticks_to_duration(self.0, Self::frequency())
  }

  /// Converts this elapsed tick count to a [`Duration`].
  ///
  /// This saturates at [`Duration::MAX`] when the calibrated conversion does not fit in
  /// `Duration`'s nanosecond storage.
  #[inline]
  #[must_use]
  pub fn as_duration(&self) -> Duration {
    convert::ticks_to_duration_saturating(self.0, Self::frequency())
  }

  /// Saturating subtraction. Returns `Ticks(0)` on underflow instead of wrapping.
  #[inline]
  pub const fn saturating_sub(&self, other: Self) -> Self {
    Self(self.0.saturating_sub(other.0))
  }

  /// Wrapping subtraction. Useful when the counter may have rolled over between samples.
  #[inline]
  pub const fn wrapping_sub(&self, other: Self) -> Self {
    Self(self.0.wrapping_sub(other.0))
  }

  /// Checked subtraction. Returns [`None`] on underflow.
  #[inline]
  #[must_use]
  pub const fn checked_sub(&self, other: Self) -> Option<Self> {
    match self.0.checked_sub(other.0) {
      Some(v) => Some(Self(v)),
      None => None,
    }
  }

  /// Saturating addition. Returns [`u64::MAX`] ticks on overflow.
  #[inline]
  pub const fn saturating_add(&self, other: Self) -> Self {
    Self(self.0.saturating_add(other.0))
  }

  /// Wrapping addition.
  #[inline]
  pub const fn wrapping_add(&self, other: Self) -> Self {
    Self(self.0.wrapping_add(other.0))
  }

  /// Checked addition. Returns [`None`] on overflow.
  #[inline]
  #[must_use]
  pub const fn checked_add(&self, other: Self) -> Option<Self> {
    match self.0.checked_add(other.0) {
      Some(v) => Some(Self(v)),
      None => None,
    }
  }
}

impl core::fmt::Display for Ticks {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "{} ticks", self.0)
  }
}

impl From<u64> for Ticks {
  #[inline]
  fn from(ticks: u64) -> Self {
    Self(ticks)
  }
}

impl From<Ticks> for u64 {
  #[inline]
  fn from(ticks: Ticks) -> Self {
    ticks.0
  }
}

impl core::ops::Add for Ticks {
  type Output = Self;

  #[inline]
  fn add(self, other: Self) -> Self {
    Self(self.0 + other.0)
  }
}

impl core::ops::AddAssign for Ticks {
  #[inline]
  fn add_assign(&mut self, other: Self) {
    self.0 += other.0;
  }
}

impl core::ops::Sub for Ticks {
  type Output = Self;

  #[inline]
  fn sub(self, other: Self) -> Self {
    Self(self.0 - other.0)
  }
}

impl core::ops::SubAssign for Ticks {
  #[inline]
  fn sub_assign(&mut self, other: Self) {
    self.0 -= other.0;
  }
}

impl core::ops::Mul<u64> for Ticks {
  type Output = Self;

  #[inline]
  fn mul(self, rhs: u64) -> Self {
    Self(self.0 * rhs)
  }
}

impl core::ops::Div<u64> for Ticks {
  type Output = Self;

  #[inline]
  fn div(self, rhs: u64) -> Self {
    Self(self.0 / rhs)
  }
}

impl core::iter::Sum for Ticks {
  fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
    iter.fold(Self(0), |acc, ticks| acc + ticks)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tick_unit_conversions_use_wide_integers() {
    let one_second = Ticks::from_raw(Ticks::frequency());

    assert_eq!(one_second.as_nanos(), 1_000_000_000);
    assert_eq!(one_second.as_micros(), 1_000_000);
    assert_eq!(one_second.as_millis(), 1_000);
    assert_eq!(one_second.as_duration(), Duration::from_secs(1));
    assert_eq!(one_second.checked_duration(), Some(Duration::from_secs(1)));
  }

  #[test]
  fn ticks_arithmetic() {
    let a = Ticks::from_raw(100);
    let b = Ticks::from_raw(50);

    assert_eq!(a + b, Ticks::from_raw(150));
    assert_eq!(a - b, Ticks::from_raw(50));
    assert_eq!(a * 2, Ticks::from_raw(200));
    assert_eq!(a / 2, Ticks::from_raw(50));
    assert_eq!(Ticks::from_raw(u64::MAX).wrapping_add(Ticks::from_raw(1)), Ticks::from_raw(0));
    assert_eq!(Ticks::from_raw(u64::MAX).checked_add(Ticks::from_raw(1)), None);
  }

  #[test]
  fn ticks_display() {
    let ticks = Ticks::from_raw(12345);
    assert_eq!(format!("{ticks}"), "12345 ticks");
  }
}
