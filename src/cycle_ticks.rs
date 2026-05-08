use std::time::Duration;

use crate::{arch, convert};

/// An elapsed raw cycle-counter delta from [`crate::Cycles`].
///
/// `CycleTicks` is the delta type for [`crate::Cycles`]. It converts through the cycle-counter
/// frequency selected for the current process. On some targets this is the same machine counter
/// used by [`crate::Instant`]; on targets with a faster PMU/core-cycle path it can use a
/// different frequency.
///
/// ```
/// use hotclock::Cycles;
///
/// let ticks = Cycles::now().elapsed_ticks();
/// println!("{} ns", ticks.as_nanos());
/// ```
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct CycleTicks(u64);

impl CycleTicks {
  /// Returns the selected cycle-counter frequency in Hz.
  #[inline]
  #[must_use]
  pub fn frequency() -> u64 {
    arch::cycle_frequency()
  }

  /// Creates a `CycleTicks` value from a raw `u64` counter delta.
  #[inline]
  pub const fn from_raw(ticks: u64) -> Self {
    Self(ticks)
  }

  /// Returns the underlying raw counter delta.
  #[inline]
  #[must_use]
  pub const fn as_raw(&self) -> u64 {
    self.0
  }

  /// Returns `true` if the delta is zero.
  #[inline]
  #[must_use]
  pub const fn is_zero(&self) -> bool {
    self.0 == 0
  }

  /// Converts this elapsed delta to nanoseconds using the calibrated cycle frequency.
  #[inline]
  #[must_use]
  pub fn as_nanos(&self) -> u128 {
    convert::ticks_to_unit(self.0, Self::frequency(), convert::NANOS_PER_SECOND)
  }

  /// Converts this elapsed delta to microseconds using the calibrated cycle frequency.
  #[inline]
  #[must_use]
  pub fn as_micros(&self) -> u128 {
    convert::ticks_to_unit(self.0, Self::frequency(), convert::MICROS_PER_SECOND)
  }

  /// Converts this elapsed delta to milliseconds using the calibrated cycle frequency.
  #[inline]
  #[must_use]
  pub fn as_millis(&self) -> u128 {
    convert::ticks_to_unit(self.0, Self::frequency(), convert::MILLIS_PER_SECOND)
  }

  /// Converts this elapsed delta to seconds as a floating-point value.
  #[inline]
  #[must_use]
  #[allow(clippy::cast_precision_loss)]
  pub fn as_secs_f64(&self) -> f64 {
    let freq = Self::frequency();
    self.0 as f64 / freq as f64
  }

  /// Converts this elapsed delta to a [`Duration`], or [`None`] if it does not fit.
  #[inline]
  #[must_use]
  pub fn checked_duration(&self) -> Option<Duration> {
    convert::ticks_to_duration(self.0, Self::frequency())
  }

  /// Converts this elapsed delta to a [`Duration`], saturating on overflow.
  #[inline]
  #[must_use]
  pub fn as_duration(&self) -> Duration {
    convert::ticks_to_duration_saturating(self.0, Self::frequency())
  }

  /// Saturating subtraction. Returns `CycleTicks(0)` on underflow.
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

impl core::fmt::Display for CycleTicks {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "{} cycles", self.0)
  }
}

impl From<u64> for CycleTicks {
  #[inline]
  fn from(ticks: u64) -> Self {
    Self(ticks)
  }
}

impl From<CycleTicks> for u64 {
  #[inline]
  fn from(ticks: CycleTicks) -> Self {
    ticks.0
  }
}

impl core::ops::Add for CycleTicks {
  type Output = Self;

  #[inline]
  fn add(self, other: Self) -> Self {
    Self(self.0 + other.0)
  }
}

impl core::ops::AddAssign for CycleTicks {
  #[inline]
  fn add_assign(&mut self, other: Self) {
    self.0 += other.0;
  }
}

impl core::ops::Sub for CycleTicks {
  type Output = Self;

  #[inline]
  fn sub(self, other: Self) -> Self {
    Self(self.0 - other.0)
  }
}

impl core::ops::SubAssign for CycleTicks {
  #[inline]
  fn sub_assign(&mut self, other: Self) {
    self.0 -= other.0;
  }
}

impl core::ops::Mul<u64> for CycleTicks {
  type Output = Self;

  #[inline]
  fn mul(self, rhs: u64) -> Self {
    Self(self.0 * rhs)
  }
}

impl core::ops::Div<u64> for CycleTicks {
  type Output = Self;

  #[inline]
  fn div(self, rhs: u64) -> Self {
    Self(self.0 / rhs)
  }
}

impl core::iter::Sum for CycleTicks {
  fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
    iter.fold(Self(0), |acc, ticks| acc + ticks)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn cycle_tick_conversions_use_wide_integers() {
    let one_second = CycleTicks::from_raw(CycleTicks::frequency());

    assert_eq!(one_second.as_nanos(), 1_000_000_000);
    assert_eq!(one_second.as_micros(), 1_000_000);
    assert_eq!(one_second.as_millis(), 1_000);
    assert_eq!(one_second.as_duration(), Duration::from_secs(1));
    assert_eq!(one_second.checked_duration(), Some(Duration::from_secs(1)));
  }

  #[test]
  fn cycle_ticks_arithmetic() {
    let a = CycleTicks::from_raw(100);
    let b = CycleTicks::from_raw(50);

    assert_eq!(a + b, CycleTicks::from_raw(150));
    assert_eq!(a - b, CycleTicks::from_raw(50));
    assert_eq!(a * 2, CycleTicks::from_raw(200));
    assert_eq!(a / 2, CycleTicks::from_raw(50));
    assert_eq!(
      CycleTicks::from_raw(u64::MAX).wrapping_add(CycleTicks::from_raw(1)),
      CycleTicks::from_raw(0)
    );
    assert_eq!(CycleTicks::from_raw(u64::MAX).checked_add(CycleTicks::from_raw(1)), None);
  }
}
