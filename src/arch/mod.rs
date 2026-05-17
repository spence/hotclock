use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(target_arch = "aarch64")]
pub mod aarch64;
pub mod fallback;
#[cfg(target_arch = "loongarch64")]
pub mod loongarch64;
#[cfg(target_arch = "riscv64")]
pub mod riscv64;
#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
pub mod wasm;
#[cfg(target_arch = "x86")]
pub mod x86;
#[cfg(target_arch = "x86_64")]
pub mod x86_64;

mod direct;
pub use direct::{ticks, ticks_ordered};

// Cached at first elapsed() call. Stored as fixed-point Q32:
//   nanos_per_tick_q32 = (1e9 << 32) / frequency
// Then converting ticks to nanos becomes (ticks * scale) >> 32, replacing
// the per-call u128 division with a multiply + shift.
static NANOS_PER_TICK_Q32: AtomicU64 = AtomicU64::new(0);

const NANOS_PER_SECOND_Q32: u128 = 1_000_000_000u128 << 32;

#[inline]
#[must_use]
pub fn nanos_per_tick_q32() -> u64 {
  let cached = NANOS_PER_TICK_Q32.load(Ordering::Relaxed);
  if cached != 0 {
    return cached;
  }
  let freq = read_frequency();
  let scale = u64::try_from(NANOS_PER_SECOND_Q32 / u128::from(freq)).unwrap_or(u64::MAX);
  NANOS_PER_TICK_Q32.store(scale, Ordering::Relaxed);

  // Spawn the periodic recalibration thread on first use. Only compiled
  // when the `recalibrate-background` feature is enabled — which is the
  // only feature that pulls in `std`.
  #[cfg(feature = "recalibrate-background")]
  crate::background::ensure_thread();

  scale
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn read_frequency() -> u64 {
  aarch64::cntfrq()
}

#[cfg(all(not(target_arch = "aarch64"), target_os = "macos"))]
#[inline]
fn read_frequency() -> u64 {
  // mach_timebase_info reports (numer, denom) such that
  //   nanoseconds = ticks * numer / denom
  // so the effective tick rate is 1e9 * denom / numer Hz.
  #[repr(C)]
  struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
  }
  unsafe extern "C" {
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
  }
  let mut info = MachTimebaseInfo { numer: 0, denom: 0 };
  // SAFETY: `mach_timebase_info` populates the struct.
  unsafe { mach_timebase_info(&mut info) };
  1_000_000_000u64 * u64::from(info.denom) / u64::from(info.numer)
}

#[cfg(all(not(target_arch = "aarch64"), target_os = "windows"))]
#[inline]
fn read_frequency() -> u64 {
  unsafe extern "system" {
    fn QueryPerformanceFrequency(lpFrequency: *mut i64) -> i32;
  }
  let mut freq: i64 = 0;
  // SAFETY: `QueryPerformanceFrequency` writes a single i64.
  unsafe { QueryPerformanceFrequency(&mut freq) };
  freq as u64
}

#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
#[inline]
fn read_frequency() -> u64 {
  // ticks() returns nanos directly; identity Q32 transform.
  1_000_000_000
}

#[cfg(target_os = "wasi")]
#[inline]
fn read_frequency() -> u64 {
  // ticks() returns nanos from clock_time_get directly; identity transform.
  1_000_000_000
}

// x86 (non-macOS, non-Windows): prefer CPUID leaf 15h when available — modern
// Intel (Skylake+) and AMD (Zen2+) expose the exact architectural TSC
// frequency, eliminating the ~500 ppm error baked into spin-loop calibration.
// Fall back to calibration on older / virtualized CPUs that zero the leaf.
#[cfg(all(
  any(target_arch = "x86_64", target_arch = "x86"),
  not(any(target_os = "macos", target_os = "windows")),
))]
#[inline]
fn read_frequency() -> u64 {
  #[cfg(target_arch = "x86_64")]
  if let Some(hz) = x86_64::cpuid_tsc_hz() {
    return hz;
  }
  #[cfg(target_arch = "x86")]
  if let Some(hz) = x86::cpuid_tsc_hz() {
    return hz;
  }
  crate::calibration::calibrate_frequency()
}

#[cfg(not(any(
  target_arch = "aarch64",
  target_arch = "x86_64",
  target_arch = "x86",
  target_os = "macos",
  target_os = "windows",
  target_os = "wasi",
  all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
)))]
#[inline]
fn read_frequency() -> u64 {
  crate::calibration::calibrate_frequency()
}

/// Re-derive the tick-to-nanosecond scaling against the platform monotonic
/// clock and atomically replace the cached Q32 reciprocal. The next
/// `ticks_to_duration` call observes the new value via the Acquire/Relaxed
/// load in `nanos_per_tick_q32`.
///
/// On platforms where the frequency comes from an authoritative register or
/// OS API (aarch64 `cntfrq_el0`, macOS `mach_timebase_info`, Windows
/// `QueryPerformanceFrequency`, WASI / wasm fixed at 1 GHz), this is a
/// no-op — re-reading would just yield the same value.
///
/// `recalibrate` is `#![no_std]`-compatible; it uses the same spin-loop +
/// `clock_gettime` path that already runs at startup.
pub fn recalibrate() {
  #[cfg(all(
    any(target_arch = "x86_64", target_arch = "x86"),
    not(any(target_os = "macos", target_os = "windows")),
  ))]
  {
    // Skip CPUID 15h: it reports *nominal* frequency, which doesn't change.
    // Recalibration's job is to track *actual* frequency drift over uptime,
    // so we go straight to clock_gettime-based spin-loop calibration.
    let new_hz = crate::calibration::calibrate_frequency();
    if new_hz > 0 {
      let scale =
        u64::try_from(NANOS_PER_SECOND_Q32 / u128::from(new_hz)).unwrap_or(u64::MAX);
      NANOS_PER_TICK_Q32.store(scale, Ordering::Release);
    }
  }
  #[cfg(not(all(
    any(target_arch = "x86_64", target_arch = "x86"),
    not(any(target_os = "macos", target_os = "windows")),
  )))]
  {
    // No-op on platforms where the frequency source is authoritative.
  }
}
