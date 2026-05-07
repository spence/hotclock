//! Benchmark-only access to clock candidates.
//!
//! This module is intentionally hidden behind `bench-internals`; it exists to prove clock
//! selection decisions without freezing the candidate list as public API.

use crate::arch;
use crate::counter_eval::{CounterLatency, score_counter};

#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
#[path = "bench_internals/perf_rdpmc_x86_64_linux.rs"]
mod perf_rdpmc_x86_64_linux;

/// Clock source family used by benchmark-only candidate reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockCandidateKind {
  /// CPU or platform counter read directly by hotclock.
  Hardware,
  /// Operating-system timer used when direct counters are unavailable or fail validation.
  OsFallback,
}

impl ClockCandidateKind {
  #[must_use]
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Hardware => "hardware",
      Self::OsFallback => "os-fallback",
    }
  }
}

/// Single clock source that can be measured by benchmark tooling.
#[derive(Clone, Copy)]
pub struct ClockCandidate {
  /// Stable benchmark label.
  pub name: &'static str,
  /// Broad clock source family.
  pub kind: ClockCandidateKind,
  /// Whether production hotclock can use this source on the current target.
  pub selected_by_hotclock: bool,
  /// One-time setup that must complete before the timed read loop.
  pub prepare: Option<fn()>,
  /// Candidate may fault when unavailable and must only run in a crash-isolated child.
  pub requires_child_process: bool,
  /// Reads the raw counter value.
  pub read: fn() -> u64,
}

/// Validation and latency data for one candidate clock.
#[derive(Clone, Copy, Debug)]
pub struct ClockCandidateEvaluation {
  /// Candidate passed hotclock's runtime safety checks.
  pub valid: bool,
  /// Smallest observed positive counter delta while validating.
  pub precision_ticks: Option<u64>,
  /// Raw latency timing from repeated candidate reads.
  pub latency: ClockCandidateLatency,
}

/// Candidate read cost in nanoseconds per call.
#[derive(Clone, Copy, Debug)]
pub struct ClockCandidateLatency {
  pub best_ns: f64,
  pub mean_ns: f64,
  pub median_ns: f64,
  pub worst_ns: f64,
  pub stddev_ns: f64,
  pub ci95_low_ns: f64,
  pub ci95_high_ns: f64,
  pub samples: u64,
}

impl ClockCandidate {
  #[allow(dead_code)]
  const fn hardware(name: &'static str, selected_by_hotclock: bool, read: fn() -> u64) -> Self {
    Self {
      name,
      kind: ClockCandidateKind::Hardware,
      selected_by_hotclock,
      prepare: None,
      requires_child_process: false,
      read,
    }
  }

  #[allow(dead_code)]
  const fn prepared_hardware(
    name: &'static str,
    selected_by_hotclock: bool,
    prepare: fn(),
    read: fn() -> u64,
  ) -> Self {
    Self {
      name,
      kind: ClockCandidateKind::Hardware,
      selected_by_hotclock,
      prepare: Some(prepare),
      requires_child_process: false,
      read,
    }
  }

  #[allow(dead_code)]
  const fn crash_isolated_hardware(
    name: &'static str,
    selected_by_hotclock: bool,
    read: fn() -> u64,
  ) -> Self {
    Self {
      name,
      kind: ClockCandidateKind::Hardware,
      selected_by_hotclock,
      prepare: None,
      requires_child_process: true,
      read,
    }
  }

  const fn os_fallback(name: &'static str, selected_by_hotclock: bool, read: fn() -> u64) -> Self {
    Self {
      name,
      kind: ClockCandidateKind::OsFallback,
      selected_by_hotclock,
      prepare: None,
      requires_child_process: false,
      read,
    }
  }
}

macro_rules! candidates {
  ($($candidate:expr),+ $(,)?) => {{
    const CANDIDATES: &[ClockCandidate] = &[$($candidate),+];
    CANDIDATES
  }};
}

#[must_use]
pub fn candidate_clocks() -> &'static [ClockCandidate] {
  candidates()
}

