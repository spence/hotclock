#![allow(clippy::cast_precision_loss)]

use std::hint::black_box;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant as StdInstant;
use std::{process, thread};

use tach::{Cycles, Instant};

const DEFAULT_WARMUP_ITERS: usize = 10_000;
const DEFAULT_MEASURE_ITERS: usize = 100_000;
const DEFAULT_SAMPLES: usize = 31;
const FASTEST_EPSILON_NS: f64 = 0.005;

#[derive(Clone, Copy)]
struct Stats {
  min: f64,
  p25: f64,
  median: f64,
  p75: f64,
  p95: f64,
  max: f64,
}

impl Stats {
  fn ns_op(self) -> f64 {
    self.median
  }
}

struct Row {
  target: String,
  environment: String,
  expected_instant: String,
  expected_cycles: String,
  selected_instant: &'static str,
  selected_cycles: &'static str,
  tach_instant: Stats,
  tach_cycles: Stats,
  quanta: Stats,
  minstant: Stats,
  fastant: Stats,
  std_instant: Stats,
  primitives: Vec<PrimitiveOutcome>,
  expected_set: bool,
  matches_expected: bool,
  fastest_instant_api: bool,
}

fn main() {
  if let Ok(runtime_api) = std::env::var("AWS_LAMBDA_RUNTIME_API") {
    if let Err(error) = lambda_loop(&runtime_api) {
      eprintln!("lambda runtime failed: {error}");
      std::process::exit(1);
    }
    return;
  }

  let (report, ok) = run_report();
  print!("{report}");

  if !ok {
    std::process::exit(2);
  }
}

fn run_report() -> (String, bool) {
  if std::env::var("TACH_VALIDATION_RACE_STRESS").as_deref() == Ok("1") {
    run_race_stress();
  }

  let expected_instant = std::env::var("TACH_EXPECT_INSTANT").unwrap_or_else(|_| "unset".into());
  let expected_cycles = std::env::var("TACH_EXPECT_CYCLES").unwrap_or_else(|_| "unset".into());
  let expected_set = expected_instant != "unset" && expected_cycles != "unset";

  let selected_instant = Instant::implementation();
  let selected_cycles = Cycles::implementation();

  let measurements = measure_interleaved();
  let tach_instant = measurements.tach_instant;
  let tach_cycles = measurements.tach_cycles;
  let quanta = measurements.quanta;
  let minstant = measurements.minstant;
  let fastant = measurements.fastant;
  let std_instant = measurements.std_instant;
  let primitives = measurements.primitives;
  let fastest_instant_api = tach_instant.ns_op() <= quanta.ns_op() + FASTEST_EPSILON_NS
    && tach_instant.ns_op() <= minstant.ns_op() + FASTEST_EPSILON_NS
    && tach_instant.ns_op() <= fastant.ns_op() + FASTEST_EPSILON_NS
    && tach_instant.ns_op() <= std_instant.ns_op() + FASTEST_EPSILON_NS;

  let row = Row {
    target: target_label(),
    environment: environment_label(),
    expected_set,
    matches_expected: expected_set
      && expected_instant == selected_instant
      && expected_cycles == selected_cycles,
    expected_instant,
    expected_cycles,
    selected_instant,
    selected_cycles,
    tach_instant,
    tach_cycles,
    quanta,
    minstant,
    fastant,
    std_instant,
    primitives,
    fastest_instant_api,
  };

  let ok = std::env::var("TACH_ENFORCE_EXPECTED").as_deref() != Ok("1")
    || (row.expected_set && row.matches_expected);
  (render_row(&row), ok)
}

struct Measurements {
  tach_instant: Stats,
  tach_cycles: Stats,
  quanta: Stats,
  minstant: Stats,
  fastant: Stats,
  std_instant: Stats,
  /// One entry per applicable clock primitive on this target/env. Entry order matches
  /// `applicable_primitives()`. Unavailable primitives appear with their probe reason so
  /// the rendered table can distinguish architectural absence from kernel/hypervisor blocks.
  primitives: Vec<PrimitiveOutcome>,
}

#[derive(Clone)]
struct PrimitiveOutcome {
  kind: PrimitiveKind,
  result: PrimitiveResult,
}

#[derive(Clone)]
enum PrimitiveResult {
  Bench(Stats),
  Unavailable(UnavailReason),
}

#[derive(Clone)]
struct UnavailReason {
  category: UnavailCategory,
  detail: String,
}

#[allow(dead_code)] // Kernel/Host only constructed on cfg(target_os = "linux")
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnavailCategory {
  /// Primitive does not exist on this architecture/OS at all (e.g., RDTSC on aarch64).
  /// Renders as `—` in the table.
  NotApplicable,
  /// Primitive exists but the kernel/sysctl/perf-paranoid policy blocks user access.
  /// Renders as `kernel: <detail>`.
  Kernel,
  /// Primitive exists and kernel exposes it but the hypervisor or container strips it
  /// or makes it pathologically slow (e.g., m7i.4xlarge perf-RDPMC at 1334 ns).
  /// Renders as `host: <detail>`.
  Host,
}

