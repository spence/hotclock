//! Cross-platform CPU cycle/tick counter with runtime auto-selection.
//!
//! A Rust port of [libcpucycles](https://cpucycles.cr.yp.to/) that provides sub-nanosecond
//! timing by directly reading hardware counters (RDTSC, CNTVCT\_EL0, etc.). The best available
//! counter is selected automatically at process startup via static initializers.
//!
//! Roughly **~30x faster** than [`std::time::Instant`] on typical hardware.
//!
//! # Quick start
//!
//! ```
//! use cputicks::Ticks;
//!
//! let start = Ticks::now();
//! // ... do some work ...
//! let elapsed = start.elapsed();
//! println!("Elapsed: {:?}", elapsed.as_duration());
//! ```
//!
//! # Platform support
//!
//! | Architecture    | Primary        | Fallback       |
//! |-----------------|----------------|----------------|
//! | x86\_64         | RDTSC          | OS timer       |
//! | x86             | RDTSC          | OS timer       |
//! | aarch64         | CNTVCT\_EL0    | OS timer       |
//! | aarch64 (Linux) | PMCCNTR\_EL0   | CNTVCT\_EL0    |
//! | riscv64         | rdcycle        | OS timer       |
//! | powerpc64       | mftb           | OS timer       |
//! | s390x           | stckf          | OS timer       |
//! | loongarch64     | rdtime.d       | OS timer       |
//! | other           | —              | OS timer       |
//!
//! OS timers: `mach_absolute_time` (macOS), `clock_gettime` (Unix), [`Instant`](std::time::Instant) (other).
//!
//! # Frequency calibration
//!
//! [`Ticks::frequency()`] is lazily calibrated on first call by measuring tick rate against
//! the system clock. The result is cached for the lifetime of the process. Time-conversion
//! methods ([`as_nanos`](Ticks::as_nanos), [`as_duration`](Ticks::as_duration), etc.) call
//! `frequency()` internally and therefore trigger calibration on first use.

mod arch;
mod selection;

/// A lightweight wrapper around a raw hardware tick count.
///
/// `Ticks` is a `#[repr(transparent)]` newtype over [`u64`], so it has zero runtime cost
/// beyond the underlying counter read. Values are comparable, hashable, and support basic
/// arithmetic (`+`, `-`, `*`, `/`, [`Sum`](core::iter::Sum)).
///
/// # Obtaining ticks
///
/// Use [`Ticks::now()`] to sample the current counter. Use [`elapsed()`](Ticks::elapsed) or
/// subtraction to compute intervals:
///
/// ```
/// use cputicks::Ticks;
///
/// let start = Ticks::now();
/// // ... work ...
/// let delta = start.elapsed();        // or: Ticks::now() - start
/// println!("{} ns", delta.as_nanos());
/// ```
///
/// # Converting to wall-clock time
///
/// [`as_nanos`](Ticks::as_nanos), [`as_micros`](Ticks::as_micros),
/// [`as_millis`](Ticks::as_millis), [`as_secs_f64`](Ticks::as_secs_f64), and
/// [`as_duration`](Ticks::as_duration) all divide by the counter frequency. The frequency
/// is calibrated lazily on first call and cached thereafter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct Ticks(pub u64);

impl Ticks {
  /// Reads the current value of the selected hardware tick counter.
  ///
  /// This is an extremely lightweight operation — typically a single inline assembly
  /// instruction (e.g. `rdtsc` on x86\_64, `mrs cntvct_el0` on aarch64).
  ///
  /// # Example
  ///
  /// ```
  /// use cputicks::Ticks;
  /// let t = Ticks::now();
  /// assert!(t.as_raw() > 0);
  /// ```
  #[inline(always)]
  pub fn now() -> Self {
    Ticks(arch::ticks())
  }

  /// Returns the number of ticks that have elapsed since `self` was sampled.
  ///
  /// Equivalent to `Ticks::now() - self`, but uses wrapping subtraction to
  /// handle counter rollover gracefully.
  ///
  /// # Example
  ///
  /// ```
  /// use cputicks::Ticks;
  /// let start = Ticks::now();
  /// // ... work ...
  /// let elapsed = start.elapsed();
  /// println!("{:?}", elapsed.as_duration());
  /// ```
  #[inline(always)]
  pub fn elapsed(&self) -> Self {
    Ticks(arch::ticks().wrapping_sub(self.0))
  }

  /// Returns the tick counter frequency in Hz.
  ///
  /// Calibrated lazily on first call by measuring the tick rate against the system clock
  /// over a short spin-wait. The result is cached for the lifetime of the process.
  ///
  /// # Example
  ///
  /// ```
  /// use cputicks::Ticks;
  /// let freq = Ticks::frequency();
  /// println!("{:.2} MHz", freq as f64 / 1e6);
  /// ```
  #[inline]
  pub fn frequency() -> u64 {
    arch::frequency()
  }