#[must_use]
pub fn evaluate_candidate_clock(candidate: ClockCandidate) -> ClockCandidateEvaluation {
  if candidate.requires_child_process {
    return ClockCandidateEvaluation {
      valid: false,
      precision_ticks: None,
      latency: ClockCandidateLatency {
        best_ns: 0.0,
        mean_ns: 0.0,
        median_ns: 0.0,
        worst_ns: 0.0,
        stddev_ns: 0.0,
        ci95_low_ns: 0.0,
        ci95_high_ns: 0.0,
        samples: 0,
      },
    };
  }

  match score_counter(candidate.read) {
    Some(score) => ClockCandidateEvaluation {
      valid: true,
      precision_ticks: Some(score.validation.precision_ticks),
      latency: latency(score.latency),
    },
    None => ClockCandidateEvaluation {
      valid: false,
      precision_ticks: None,
      latency: ClockCandidateLatency {
        best_ns: 0.0,
        mean_ns: 0.0,
        median_ns: 0.0,
        worst_ns: 0.0,
        stddev_ns: 0.0,
        ci95_low_ns: 0.0,
        ci95_high_ns: 0.0,
        samples: 0,
      },
    },
  }
}

fn latency(latency: CounterLatency) -> ClockCandidateLatency {
  ClockCandidateLatency {
    best_ns: latency.best_ns_per_call(),
    mean_ns: latency.mean_ns_per_call(),
    median_ns: latency.median_ns_per_call(),
    worst_ns: latency.worst_ns_per_call(),
    stddev_ns: latency.stddev_ns_per_call(),
    ci95_low_ns: latency.ci95_low_ns_per_call(),
    ci95_high_ns: latency.ci95_high_ns_per_call(),
    samples: latency.samples,
  }
}

#[cfg(target_arch = "x86_64")]
fn candidates() -> &'static [ClockCandidate] {
  #[cfg(target_os = "macos")]
  {
    candidates![
      ClockCandidate::hardware("x86_64-rdtsc", true, arch::x86_64::rdtsc),
      ClockCandidate::os_fallback("macos-mach", true, macos::mach_time),
    ]
  }
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    candidates![
      ClockCandidate::hardware(
        "x86_64-rdpmc-fixed-core-cycles",
        false,
        perf_rdpmc_x86_64_linux::rdpmc_fixed_core_cycles_checked
      ),
      ClockCandidate::prepared_hardware(
        "x86_64-rdpmc-fixed-core-cycles-raw",
        false,
        perf_rdpmc_x86_64_linux::prepare_rdpmc_fixed_core_cycles,
        perf_rdpmc_x86_64_linux::rdpmc_fixed_core_cycles_raw
      ),
      ClockCandidate::crash_isolated_hardware(
        "x86_64-rdpmc-fixed-core-cycles-blind",
        false,
        perf_rdpmc_x86_64_linux::rdpmc_fixed_core_cycles_raw
      ),
      ClockCandidate::hardware(
        "x86_64-perf-rdpmc-cpu-cycles",
        false,
        perf_rdpmc_x86_64_linux::perf_rdpmc_cpu_cycles
      ),
      ClockCandidate::hardware("x86_64-rdtsc", true, arch::x86_64::rdtsc),
      ClockCandidate::hardware("x86_64-rdtscp", false, arch::x86_64::rdtscp),
      ClockCandidate::hardware("x86_64-lfence-rdtsc", false, arch::x86_64::lfence_rdtsc),
      ClockCandidate::os_fallback("unix-monotonic", true, arch::fallback::clock_monotonic),
      ClockCandidate::os_fallback("unix-monotonic-raw", false, arch::fallback::clock_monotonic_raw),
      ClockCandidate::os_fallback("linux-boottime", false, arch::fallback::clock_boottime),
      ClockCandidate::os_fallback(
        "linux-syscall-monotonic",
        false,
        arch::fallback::syscall_clock_monotonic
      ),
      ClockCandidate::os_fallback(
        "linux-syscall-monotonic-raw",
        false,
        arch::fallback::syscall_clock_monotonic_raw
      ),
      #[cfg(target_env = "gnu")]
      ClockCandidate::os_fallback(
        "linux-vdso-monotonic",
        false,
        arch::fallback::vdso_clock_monotonic
      ),
      #[cfg(target_env = "gnu")]
      ClockCandidate::os_fallback(
        "linux-vdso-monotonic-raw",
        false,
        arch::fallback::vdso_clock_monotonic_raw
      ),
    ]
  }
  #[cfg(not(unix))]
  {
    candidates![
      ClockCandidate::hardware("x86_64-rdtsc", true, arch::x86_64::rdtsc),
      ClockCandidate::os_fallback("std-instant", true, arch::fallback::instant_elapsed),
    ]
  }
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn candidates() -> &'static [ClockCandidate] {
  candidates![
    ClockCandidate::crash_isolated_hardware(
      "aarch64-pmccntr-el0-blind",
      false,
      arch::aarch64::pmccntr_el0
    ),
    ClockCandidate::hardware("aarch64-cntvct", true, arch::aarch64::cntvct),
    ClockCandidate::os_fallback("unix-monotonic", true, arch::fallback::clock_monotonic),
    ClockCandidate::os_fallback("unix-monotonic-raw", false, arch::fallback::clock_monotonic_raw),
    ClockCandidate::os_fallback("linux-boottime", false, arch::fallback::clock_boottime),
    ClockCandidate::os_fallback(
      "linux-syscall-monotonic",
      false,
      arch::fallback::syscall_clock_monotonic
    ),
    ClockCandidate::os_fallback(
      "linux-syscall-monotonic-raw",
      false,
      arch::fallback::syscall_clock_monotonic_raw
    ),
    #[cfg(target_env = "gnu")]
    ClockCandidate::os_fallback(
      "linux-vdso-monotonic",
      false,
      arch::fallback::vdso_clock_monotonic
    ),
    #[cfg(target_env = "gnu")]
    ClockCandidate::os_fallback(
      "linux-vdso-monotonic-raw",
      false,
      arch::fallback::vdso_clock_monotonic_raw
    ),
  ]
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
fn candidates() -> &'static [ClockCandidate] {
  candidates![
    ClockCandidate::hardware("aarch64-cntvct", true, arch::aarch64::cntvct),
    ClockCandidate::os_fallback("macos-mach", false, macos::mach_time),
  ]
}

