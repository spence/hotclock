#![allow(clippy::inline_always)]

use std::env;
use std::hint::black_box;
use std::process::Command;
#[cfg(not(unix))]
use std::sync::OnceLock;
use std::time::Instant as StdInstant;

use hotclock::Instant;

const CHILD_ENV: &str = "HOTCLOCK_CLOCK_VALIDATION_CHILD";
const DEFAULT_COLD_SAMPLES: usize = 9;
const DEFAULT_STEADY_SAMPLES: usize = 7;
const DEFAULT_ITERS: u64 = 1_000_000;
const DEFAULT_MAX_RATIO: f64 = 1.35;
const DEFAULT_MAX_DELTA_NS: f64 = 3.0;

type LoopFn = fn(u64) -> u64;

#[derive(Clone, Copy)]
struct Baseline {
  name: &'static str,
  loop_fn: LoopFn,
}

#[derive(Clone, Copy)]
struct Measurement {
  best_ns: f64,
  median_ns: f64,
}

fn main() {
  if env::var_os(CHILD_ENV).is_some() {
    run_child();
    return;
  }

  let cold_samples = env_usize("HOTCLOCK_CLOCK_VALIDATION_COLD_SAMPLES", DEFAULT_COLD_SAMPLES);
  let steady_samples =
    env_usize("HOTCLOCK_CLOCK_VALIDATION_STEADY_SAMPLES", DEFAULT_STEADY_SAMPLES);
  let iters = env_u64("HOTCLOCK_CLOCK_VALIDATION_ITERS", DEFAULT_ITERS);
  let max_ratio = env_f64("HOTCLOCK_CLOCK_VALIDATION_MAX_RATIO", DEFAULT_MAX_RATIO);
  let max_delta_ns = env_f64("HOTCLOCK_CLOCK_VALIDATION_MAX_DELTA_NS", DEFAULT_MAX_DELTA_NS);

  let cold = cold_measurements(cold_samples);
  let first = Instant::now();
  black_box(first.as_raw());
  let selected = Instant::implementation();
  let Some(baseline) = baseline_for(selected) else {
    eprintln!("unsupported selected clock for benchmark validation: {selected}");
    std::process::exit(2);
  };

  black_box(run_hotclock_loop(1));
  black_box((baseline.loop_fn)(1));
  black_box(run_noop_loop(1));

  let loop_overhead = measure_loop(run_noop_loop, iters, steady_samples);
  let hotclock = measure_loop(run_hotclock_loop, iters, steady_samples);
  let baseline_measurement = measure_loop(baseline.loop_fn, iters, steady_samples);

  let hotclock_net_best = subtract_ns(hotclock.best_ns, loop_overhead.best_ns);
  let hotclock_net_median = subtract_ns(hotclock.median_ns, loop_overhead.median_ns);
  let baseline_net_best = subtract_ns(baseline_measurement.best_ns, loop_overhead.best_ns);
  let baseline_net_median = subtract_ns(baseline_measurement.median_ns, loop_overhead.median_ns);
  let ratio = hotclock.best_ns / baseline_measurement.best_ns.max(0.001);
  let allowed = baseline_measurement.best_ns.mul_add(max_ratio, max_delta_ns);

  print_report(Report {
    selected,
    baseline: baseline.name,
    cold,
    iters,
    steady_samples,
    loop_overhead,
    hotclock,
    baseline_measurement,
    hotclock_net_best,
    hotclock_net_median,
    baseline_net_best,
    baseline_net_median,
    ratio,
    max_ratio,
    max_delta_ns,
  });

  if hotclock.best_ns > allowed {
    eprintln!(
      "hotclock steady-state cost exceeded selected baseline: {:.3} ns/call > {:.3} ns/call allowed",
      hotclock.best_ns, allowed
    );
    std::process::exit(1);
  }
}

fn run_child() {
  let start = StdInstant::now();
  let first = Instant::now();
  black_box(first.as_raw());
  let cold_nanos = start.elapsed().as_nanos();
  println!("{} {cold_nanos}", Instant::implementation());
}