struct BenchCase {
  index: usize,
  kind: BenchKind,
}

#[derive(Clone, Copy)]
enum BenchKind {
  TachInstant,
  TachCycles,
  Quanta,
  Minstant,
  Fastant,
  StdInstant,
  Primitive(PrimitiveKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum PrimitiveKind {
  Rdtsc,
  Rdtscp,
  DirectRdpmc,
  PerfRdpmc,
  CntvctEl0,
  CntpctEl0,
  PmccntrDirect,
  PmccntrPerf,
  MachAbs,
  MachCont,
  ClockMonotonic,
  ClockMonotonicRaw,
  ClockBoottime,
  Qpc,
}

impl PrimitiveKind {
  fn field_name(self) -> &'static str {
    match self {
      Self::Rdtsc => "rdtsc-bench",
      Self::Rdtscp => "rdtscp-bench",
      Self::DirectRdpmc => "direct-rdpmc-bench",
      Self::PerfRdpmc => "perf-rdpmc-bench",
      Self::CntvctEl0 => "cntvct-el0-bench",
      Self::CntpctEl0 => "cntpct-el0-bench",
      Self::PmccntrDirect => "pmccntr-direct-bench",
      Self::PmccntrPerf => "pmccntr-perf-bench",
      Self::MachAbs => "mach-abs-bench",
      Self::MachCont => "mach-cont-bench",
      Self::ClockMonotonic => "clock-monotonic-bench",
      Self::ClockMonotonicRaw => "clock-monotonic-raw-bench",
      Self::ClockBoottime => "clock-boottime-bench",
      Self::Qpc => "qpc-bench",
    }
  }
}

fn measure_interleaved() -> Measurements {
  let warmup_iters = env_usize("TACH_VALIDATION_WARMUP_ITERS", DEFAULT_WARMUP_ITERS);
  let measure_iters = env_usize("TACH_VALIDATION_MEASURE_ITERS", DEFAULT_MEASURE_ITERS);
  let samples = env_usize("TACH_VALIDATION_SAMPLES", DEFAULT_SAMPLES);

  let primitives_in = applicable_primitives();
  let probe_results: Vec<(PrimitiveKind, Result<(), UnavailReason>)> = primitives_in
    .iter()
    .map(|&prim| (prim, probe_primitive(prim)))
    .collect();

  let benched_primitives: Vec<PrimitiveKind> = probe_results
    .iter()
    .filter_map(|(prim, result)| if result.is_ok() { Some(*prim) } else { None })
    .collect();

  let mut cases: Vec<BenchCase> = Vec::with_capacity(FIXED_BENCH_COUNT + benched_primitives.len());
  cases.push(BenchCase { index: TACH_INSTANT, kind: BenchKind::TachInstant });
  cases.push(BenchCase { index: TACH_CYCLES, kind: BenchKind::TachCycles });
  cases.push(BenchCase { index: QUANTA, kind: BenchKind::Quanta });
  cases.push(BenchCase { index: MINSTANT, kind: BenchKind::Minstant });
  cases.push(BenchCase { index: FASTANT, kind: BenchKind::Fastant });
  cases.push(BenchCase { index: STD_INSTANT, kind: BenchKind::StdInstant });
  for (idx, prim) in benched_primitives.iter().enumerate() {
    cases.push(BenchCase {
      index: FIXED_BENCH_COUNT + idx,
      kind: BenchKind::Primitive(*prim),
    });
  }

  for case in &cases {
    run_bench_case(case.kind, warmup_iters);
  }

  let total_slots = FIXED_BENCH_COUNT + benched_primitives.len();
  let mut timings: Vec<Vec<f64>> =
    (0..total_slots).map(|_| Vec::with_capacity(samples)).collect();
  for sample in 0..samples {
    let offset = sample % cases.len();
    for step in 0..cases.len() {
      let case = &cases[(offset + step) % cases.len()];
      let start = StdInstant::now();
      run_bench_case(case.kind, measure_iters);
      timings[case.index].push(start.elapsed().as_nanos() as f64 / measure_iters as f64);
    }
  }

  let primitive_stats: std::collections::HashMap<PrimitiveKind, Stats> = benched_primitives
    .iter()
    .enumerate()
    .map(|(idx, prim)| (*prim, stats(timings[FIXED_BENCH_COUNT + idx].clone())))
    .collect();

  let primitives = probe_results
    .into_iter()
    .map(|(prim, result)| {
      let outcome = match result {
        Ok(()) => PrimitiveResult::Bench(primitive_stats[&prim]),
        Err(reason) => PrimitiveResult::Unavailable(reason),
      };
      PrimitiveOutcome { kind: prim, result: outcome }
    })
    .collect();

  Measurements {
    tach_instant: stats(timings[TACH_INSTANT].clone()),
    tach_cycles: stats(timings[TACH_CYCLES].clone()),
    quanta: stats(timings[QUANTA].clone()),
    minstant: stats(timings[MINSTANT].clone()),
    fastant: stats(timings[FASTANT].clone()),
    std_instant: stats(timings[STD_INSTANT].clone()),
    primitives,
  }
}

fn stats(mut timings: Vec<f64>) -> Stats {
  timings.sort_by(f64::total_cmp);
  let last = timings.len() - 1;
  Stats {
    min: timings[0],
    p25: timings[last / 4],
    median: timings[last / 2],
    p75: timings[(last * 3) / 4],
    p95: timings[(last * 95) / 100],
    max: timings[last],
  }
}

const TACH_INSTANT: usize = 0;
const TACH_CYCLES: usize = 1;
const QUANTA: usize = 2;
const MINSTANT: usize = 3;
const FASTANT: usize = 4;
const STD_INSTANT: usize = 5;
const FIXED_BENCH_COUNT: usize = 6;

#[inline(never)]
fn run_bench_case(kind: BenchKind, iterations: usize) {
  match kind {
    BenchKind::TachInstant => {
      for _ in 0..iterations {
        let _ = black_box(Instant::now());
      }
    }
    BenchKind::TachCycles => {
      for _ in 0..iterations {
        let _ = black_box(Cycles::now());
      }
    }
    BenchKind::Quanta => {
      for _ in 0..iterations {
        let _ = black_box(quanta::Instant::now());
      }
    }
    BenchKind::Minstant => {
      for _ in 0..iterations {
        let _ = black_box(minstant::Instant::now());
      }
    }
    BenchKind::Fastant => {
      for _ in 0..iterations {
        let _ = black_box(fastant::Instant::now());
      }
    }
    BenchKind::StdInstant => {
      for _ in 0..iterations {
        let _ = black_box(StdInstant::now());
      }
    }
    BenchKind::Primitive(prim) => run_primitive_bench(prim, iterations),
  }
}

// ============================================================================
// Per-primitive bench + probe support
// ============================================================================

/// Returns every clock primitive that could conceivably exist on this target/env
/// (architecturally relevant). Primitives are emitted in canonical column order so
/// `render_row` produces a stable layout that `render-baseline.sh` can parse.
fn applicable_primitives() -> Vec<PrimitiveKind> {
  let mut out = Vec::new();
  if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
    out.push(PrimitiveKind::Rdtsc);
    out.push(PrimitiveKind::Rdtscp);
    if cfg!(target_os = "linux") {
      out.push(PrimitiveKind::DirectRdpmc);
      out.push(PrimitiveKind::PerfRdpmc);
    }
  }
  if cfg!(target_arch = "aarch64") {
    out.push(PrimitiveKind::CntvctEl0);
    out.push(PrimitiveKind::CntpctEl0);
    if cfg!(target_os = "linux") {
      out.push(PrimitiveKind::PmccntrDirect);
      out.push(PrimitiveKind::PmccntrPerf);
    }
  }
  if cfg!(target_os = "macos") {
    out.push(PrimitiveKind::MachAbs);
    out.push(PrimitiveKind::MachCont);
  }
  if cfg!(target_os = "linux") {
    out.push(PrimitiveKind::ClockMonotonic);
    out.push(PrimitiveKind::ClockMonotonicRaw);
    out.push(PrimitiveKind::ClockBoottime);
  }
  if cfg!(target_os = "windows") {
    out.push(PrimitiveKind::Qpc);
  }
  out
}