#[cfg(all(target_arch = "aarch64", not(any(target_os = "linux", target_os = "macos"))))]
fn candidates() -> &'static [ClockCandidate] {
  candidates![
    ClockCandidate::hardware("aarch64-cntvct", true, arch::aarch64::cntvct),
    ClockCandidate::os_fallback("std-instant", true, arch::fallback::instant_elapsed),
  ]
}

#[cfg(target_arch = "x86")]
fn candidates() -> &'static [ClockCandidate] {
  #[cfg(target_os = "macos")]
  {
    candidates![
      ClockCandidate::hardware("x86-rdtsc", true, arch::x86::rdtsc),
      ClockCandidate::os_fallback("macos-mach", true, macos::mach_time),
    ]
  }
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    candidates![
      ClockCandidate::crash_isolated_hardware(
        "x86-rdpmc-fixed-core-cycles-blind",
        false,
        arch::x86::rdpmc_fixed_core_cycles
      ),
      ClockCandidate::hardware("x86-rdtsc", true, arch::x86::rdtsc),
      ClockCandidate::hardware("x86-rdtscp", false, arch::x86::rdtscp),
      ClockCandidate::hardware("x86-lfence-rdtsc", false, arch::x86::lfence_rdtsc),
      ClockCandidate::os_fallback("unix-monotonic", true, arch::fallback::clock_monotonic),
      ClockCandidate::os_fallback("unix-monotonic-raw", false, arch::fallback::clock_monotonic_raw),
      ClockCandidate::os_fallback("linux-boottime", false, arch::fallback::clock_boottime),
      ClockCandidate::os_fallback(
        "linux-syscall-monotonic",
        false,
        arch::fallback::syscall_clock_monotonic
      ),
      ClockCandidate::os_fallback(
        "linux-syscall-monotonic-raw",
        false,
        arch::fallback::syscall_clock_monotonic_raw
      ),
      #[cfg(target_env = "gnu")]
      ClockCandidate::os_fallback(
        "linux-vdso-monotonic",
        false,
        arch::fallback::vdso_clock_monotonic
      ),
      #[cfg(target_env = "gnu")]
      ClockCandidate::os_fallback(
        "linux-vdso-monotonic-raw",
        false,
        arch::fallback::vdso_clock_monotonic_raw
      ),
    ]
  }
  #[cfg(not(unix))]
  {
    candidates![
      ClockCandidate::hardware("x86-rdtsc", true, arch::x86::rdtsc),
      ClockCandidate::os_fallback("std-instant", true, arch::fallback::instant_elapsed),
    ]
  }
}