fn cold_measurements(samples: usize) -> Vec<u128> {
  let current_exe = env::current_exe().expect("current benchmark executable");
  let mut measurements = Vec::with_capacity(samples);
  let mut selected_name: Option<String> = None;

  for _ in 0..samples {
    let output = Command::new(&current_exe)
      .env(CHILD_ENV, "1")
      .output()
      .expect("spawn cold benchmark child");
    if !output.status.success() {
      eprintln!("{}", String::from_utf8_lossy(&output.stderr));
      panic!("cold benchmark child failed: {}", output.status);
    }

    let stdout = String::from_utf8(output.stdout).expect("child benchmark output is utf-8");
    let mut fields = stdout.split_whitespace();
    let selected = fields.next().expect("child selected implementation");
    let nanos = fields
      .next()
      .expect("child cold nanoseconds")
      .parse::<u128>()
      .expect("child cold nanoseconds parse");

    if let Some(expected) = &selected_name {
      assert_eq!(selected, expected, "selected clock changed across cold benchmark children");
    } else {
      selected_name = Some(selected.to_owned());
    }
    measurements.push(nanos);
  }

  measurements
}

fn measure_loop(loop_fn: LoopFn, iters: u64, samples: usize) -> Measurement {
  let mut nanos = Vec::with_capacity(samples);
  for _ in 0..samples {
    let start = StdInstant::now();
    black_box(loop_fn(iters));
    let elapsed = start.elapsed().as_nanos();
    nanos.push(ns_per_call(elapsed, iters));
  }
  nanos.sort_by(f64::total_cmp);
  Measurement { best_ns: nanos[0], median_ns: nanos[nanos.len() / 2] }
}

#[inline(never)]
fn run_hotclock_loop(iters: u64) -> u64 {
  let mut acc = 0_u64;
  for _ in 0..iters {
    acc = acc.wrapping_add(black_box(Instant::now().as_raw()));
  }
  acc
}

#[inline(never)]
fn run_noop_loop(iters: u64) -> u64 {
  let mut acc = 0_u64;
  for i in 0..iters {
    acc = acc.wrapping_add(black_box(i));
  }
  acc
}

fn baseline_for(selected: &str) -> Option<Baseline> {
  match selected {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    "x86-rdtsc" | "x86_64-rdtsc" => Some(Baseline { name: "raw rdtsc", loop_fn: run_rdtsc_loop }),

    #[cfg(target_arch = "aarch64")]
    "aarch64-cntvct" => Some(Baseline { name: "raw cntvct_el0", loop_fn: run_cntvct_loop }),

    #[cfg(target_arch = "riscv64")]
    "riscv64-rdcycle" => Some(Baseline { name: "raw rdcycle", loop_fn: run_rdcycle_loop }),

    #[cfg(target_arch = "powerpc64")]
    "powerpc64-mftb" => Some(Baseline { name: "raw mftb", loop_fn: run_mftb_loop }),

    #[cfg(target_arch = "s390x")]
    "s390x-stckf" => Some(Baseline { name: "raw stckf", loop_fn: run_stckf_loop }),

    #[cfg(target_arch = "loongarch64")]
    "loongarch64-rdtime" => Some(Baseline { name: "raw rdtime.d", loop_fn: run_rdtime_loop }),

    #[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
    "macos-mach" => Some(Baseline { name: "mach_absolute_time", loop_fn: run_mach_loop }),

    #[cfg(all(unix, not(target_os = "macos")))]
    "unix-monotonic" => {
      Some(Baseline { name: "clock_gettime(CLOCK_MONOTONIC)", loop_fn: run_clock_monotonic_loop })
    }

    #[cfg(not(unix))]
    "std-instant" => {
      Some(Baseline { name: "std::time::Instant elapsed", loop_fn: run_std_instant_loop })
    }

    _ => None,
  }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline(never)]