/// Cheap, side-effect-free probe per primitive. Returns `Ok(())` if it's safe and
/// sensible to bench the primitive on this host, otherwise an `UnavailReason` that
/// classifies the reason (architectural N/A vs kernel block vs hypervisor strip).
fn probe_primitive(prim: PrimitiveKind) -> Result<(), UnavailReason> {
  match prim {
    PrimitiveKind::Rdtsc => probe_rdtsc(),
    PrimitiveKind::Rdtscp => probe_rdtscp(),
    PrimitiveKind::DirectRdpmc => probe_direct_rdpmc(),
    PrimitiveKind::PerfRdpmc => probe_perf_rdpmc(),
    PrimitiveKind::CntvctEl0 => probe_cntvct(),
    PrimitiveKind::CntpctEl0 => probe_cntpct(),
    PrimitiveKind::PmccntrDirect => probe_pmccntr_direct(),
    PrimitiveKind::PmccntrPerf => probe_pmccntr_perf(),
    PrimitiveKind::MachAbs => probe_mach_abs(),
    PrimitiveKind::MachCont => probe_mach_cont(),
    PrimitiveKind::ClockMonotonic => probe_clock_gettime(libc_clock_monotonic()),
    PrimitiveKind::ClockMonotonicRaw => probe_clock_gettime(libc_clock_monotonic_raw()),
    PrimitiveKind::ClockBoottime => probe_clock_gettime(libc_clock_boottime()),
    PrimitiveKind::Qpc => probe_qpc(),
  }
}

