use std::time::Duration;

pub(crate) const NANOS_PER_SECOND: u128 = 1_000_000_000;
pub(crate) const MICROS_PER_SECOND: u128 = 1_000_000;
pub(crate) const MILLIS_PER_SECOND: u128 = 1_000;

#[inline]
pub(crate) fn ticks_to_unit(ticks: u64, frequency: u64, units_per_second: u128) -> u128 {
  u128::from(ticks) * units_per_second / u128::from(frequency)
}

#[inline]
pub(crate) fn ticks_to_duration(ticks: u64, frequency: u64) -> Option<Duration> {
  nanos_to_duration(ticks_to_unit(ticks, frequency, NANOS_PER_SECOND))
}

#[inline]
pub(crate) fn ticks_to_duration_saturating(ticks: u64, frequency: u64) -> Duration {
  let nanos = ticks_to_unit(ticks, frequency, NANOS_PER_SECOND);
  nanos_to_duration(nanos).unwrap_or(Duration::MAX)
}

#[inline]
pub(crate) fn duration_to_ticks(duration: Duration, frequency: u64) -> Option<u64> {
  let product = duration.as_nanos().checked_mul(u128::from(frequency))?;
  let ticks = product / NANOS_PER_SECOND + u128::from(product % NANOS_PER_SECOND != 0);
  u64::try_from(ticks).ok()
}

#[inline]
fn nanos_to_duration(nanos: u128) -> Option<Duration> {
  let secs = u64::try_from(nanos / NANOS_PER_SECOND).ok()?;
  let subsec_nanos = u32::try_from(nanos % NANOS_PER_SECOND).ok()?;
  Some(Duration::new(secs, subsec_nanos))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn duration_conversion_reports_overflow() {
    let too_many_nanos = u128::from(u64::MAX) * NANOS_PER_SECOND + NANOS_PER_SECOND;

    assert_eq!(nanos_to_duration(too_many_nanos), None);
  }

  #[test]
  fn duration_to_ticks_rounds_up() {
    assert_eq!(duration_to_ticks(Duration::from_nanos(1), 500_000_000), Some(1));
  }
}