fn run_rdtsc_loop(iters: u64) -> u64 {
  let mut acc = 0_u64;
  for _ in 0..iters {
    acc = acc.wrapping_add(black_box(rdtsc()));
  }
  acc
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn rdtsc() -> u64 {
  // SAFETY: `_rdtsc` reads the CPU timestamp counter and has no Rust memory safety preconditions.
  unsafe { core::arch::x86_64::_rdtsc() }
}

#[cfg(target_arch = "x86")]
#[inline(always)]
fn rdtsc() -> u64 {
  // SAFETY: `_rdtsc` reads the CPU timestamp counter and has no Rust memory safety preconditions.
  unsafe { core::arch::x86::_rdtsc() }
}

#[cfg(target_arch = "aarch64")]
#[inline(never)]
fn run_cntvct_loop(iters: u64) -> u64 {
  let mut acc = 0_u64;
  for _ in 0..iters {
    acc = acc.wrapping_add(black_box(cntvct()));
  }
  acc
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn cntvct() -> u64 {
  let cnt: u64;
  // SAFETY: `mrs cntvct_el0` reads the architectural virtual counter.
  unsafe {
    core::arch::asm!(
      "mrs {}, cntvct_el0",
      out(reg) cnt,
      options(nostack, nomem, preserves_flags),
    );
  }
  cnt
}

#[cfg(target_arch = "riscv64")]
#[inline(never)]
fn run_rdcycle_loop(iters: u64) -> u64 {
  let mut acc = 0_u64;
  for _ in 0..iters {
    acc = acc.wrapping_add(black_box(rdcycle()));
  }
  acc
}

#[cfg(target_arch = "riscv64")]
#[inline(always)]
fn rdcycle() -> u64 {
  let cnt: u64;
  // SAFETY: `rdcycle` reads the cycle CSR and does not touch Rust memory.
  unsafe {
    core::arch::asm!(
      "rdcycle {}",
      out(reg) cnt,
      options(nostack, nomem, preserves_flags),
    );
  }
  cnt
}

#[cfg(target_arch = "powerpc64")]
#[inline(never)]
fn run_mftb_loop(iters: u64) -> u64 {
  let mut acc = 0_u64;
  for _ in 0..iters {
    acc = acc.wrapping_add(black_box(mftb()));
  }
  acc
}

#[cfg(target_arch = "powerpc64")]
#[inline(always)]
fn mftb() -> u64 {
  let cnt: u64;
  // SAFETY: `mftb` reads the time-base register and does not touch Rust memory.
  unsafe {
    core::arch::asm!(
      "mftb {}",
      out(reg) cnt,
      options(nostack, nomem, preserves_flags),
    );
  }
  cnt
}

#[cfg(target_arch = "s390x")]
#[inline(never)]
fn run_stckf_loop(iters: u64) -> u64 {
  let mut acc = 0_u64;
  for _ in 0..iters {
    acc = acc.wrapping_add(black_box(stckf()));
  }
  acc
}

#[cfg(target_arch = "s390x")]
#[inline(always)]
fn stckf() -> u64 {
  let mut cnt = 0_u64;
  // SAFETY: `cnt` is a valid writable 8-byte slot for the STCKF store.
  unsafe {
    core::arch::asm!(
      "stckf 0({})",
      in(reg) &mut cnt,
      options(nostack, preserves_flags),
    );
  }
  cnt
}

#[cfg(target_arch = "loongarch64")]
#[inline(never)]
fn run_rdtime_loop(iters: u64) -> u64 {
  let mut acc = 0_u64;
  for _ in 0..iters {
    acc = acc.wrapping_add(black_box(rdtime()));
  }
  acc
}

#[cfg(target_arch = "loongarch64")]
#[inline(always)]
fn rdtime() -> u64 {
  let cnt: u64;
  // SAFETY: `rdtime.d` reads the architectural timer.
  unsafe {
    core::arch::asm!(
      "rdtime.d {}, $zero",
      out(reg) cnt,
      options(nostack, nomem, preserves_flags),
    );
  }
  cnt
}

#[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
#[inline(never)]
fn run_mach_loop(iters: u64) -> u64 {
  let mut acc = 0_u64;
  for _ in 0..iters {
    acc = acc.wrapping_add(black_box(mach_absolute_time()));
  }
  acc
}

#[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
#[inline(always)]
fn mach_absolute_time() -> u64 {
  unsafe extern "C" {
    fn mach_absolute_time() -> u64;
  }

  // SAFETY: `mach_absolute_time` takes no arguments and returns the host monotonic tick value.
  unsafe { mach_absolute_time() }
}

#[cfg(all(unix, not(target_os = "macos")))]
#[repr(C)]
struct Timespec {
  tv_sec: i64,
  tv_nsec: i64,
}

#[cfg(all(unix, not(target_os = "macos")))]
unsafe extern "C" {
  fn clock_gettime(clk_id: i32, tp: *mut Timespec) -> i32;
}

#[cfg(all(unix, not(target_os = "macos")))]
#[inline(never)]
fn run_clock_monotonic_loop(iters: u64) -> u64 {
  let mut acc = 0_u64;
  for _ in 0..iters {
    acc = acc.wrapping_add(black_box(clock_monotonic()));
  }
  acc
}

#[cfg(all(unix, not(target_os = "macos")))]
#[inline(always)]
fn clock_monotonic() -> u64 {
  const CLOCK_MONOTONIC: i32 = 1;
  let mut ts = Timespec { tv_sec: 0, tv_nsec: 0 };
  // SAFETY: `ts` is a valid writable `timespec` for the libc call.
  let rc = unsafe { clock_gettime(CLOCK_MONOTONIC, &mut ts) };
  debug_assert_eq!(rc, 0);
  ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

#[cfg(not(unix))]
#[inline(never)]
fn run_std_instant_loop(iters: u64) -> u64 {
  let mut acc = 0_u64;
  for _ in 0..iters {
    acc = acc.wrapping_add(black_box(std_instant_elapsed()));
  }
  acc
}

#[cfg(not(unix))]
#[inline(always)]
fn std_instant_elapsed() -> u64 {
  static START: OnceLock<StdInstant> = OnceLock::new();
  START.get_or_init(StdInstant::now).elapsed().as_nanos() as u64
}

struct Report<'a> {
  selected: &'a str,
  baseline: &'static str,
  cold: Vec<u128>,
  iters: u64,
  steady_samples: usize,
  loop_overhead: Measurement,
  hotclock: Measurement,
  baseline_measurement: Measurement,
  hotclock_net_best: f64,
  hotclock_net_median: f64,
  baseline_net_best: f64,
  baseline_net_median: f64,
  ratio: f64,
  max_ratio: f64,
  max_delta_ns: f64,
}

fn print_report(mut report: Report<'_>) {
  report.cold.sort_unstable();
  let cold_median = report.cold[report.cold.len() / 2];
  let cold_min = report.cold[0];
  let cold_max = report.cold[report.cold.len() - 1];

  println!("hotclock clock validation");
  println!("target: {}-{}", env::consts::OS, env::consts::ARCH);
  println!("selected: {}", report.selected);
  println!("known fastest baseline: {}", report.baseline);
  println!(
    "cold first call: median={} ns min={} ns max={} ns samples={}",
    cold_median,
    cold_min,
    cold_max,
    report.cold.len()
  );
  println!(
    "steady loop overhead: best={:.3} ns/call median={:.3} ns/call",
    report.loop_overhead.best_ns, report.loop_overhead.median_ns
  );
  println!(
    "steady hotclock raw: best={:.3} ns/call median={:.3} ns/call",
    report.hotclock.best_ns, report.hotclock.median_ns
  );
  println!(
    "steady baseline raw: best={:.3} ns/call median={:.3} ns/call",
    report.baseline_measurement.best_ns, report.baseline_measurement.median_ns
  );
  println!(
    "steady hotclock net: best={:.3} ns/call median={:.3} ns/call",
    report.hotclock_net_best, report.hotclock_net_median
  );
  println!(
    "steady baseline net: best={:.3} ns/call median={:.3} ns/call",
    report.baseline_net_best, report.baseline_net_median
  );
  println!(
    "steady ratio: {:.3}x over selected baseline; threshold <= {:.2}x + {:.2} ns",
    report.ratio, report.max_ratio, report.max_delta_ns
  );
  println!("iters: {} steady_samples: {}", report.iters, report.steady_samples);
}

#[allow(clippy::cast_precision_loss)]
fn ns_per_call(total_nanos: u128, iters: u64) -> f64 {
  total_nanos as f64 / iters as f64
}

fn subtract_ns(value: f64, overhead: f64) -> f64 {
  (value - overhead).max(0.001)
}

fn env_usize(name: &str, default: usize) -> usize {
  env::var(name)
    .ok()
    .and_then(|value| value.parse().ok())
    .unwrap_or(default)
    .max(1)
}

fn env_u64(name: &str, default: u64) -> u64 {
  env::var(name)
    .ok()
    .and_then(|value| value.parse().ok())
    .unwrap_or(default)
    .max(1)
}

fn env_f64(name: &str, default: f64) -> f64 {
  env::var(name).ok().and_then(|value| value.parse().ok()).unwrap_or(default)
}