#[inline(never)]
fn run_primitive_bench(prim: PrimitiveKind, iterations: usize) {
  match prim {
    PrimitiveKind::Rdtsc => bench_rdtsc(iterations),
    PrimitiveKind::Rdtscp => bench_rdtscp(iterations),
    PrimitiveKind::DirectRdpmc => bench_direct_rdpmc(iterations),
    PrimitiveKind::PerfRdpmc => bench_perf_rdpmc(iterations),
    PrimitiveKind::CntvctEl0 => bench_cntvct(iterations),
    PrimitiveKind::CntpctEl0 => bench_cntpct(iterations),
    PrimitiveKind::PmccntrDirect => bench_pmccntr_direct(iterations),
    PrimitiveKind::PmccntrPerf => bench_pmccntr_perf(iterations),
    PrimitiveKind::MachAbs => bench_mach_abs(iterations),
    PrimitiveKind::MachCont => bench_mach_cont(iterations),
    PrimitiveKind::ClockMonotonic => bench_clock_gettime(libc_clock_monotonic(), iterations),
    PrimitiveKind::ClockMonotonicRaw => bench_clock_gettime(libc_clock_monotonic_raw(), iterations),
    PrimitiveKind::ClockBoottime => bench_clock_gettime(libc_clock_boottime(), iterations),
    PrimitiveKind::Qpc => bench_qpc(iterations),
  }
}

fn not_applicable() -> UnavailReason {
  UnavailReason { category: UnavailCategory::NotApplicable, detail: String::new() }
}

#[allow(dead_code)] // only called on cfg(target_os = "linux") paths
fn kernel_unavail(detail: impl Into<String>) -> UnavailReason {
  UnavailReason { category: UnavailCategory::Kernel, detail: detail.into() }
}

#[allow(dead_code)] // only called on cfg(target_os = "linux") paths
fn host_unavail(detail: impl Into<String>) -> UnavailReason {
  UnavailReason { category: UnavailCategory::Host, detail: detail.into() }
}