#[cfg(target_arch = "riscv64")]
fn candidates() -> &'static [ClockCandidate] {
  #[cfg(unix)]
  {
    candidates![
      ClockCandidate::hardware("riscv64-rdcycle", true, arch::riscv64::rdcycle),
      ClockCandidate::os_fallback("unix-monotonic", true, arch::fallback::clock_monotonic),
    ]
  }
  #[cfg(not(unix))]
  {
    candidates![
      ClockCandidate::hardware("riscv64-rdcycle", true, arch::riscv64::rdcycle),
      ClockCandidate::os_fallback("std-instant", true, arch::fallback::instant_elapsed),
    ]
  }
}

#[cfg(target_arch = "powerpc64")]
fn candidates() -> &'static [ClockCandidate] {
  #[cfg(unix)]
  {
    candidates![
      ClockCandidate::hardware("powerpc64-mftb", true, arch::powerpc64::mftb),
      ClockCandidate::os_fallback("unix-monotonic", true, arch::fallback::clock_monotonic),
    ]
  }
  #[cfg(not(unix))]
  {
    candidates![
      ClockCandidate::hardware("powerpc64-mftb", true, arch::powerpc64::mftb),
      ClockCandidate::os_fallback("std-instant", true, arch::fallback::instant_elapsed),
    ]
  }
}

#[cfg(target_arch = "s390x")]
fn candidates() -> &'static [ClockCandidate] {
  #[cfg(unix)]
  {
    candidates![
      ClockCandidate::hardware("s390x-stckf", true, arch::s390x::stckf),
      ClockCandidate::os_fallback("unix-monotonic", true, arch::fallback::clock_monotonic),
    ]
  }
  #[cfg(not(unix))]
  {
    candidates![
      ClockCandidate::hardware("s390x-stckf", true, arch::s390x::stckf),
      ClockCandidate::os_fallback("std-instant", true, arch::fallback::instant_elapsed),
    ]
  }
}

#[cfg(target_arch = "loongarch64")]
fn candidates() -> &'static [ClockCandidate] {
  #[cfg(unix)]
  {
    candidates![
      ClockCandidate::hardware("loongarch64-rdtime", true, arch::loongarch64::rdtime),
      ClockCandidate::os_fallback("unix-monotonic", true, arch::fallback::clock_monotonic),
    ]
  }
  #[cfg(not(unix))]
  {
    candidates![
      ClockCandidate::hardware("loongarch64-rdtime", true, arch::loongarch64::rdtime),
      ClockCandidate::os_fallback("std-instant", true, arch::fallback::instant_elapsed),
    ]
  }
}

#[cfg(not(any(
  target_arch = "x86_64",
  target_arch = "x86",
  target_arch = "aarch64",
  target_arch = "riscv64",
  target_arch = "powerpc64",
  target_arch = "s390x",
  target_arch = "loongarch64",
)))]
fn candidates() -> &'static [ClockCandidate] {
  #[cfg(target_os = "macos")]
  {
    candidates![ClockCandidate::os_fallback("macos-mach", true, macos::mach_time)]
  }
  #[cfg(all(unix, not(target_os = "macos")))]
  {
    candidates![ClockCandidate::os_fallback(
      "unix-monotonic",
      true,
      arch::fallback::clock_monotonic
    )]
  }
  #[cfg(not(unix))]
  {
    candidates![ClockCandidate::os_fallback("std-instant", true, arch::fallback::instant_elapsed)]
  }
}

#[cfg(target_os = "macos")]
mod macos {
  unsafe extern "C" {
    fn mach_absolute_time() -> u64;
  }

  #[inline(always)]
  pub fn mach_time() -> u64 {
    // SAFETY: `mach_absolute_time` is a zero-argument monotonic host counter read.
    unsafe { mach_absolute_time() }
  }
}