  /// Returns the name of the selected counter implementation (e.g. `"aarch64-cntvct"`,
  /// `"x86_64-rdtsc"`, `"macos-mach"`).
  ///
  /// Useful for diagnostics and logging.
  #[inline]
  pub fn implementation() -> &'static str {
    arch::implementation()
  }

  /// Converts the tick count to nanoseconds using the calibrated frequency.
  #[inline]
  pub fn as_nanos(&self) -> u64 {
    let freq = Self::frequency();
    (self.0 as u128 * 1_000_000_000 / freq as u128) as u64
  }

  /// Converts the tick count to microseconds using the calibrated frequency.
  #[inline]
  pub fn as_micros(&self) -> u64 {
    let freq = Self::frequency();
    (self.0 as u128 * 1_000_000 / freq as u128) as u64
  }

  /// Converts the tick count to milliseconds using the calibrated frequency.
  #[inline]
  pub fn as_millis(&self) -> u64 {
    let freq = Self::frequency();
    (self.0 as u128 * 1_000 / freq as u128) as u64
  }

  /// Converts the tick count to seconds as a floating-point value.
  #[inline]
  pub fn as_secs_f64(&self) -> f64 {
    let freq = Self::frequency();
    self.0 as f64 / freq as f64
  }

  /// Converts the tick count to a [`std::time::Duration`].
  ///
  /// This is a convenience wrapper around [`as_nanos`](Ticks::as_nanos).
  #[inline]
  pub fn as_duration(&self) -> std::time::Duration {
    std::time::Duration::from_nanos(self.as_nanos())
  }

  /// Creates a `Ticks` value from a raw `u64` counter reading.
  #[inline]
  pub const fn from_raw(ticks: u64) -> Self {
    Ticks(ticks)
  }

  /// Returns the underlying raw tick count.
  #[inline]
  pub const fn as_raw(&self) -> u64 {
    self.0
  }

  /// Returns `true` if the tick count is zero.
  #[inline]
  pub const fn is_zero(&self) -> bool {
    self.0 == 0
  }

  /// Saturating subtraction. Returns `Ticks(0)` on underflow instead of wrapping.
  #[inline]
  pub const fn saturating_sub(&self, other: Self) -> Self {
    Ticks(self.0.saturating_sub(other.0))
  }

  /// Wrapping subtraction. Useful when the counter may have rolled over between samples.
  #[inline]
  pub const fn wrapping_sub(&self, other: Self) -> Self {
    Ticks(self.0.wrapping_sub(other.0))
  }

  /// Checked subtraction. Returns [`None`] on underflow.
  #[inline]
  pub const fn checked_sub(&self, other: Self) -> Option<Self> {
    match self.0.checked_sub(other.0) {
      Some(v) => Some(Ticks(v)),
      None => None,
    }
  }

  /// Saturating addition. Returns [`u64::MAX`] ticks on overflow.
  #[inline]
  pub const fn saturating_add(&self, other: Self) -> Self {
    Ticks(self.0.saturating_add(other.0))
  }

  /// Resets this value to the current tick counter, equivalent to `*self = Ticks::now()`.
  #[inline]
  pub fn reset(&mut self) {
    self.0 = arch::ticks();
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
    Ticks(ticks)
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
    Ticks(self.0 + other.0)
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
    Ticks(self.0 - other.0)
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
    Ticks(self.0 * rhs)
  }
}

impl core::ops::Div<u64> for Ticks {
  type Output = Self;

  #[inline]
  fn div(self, rhs: u64) -> Self {
    Ticks(self.0 / rhs)
  }
}

impl core::iter::Sum for Ticks {
  fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
    iter.fold(Ticks(0), |acc, x| acc + x)
  }
}

/// Returns the tick counter frequency in Hz.
///
/// This is the free-function form of [`Ticks::frequency()`]. See that method for details.
pub use arch::frequency;
/// Returns the name of the selected counter implementation.
///
/// This is the free-function form of [`Ticks::implementation()`]. See that method for details.
pub use arch::implementation;

#[cfg(all(test))]
mod tests {
  use super::*;

  #[test]
  fn test_ticks_now() {
    let c = Ticks::now();
    assert!(c.0 > 0);
  }

  #[test]
  fn test_ticks_elapsed() {
    let start = Ticks::now();
    let mut sum = 0u64;
    for i in 0..1000 {
      sum = sum.wrapping_add(i);
    }
    let _ = std::hint::black_box(sum);
    let elapsed = start.elapsed();
    assert!(elapsed.0 > 0);
  }

  #[test]
  fn test_ticks_monotonic() {
    let a = Ticks::now();
    let b = Ticks::now();
    let c = Ticks::now();
    // Should be monotonically increasing (or at least not decreasing much)
    assert!(b.0 >= a.0 || a.0.wrapping_sub(b.0) < 1000);
    assert!(c.0 >= b.0 || b.0.wrapping_sub(c.0) < 1000);
  }

  #[test]
  fn test_implementation() {
    let impl_name = Ticks::implementation();
    assert!(!impl_name.is_empty());
    println!("Implementation: {}", impl_name);
  }

  #[test]
  fn test_frequency() {
    let freq = Ticks::frequency();
    println!("Frequency: {} Hz ({:.2} MHz)", freq, freq as f64 / 1e6);
    // Sanity check: between 1 MHz and 100 GHz
    assert!(freq >= 1_000_000);
    assert!(freq <= 100_000_000_000);
  }

  #[test]
  fn test_ticks_arithmetic() {
    let a = Ticks(100);
    let b = Ticks(50);

    assert_eq!(a + b, Ticks(150));
    assert_eq!(a - b, Ticks(50));
    assert_eq!(a * 2, Ticks(200));
    assert_eq!(a / 2, Ticks(50));
  }

  #[test]
  fn test_ticks_display() {
    let c = Ticks(12345);
    assert_eq!(format!("{}", c), "12345 ticks");
  }

  #[test]
  fn test_as_duration() {
    let start = Ticks::now();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let elapsed = start.elapsed();

    let duration = elapsed.as_duration();
    println!("Slept for {:?}", duration);
    // Should be at least 10ms
    assert!(duration.as_millis() >= 9);
    // But not more than 100ms (allowing for scheduler delays)
    assert!(duration.as_millis() < 100);
  }
}