// ---------------- x86 / x86_64 primitives ----------------

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn probe_rdtsc() -> Result<(), UnavailReason> {
  Ok(())
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn probe_rdtsc() -> Result<(), UnavailReason> {
  Err(not_applicable())
}

#[cfg(target_arch = "x86_64")]
fn bench_rdtsc(iters: usize) {
  for _ in 0..iters {
    // SAFETY: `_rdtsc` reads the architectural timestamp counter.
    let _ = black_box(unsafe { core::arch::x86_64::_rdtsc() });
  }
}

#[cfg(target_arch = "x86")]
fn bench_rdtsc(iters: usize) {
  for _ in 0..iters {
    // SAFETY: `_rdtsc` reads the architectural timestamp counter.
    let _ = black_box(unsafe { core::arch::x86::_rdtsc() });
  }
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn bench_rdtsc(_iters: usize) {}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn probe_rdtscp() -> Result<(), UnavailReason> {
  Ok(())
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn probe_rdtscp() -> Result<(), UnavailReason> {
  Err(not_applicable())
}

#[cfg(target_arch = "x86_64")]
fn bench_rdtscp(iters: usize) {
  for _ in 0..iters {
    let mut aux: u32 = 0;
    // SAFETY: `__rdtscp` is the serializing variant of RDTSC; available on all x86_64.
    let _ = black_box(unsafe { core::arch::x86_64::__rdtscp(&mut aux) });
  }
}

#[cfg(not(target_arch = "x86_64"))]
fn bench_rdtscp(_iters: usize) {}

#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
fn probe_direct_rdpmc() -> Result<(), UnavailReason> {
  // `/sys/bus/event_source/devices/cpu/rdpmc` must be 2 (all-processes) for direct
  // user-mode RDPMC; 0 or 1 mean it's gated.
  match std::fs::read_to_string("/sys/bus/event_source/devices/cpu/rdpmc") {
    Ok(value) => {
      let trimmed = value.trim();
      if trimmed == "2" {
        Ok(())
      } else {
        Err(kernel_unavail(format!("sysfs rdpmc={trimmed}, need 2")))
      }
    }
    Err(error) => Err(kernel_unavail(format!("sysfs rdpmc read: {error}"))),
  }
}

#[cfg(not(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64"))))]
fn probe_direct_rdpmc() -> Result<(), UnavailReason> {
  Err(not_applicable())
}

#[cfg(target_arch = "x86_64")]
fn bench_direct_rdpmc(iters: usize) {
  // Fixed-function PMC index for unhalted core cycles is selector 0x40000001.
  const RDPMC_FIXED_CORE_CYCLES: u32 = 0x40000001;
  for _ in 0..iters {
    let lo: u32;
    let hi: u32;
    // SAFETY: Only invoked after probe_direct_rdpmc confirmed sysfs rdpmc=2 (all-processes).
    unsafe {
      core::arch::asm!(
        "rdpmc",
        in("ecx") RDPMC_FIXED_CORE_CYCLES,
        out("eax") lo,
        out("edx") hi,
        options(nomem, nostack, preserves_flags),
      );
    }
    let _ = black_box(((hi as u64) << 32) | (lo as u64));
  }
}

#[cfg(not(target_arch = "x86_64"))]
fn bench_direct_rdpmc(_iters: usize) {}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn probe_perf_rdpmc() -> Result<(), UnavailReason> {
  if tach::arch::perf_rdpmc_linux::perf_rdpmc_cpu_cycles_available() {
    Ok(())
  } else {
    Err(host_unavail("perf_event_open + mmap returned cap_user_rdpmc=false"))
  }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn probe_perf_rdpmc() -> Result<(), UnavailReason> {
  Err(not_applicable())
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn bench_perf_rdpmc(iters: usize) {
  for _ in 0..iters {
    let _ = black_box(tach::arch::perf_rdpmc_linux::perf_rdpmc_cpu_cycles());
  }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn bench_perf_rdpmc(_iters: usize) {}

// ---------------- aarch64 primitives ----------------

#[cfg(target_arch = "aarch64")]
fn probe_cntvct() -> Result<(), UnavailReason> {
  Ok(())
}

#[cfg(not(target_arch = "aarch64"))]
fn probe_cntvct() -> Result<(), UnavailReason> {
  Err(not_applicable())
}

#[cfg(target_arch = "aarch64")]
fn bench_cntvct(iters: usize) {
  for _ in 0..iters {
    let value: u64;
    // SAFETY: CNTVCT_EL0 is the architectural virtual counter, always readable from EL0.
    unsafe {
      core::arch::asm!(
        "mrs {0}, cntvct_el0",
        out(reg) value,
        options(nomem, nostack, preserves_flags),
      );
    }
    let _ = black_box(value);
  }
}

#[cfg(not(target_arch = "aarch64"))]
fn bench_cntvct(_iters: usize) {}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn probe_cntpct() -> Result<(), UnavailReason> {
  // Linux gates CNTPCT_EL0 EL0 access via CNTKCTL_EL1.EL0PCTEN. Without a sigaction-guarded
  // probe we can't safely call it. Mark as kernel-blocked by default; some kernels expose
  // it but the historical default on AL2023 + Graviton is "blocked".
  Err(kernel_unavail("Linux gates CNTKCTL_EL1.EL0PCTEN by default"))
}

#[cfg(all(not(target_os = "linux"), target_arch = "aarch64"))]
fn probe_cntpct() -> Result<(), UnavailReason> {
  // Darwin & Windows ARM expose CNTPCT_EL0 readably from EL0.
  Ok(())
}

#[cfg(not(target_arch = "aarch64"))]
fn probe_cntpct() -> Result<(), UnavailReason> {
  Err(not_applicable())
}

#[cfg(target_arch = "aarch64")]
fn bench_cntpct(iters: usize) {
  for _ in 0..iters {
    let value: u64;
    // SAFETY: Only invoked when probe_cntpct returns Ok (darwin / windows aarch64).
    unsafe {
      core::arch::asm!(
        "mrs {0}, cntpct_el0",
        out(reg) value,
        options(nomem, nostack, preserves_flags),
      );
    }
    let _ = black_box(value);
  }
}

#[cfg(not(target_arch = "aarch64"))]
fn bench_cntpct(_iters: usize) {}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn probe_pmccntr_direct() -> Result<(), UnavailReason> {
  // PMUSERENR_EL0.EN must be 1, normally only via /proc/sys/kernel/perf_user_access=1.
  match std::fs::read_to_string("/proc/sys/kernel/perf_user_access") {
    Ok(value) if value.trim() == "1" => Ok(()),
    Ok(value) => Err(kernel_unavail(format!("perf_user_access={}, need 1", value.trim()))),
    Err(error) => Err(kernel_unavail(format!("perf_user_access read: {error}"))),
  }
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
fn probe_pmccntr_direct() -> Result<(), UnavailReason> {
  Err(not_applicable())
}

#[cfg(target_arch = "aarch64")]
fn bench_pmccntr_direct(iters: usize) {
  for _ in 0..iters {
    let value: u64;
    // SAFETY: Only invoked when probe_pmccntr_direct confirmed perf_user_access=1.
    unsafe {
      core::arch::asm!(
        "mrs {0}, pmccntr_el0",
        out(reg) value,
        options(nomem, nostack, preserves_flags),
      );
    }
    let _ = black_box(value);
  }
}

#[cfg(not(target_arch = "aarch64"))]
fn bench_pmccntr_direct(_iters: usize) {}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn probe_pmccntr_perf() -> Result<(), UnavailReason> {
  if tach::arch::perf_pmccntr_linux::perf_pmccntr_cpu_cycles_available() {
    Ok(())
  } else {
    Err(host_unavail("perf_event_open + mmap returned cap_user_rdpmc=false"))
  }
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
fn probe_pmccntr_perf() -> Result<(), UnavailReason> {
  Err(not_applicable())
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn bench_pmccntr_perf(iters: usize) {
  for _ in 0..iters {
    let _ = black_box(tach::arch::perf_pmccntr_linux::perf_pmccntr_cpu_cycles());
  }
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
fn bench_pmccntr_perf(_iters: usize) {}

// ---------------- Darwin primitives ----------------

#[cfg(target_os = "macos")]
unsafe extern "C" {
  fn mach_absolute_time() -> u64;
  fn mach_continuous_time() -> u64;
}

#[cfg(target_os = "macos")]
fn probe_mach_abs() -> Result<(), UnavailReason> {
  Ok(())
}

#[cfg(not(target_os = "macos"))]
fn probe_mach_abs() -> Result<(), UnavailReason> {
  Err(not_applicable())
}

#[cfg(target_os = "macos")]
fn bench_mach_abs(iters: usize) {
  for _ in 0..iters {
    // SAFETY: `mach_absolute_time` has no Rust-side safety preconditions.
    let _ = black_box(unsafe { mach_absolute_time() });
  }
}

#[cfg(not(target_os = "macos"))]
fn bench_mach_abs(_iters: usize) {}

#[cfg(target_os = "macos")]
fn probe_mach_cont() -> Result<(), UnavailReason> {
  Ok(())
}

#[cfg(not(target_os = "macos"))]
fn probe_mach_cont() -> Result<(), UnavailReason> {
  Err(not_applicable())
}

#[cfg(target_os = "macos")]
fn bench_mach_cont(iters: usize) {
  for _ in 0..iters {
    // SAFETY: `mach_continuous_time` has no Rust-side safety preconditions (macOS 10.12+).
    let _ = black_box(unsafe { mach_continuous_time() });
  }
}

#[cfg(not(target_os = "macos"))]
fn bench_mach_cont(_iters: usize) {}

// ---------------- Linux clock_gettime ----------------

#[cfg(target_os = "linux")]
const LIBC_CLOCK_MONOTONIC: i32 = 1;
#[cfg(target_os = "linux")]
const LIBC_CLOCK_MONOTONIC_RAW: i32 = 4;
#[cfg(target_os = "linux")]
const LIBC_CLOCK_BOOTTIME: i32 = 7;

#[cfg(target_os = "linux")]
fn libc_clock_monotonic() -> i32 {
  LIBC_CLOCK_MONOTONIC
}
#[cfg(not(target_os = "linux"))]
fn libc_clock_monotonic() -> i32 {
  0
}

#[cfg(target_os = "linux")]
fn libc_clock_monotonic_raw() -> i32 {
  LIBC_CLOCK_MONOTONIC_RAW
}
#[cfg(not(target_os = "linux"))]
fn libc_clock_monotonic_raw() -> i32 {
  0
}

#[cfg(target_os = "linux")]
fn libc_clock_boottime() -> i32 {
  LIBC_CLOCK_BOOTTIME
}
#[cfg(not(target_os = "linux"))]
fn libc_clock_boottime() -> i32 {
  0
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
  fn clock_gettime(clk_id: i32, ts: *mut Timespec) -> i32;
}

#[cfg(target_os = "linux")]
#[repr(C)]
#[derive(Default)]
struct Timespec {
  tv_sec: i64,
  tv_nsec: i64,
}

#[cfg(target_os = "linux")]
fn probe_clock_gettime(_clk_id: i32) -> Result<(), UnavailReason> {
  Ok(())
}

#[cfg(not(target_os = "linux"))]
fn probe_clock_gettime(_clk_id: i32) -> Result<(), UnavailReason> {
  Err(not_applicable())
}

#[cfg(target_os = "linux")]
fn bench_clock_gettime(clk_id: i32, iters: usize) {
  for _ in 0..iters {
    let mut ts = Timespec::default();
    // SAFETY: `clock_gettime` is a well-behaved glibc/musl function with no Rust-side safety
    // preconditions.
    let _ = black_box(unsafe { clock_gettime(clk_id, &mut ts) });
    let _ = black_box(ts.tv_sec.wrapping_mul(1_000_000_000).wrapping_add(ts.tv_nsec));
  }
}

#[cfg(not(target_os = "linux"))]
fn bench_clock_gettime(_clk_id: i32, _iters: usize) {}

// ---------------- Windows QPC ----------------

#[cfg(target_os = "windows")]
unsafe extern "system" {
  fn QueryPerformanceCounter(perf_count: *mut i64) -> i32;
}

#[cfg(target_os = "windows")]
fn probe_qpc() -> Result<(), UnavailReason> {
  Ok(())
}

#[cfg(not(target_os = "windows"))]
fn probe_qpc() -> Result<(), UnavailReason> {
  Err(not_applicable())
}

#[cfg(target_os = "windows")]
fn bench_qpc(iters: usize) {
  for _ in 0..iters {
    let mut value: i64 = 0;
    // SAFETY: QueryPerformanceCounter has no Rust-side preconditions; available on all
    // Windows builds since XP.
    let _ = black_box(unsafe { QueryPerformanceCounter(&mut value) });
    let _ = black_box(value);
  }
}

#[cfg(not(target_os = "windows"))]
fn bench_qpc(_iters: usize) {}

fn render_row(row: &Row) -> String {
  let mut out = String::new();
  out.push_str("┌────────────────────────────────┬────────────────────────────────────────┐\n");
  out.push_str("│ Field                          │ Value                                  │\n");
  out.push_str("├────────────────────────────────┼────────────────────────────────────────┤\n");
  push_field(&mut out, "target", &row.target);
  push_field(&mut out, "environment", &row.environment);
  push_field(&mut out, "fastest-known-instant-clock", &row.expected_instant);
  push_field(&mut out, "fastest-known-cycles-clock", &row.expected_cycles);
  push_field(&mut out, "selected-instant-clock", row.selected_instant);
  push_field(&mut out, "selected-cycles-clock", row.selected_cycles);
  push_field(&mut out, "tach-instant-bench", &format_ns(row.tach_instant));
  push_field(&mut out, "tach-cycles-bench", &format_ns(row.tach_cycles));
  for outcome in &row.primitives {
    let value = match &outcome.result {
      PrimitiveResult::Bench(stats) => format_ns(*stats),
      PrimitiveResult::Unavailable(reason) => format_unavail(reason),
    };
    push_field(&mut out, outcome.kind.field_name(), &value);
  }
  push_field(&mut out, "quanta-bench", &format_ns(row.quanta));
  push_field(&mut out, "minstant-bench", &format_ns(row.minstant));
  push_field(&mut out, "fastant-bench", &format_ns(row.fastant));
  push_field(&mut out, "std-instant-bench", &format_ns(row.std_instant));
  push_field(&mut out, "fastest-instant-api", yes_no(row.fastest_instant_api));
  push_field(&mut out, "cycles-le-instant", cycles_le_instant_status(row));
  push_field(&mut out, "matches-expected", expected_status(row));
  out.push_str("└────────────────────────────────┴────────────────────────────────────────┘\n");

  if std::env::var("TACH_VALIDATION_DISTRIBUTION").as_deref() == Ok("1") {
    render_distribution(&mut out, row);
  }

  out
}

fn render_distribution(out: &mut String, row: &Row) {
  out.push('\n');
  out.push_str(
    "┌───────────────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┐\n",
  );
  out.push_str(
    "│ Benchmark         │ min      │ p25      │ median   │ p75      │ p95      │ max      │\n",
  );
  out.push_str(
    "├───────────────────┼──────────┼──────────┼──────────┼──────────┼──────────┼──────────┤\n",
  );
  push_stats(out, "tach-instant", row.tach_instant);
  push_stats(out, "tach-cycles", row.tach_cycles);
  for outcome in &row.primitives {
    if let PrimitiveResult::Bench(stats) = &outcome.result {
      let label = outcome.kind.field_name().trim_end_matches("-bench");
      push_stats(out, label, *stats);
    }
  }
  push_stats(out, "quanta", row.quanta);
  push_stats(out, "minstant", row.minstant);
  push_stats(out, "fastant", row.fastant);
  push_stats(out, "std-instant", row.std_instant);
  out.push_str(
    "└───────────────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┘\n",
  );
}

fn push_stats(out: &mut String, name: &str, stats: Stats) {
  out.push_str(&format!(
    "│ {name:<17} │ {:>6.3}ns │ {:>6.3}ns │ {:>6.3}ns │ {:>6.3}ns │ {:>6.3}ns │ {:>6.3}ns │\n",
    stats.min, stats.p25, stats.median, stats.p75, stats.p95, stats.max,
  ));
}

fn push_field(out: &mut String, field: &str, value: &str) {
  out.push_str(&format!("│ {field:<30} │ {value:<38} │\n"));
}

fn format_ns(stats: Stats) -> String {
  format!("{:.3} ns/op", stats.ns_op())
}

fn format_unavail(reason: &UnavailReason) -> String {
  match reason.category {
    UnavailCategory::NotApplicable => "unavailable: not applicable".to_string(),
    UnavailCategory::Kernel => format!("unavailable: kernel: {}", reason.detail),
    UnavailCategory::Host => format!("unavailable: host: {}", reason.detail),
  }
}

fn expected_status(row: &Row) -> &'static str {
  if !row.expected_set {
    "unchecked"
  } else if row.matches_expected {
    "yes"
  } else {
    "no"
  }
}

/// The Cycles ≤ Instant contract: Cycles selection always includes the Instant
/// counter as a candidate, so the chosen Cycles read should never exceed the
/// Instant read by more than measurement noise (0.5 ns is near the noise floor on
/// modern hardware). A "fail" here indicates a selection or patching bug, not
/// noise — sharpen the measurement (more samples, pinned core) rather than
/// widening this tolerance.
fn cycles_le_instant_status(row: &Row) -> &'static str {
  const TOLERANCE_NS: f64 = 0.5;
  if row.tach_cycles.ns_op() <= row.tach_instant.ns_op() + TOLERANCE_NS {
    "pass"
  } else {
    "fail"
  }
}

fn yes_no(value: bool) -> &'static str {
  if value { "yes" } else { "no" }
}

fn env_usize(name: &str, default: usize) -> usize {
  std::env::var(name).ok().and_then(|value| value.parse().ok()).unwrap_or(default)
}

fn run_race_stress() {
  let threads = env_usize("TACH_VALIDATION_RACE_THREADS", 32);
  let iterations = env_usize("TACH_VALIDATION_RACE_ITERS", 20_000);
  let barrier = Arc::new(Barrier::new(threads));
  let failed = Arc::new(AtomicBool::new(false));
  let mut handles = Vec::with_capacity(threads);

  for _ in 0..threads {
    let barrier = Arc::clone(&barrier);
    let failed = Arc::clone(&failed);
    handles.push(thread::spawn(move || {
      barrier.wait();
      for _ in 0..iterations {
        let start = Instant::now();
        let end = Instant::now();
        if end.checked_duration_since(start).is_none() {
          failed.store(true, Ordering::Relaxed);
        }
        let _ = black_box(Cycles::now());
      }
    }));
  }

  for handle in handles {
    if handle.join().is_err() {
      failed.store(true, Ordering::Relaxed);
    }
  }

  if failed.load(Ordering::Relaxed) {
    eprintln!("race stress failed");
    process::exit(3);
  }
}

fn target_label() -> String {
  format!("{}-{}-{}-{}bit", std::env::consts::ARCH, std::env::consts::OS, target_env(), usize::BITS)
}

fn target_env() -> &'static str {
  if cfg!(target_env = "gnu") {
    "gnu"
  } else if cfg!(target_env = "musl") {
    "musl"
  } else if cfg!(target_env = "msvc") {
    "msvc"
  } else {
    "unknown"
  }
}

fn environment_label() -> String {
  if let Ok(name) = std::env::var("TACH_ENV_NAME") {
    return name;
  }

  #[cfg(target_os = "linux")]
  {
    linux_environment_label()
  }

  #[cfg(not(target_os = "linux"))]
  {
    format!("local-{}", std::env::consts::OS)
  }
}

fn lambda_loop(runtime_api: &str) -> std::io::Result<()> {
  loop {
    let response = http_request(runtime_api, "GET", "/2018-06-01/runtime/invocation/next", &[])?;
    let request_id = header_value(&response.headers, "lambda-runtime-aws-request-id")
      .ok_or_else(|| std::io::Error::other("missing Lambda request id"))?;

    let (report, ok) = run_report();
    let path = if ok {
      format!("/2018-06-01/runtime/invocation/{request_id}/response")
    } else {
      format!("/2018-06-01/runtime/invocation/{request_id}/error")
    };
    let body = if ok {
      report
    } else {
      format!(
        "{{\"errorMessage\":\"{}\",\"errorType\":\"TachSelectionMismatch\"}}",
        json_escape(&report)
      )
    };

    let _ = http_request(runtime_api, "POST", &path, body.as_bytes())?;
  }
}

struct HttpResponse {
  headers: String,
}

fn http_request(
  host: &str,
  method: &str,
  path: &str,
  body: &[u8],
) -> std::io::Result<HttpResponse> {
  let mut stream = TcpStream::connect(host)?;
  write!(
    stream,
    "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
    body.len()
  )?;
  stream.write_all(body)?;

  let mut response = Vec::new();
  stream.read_to_end(&mut response)?;
  let headers_end = response
    .windows(4)
    .position(|window| window == b"\r\n\r\n")
    .ok_or_else(|| std::io::Error::other("invalid HTTP response"))?;
  let headers = String::from_utf8_lossy(&response[..headers_end]).into_owned();

  Ok(HttpResponse { headers })
}

fn header_value(headers: &str, name: &str) -> Option<String> {
  headers.lines().find_map(|line| {
    let (key, value) = line.split_once(':')?;
    if key.eq_ignore_ascii_case(name) { Some(value.trim().to_string()) } else { None }
  })
}

fn json_escape(value: &str) -> String {
  let mut escaped = String::with_capacity(value.len());
  for ch in value.chars() {
    match ch {
      '"' => escaped.push_str("\\\""),
      '\\' => escaped.push_str("\\\\"),
      '\n' => escaped.push_str("\\n"),
      '\r' => escaped.push_str("\\r"),
      '\t' => escaped.push_str("\\t"),
      ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
      ch => escaped.push(ch),
    }
  }
  escaped
}

#[cfg(target_os = "linux")]
fn linux_environment_label() -> String {
  if std::env::var_os("AWS_LAMBDA_FUNCTION_NAME").is_some() {
    return "aws-lambda".into();
  }

  let product = std::fs::read_to_string("/sys/class/dmi/id/product_name")
    .ok()
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty());

  if let Some(product) = product {
    return product;
  }

  if std::path::Path::new("/.dockerenv").exists() {
    return "container".into();
  }

  "linux".into()
}
