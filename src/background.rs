//! Background recalibration thread. Compiled only with the
//! `recalibrate-background` Cargo feature, which **requires `std`** —
//! everything in this file uses `std::thread`, `std::sync::OnceLock`, and
//! `std::time::Duration`. The rest of the crate stays `#![no_std]`.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use crate::Instant;

const DEFAULT_INTERVAL_SECS: u64 = 60;

static INTERVAL_SECS: AtomicU64 = AtomicU64::new(DEFAULT_INTERVAL_SECS);
static THREAD: OnceLock<()> = OnceLock::new();

/// Configure the interval at which the background recalibration thread
/// runs. Takes effect on the next sleep cycle (so up to one current-interval
/// worth of delay before the change is observed).
///
/// Minimum is 1 second; smaller values are clamped up. Default is 60 seconds.
///
/// Available only with the `recalibrate-background` Cargo feature, which
/// **requires `std`**. The default tach build is `#![no_std]`; enabling this
/// feature is the only thing that promotes the crate to `std`.
pub fn set_recalibration_interval(interval: Duration) {
  let secs = interval.as_secs().max(1);
  INTERVAL_SECS.store(secs, Ordering::Relaxed);
}

pub(crate) fn ensure_thread() {
  THREAD.get_or_init(|| {
    let _ = thread::Builder::new()
      .name("tach-recalibrate".into())
      .spawn(|| {
        loop {
          let secs = INTERVAL_SECS.load(Ordering::Relaxed);
          thread::sleep(Duration::from_secs(secs));
          Instant::recalibrate();
        }
      });
  });
}
