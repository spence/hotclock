#![cfg(feature = "bench-internals")]
#![allow(clippy::cast_precision_loss)]

use std::fmt::Write as _;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant as StdInstant;

use hotclock::Instant;
use hotclock::bench_internals::{ClockCandidate, candidate_clocks, evaluate_candidate_clock};
use serde_json::{Value, json};

const DEFAULT_WARMUP_ITERS: usize = 5_000;
const DEFAULT_MEASURE_ITERS: usize = 50_000;
const DEFAULT_SAMPLES: usize = 11;

#[derive(Clone, Copy)]
struct BenchmarkConfig {
  warmup_iters: usize,
  measure_iters: usize,
  samples: usize,
}

#[derive(Clone, Copy)]
struct Stats {
  best_ns: f64,
  mean_ns: f64,
  median_ns: f64,
  worst_ns: f64,
  stddev_ns: f64,
  ci95_low_ns: f64,
  ci95_high_ns: f64,
  samples: u64,
}

struct CandidateResult {
  candidate: ClockCandidate,
  valid: bool,
  precision_ticks: Option<u64>,
  fastest_valid: bool,
  selected: bool,
  stats: Stats,
}

struct ComparisonResult {
  name: &'static str,
  operation: &'static str,
  stats: Stats,
}

fn main() -> std::io::Result<()> {
  let _ = Instant::frequency();

  let mut candidates = measure_candidates();
  mark_fastest_valid_candidate(&mut candidates);

  let comparison_config = comparison_config();
  let comparisons =
    if skip_comparisons() { Vec::new() } else { measure_comparisons(comparison_config) };
  let compile_identity = compile_identity();
  let runtime_identity = runtime_identity();
  let proof_eligible =
    candidates.iter().filter(|row| row.candidate.selected_by_hotclock).count() > 1;
  let markdown = render_markdown(
    &compile_identity,
    &runtime_identity,
    proof_eligible,
    &candidates,
    comparison_config,
    &comparisons,
  );
  let json = render_json(
    &compile_identity,
    &runtime_identity,
    proof_eligible,
    &candidates,
    comparison_config,
    &comparisons,
  );

  println!("{markdown}");

  if let Some(report_dir) = std::env::var_os("HOTCLOCK_REPORT_DIR") {
    let report_dir = PathBuf::from(report_dir);
    std::fs::create_dir_all(&report_dir)?;
    std::fs::write(report_dir.join("runtime-selection-report.md"), &markdown)?;
    std::fs::write(report_dir.join("runtime-selection-report.json"), json.to_string())?;
  }

  Ok(())
}

fn measure_candidates() -> Vec<CandidateResult> {
  candidate_clocks()
    .iter()
    .map(|candidate| {
      let evaluation = evaluate_candidate_clock(*candidate);
      CandidateResult {
        candidate: *candidate,
        valid: evaluation.valid,
        precision_ticks: evaluation.precision_ticks,
        fastest_valid: false,
        selected: candidate.name == Instant::implementation(),
        stats: if evaluation.valid {
          Stats {
            best_ns: evaluation.latency.best_ns,
            mean_ns: evaluation.latency.mean_ns,
            median_ns: evaluation.latency.median_ns,
            worst_ns: evaluation.latency.worst_ns,
            stddev_ns: evaluation.latency.stddev_ns,
            ci95_low_ns: evaluation.latency.ci95_low_ns,
            ci95_high_ns: evaluation.latency.ci95_high_ns,
            samples: evaluation.latency.samples,
          }
        } else {
          Stats::zero()
        },
      }
    })
    .collect()
}

impl Stats {
  const fn zero() -> Self {
    Self {
      best_ns: 0.0,
      mean_ns: 0.0,
      median_ns: 0.0,
      worst_ns: 0.0,
      stddev_ns: 0.0,
      ci95_low_ns: 0.0,
      ci95_high_ns: 0.0,
      samples: 0,
    }
  }
}

fn mark_fastest_valid_candidate(candidates: &mut [CandidateResult]) {
  let fastest = candidates
    .iter()
    .enumerate()
    .filter(|(_, row)| row.valid && row.candidate.selected_by_hotclock)
    .min_by(|(_, left), (_, right)| {
      (
        ordered_f64(left.stats.median_ns),
        ordered_f64(left.stats.best_ns),
        left.precision_ticks.unwrap_or(u64::MAX),
      )
        .cmp(&(
          ordered_f64(right.stats.median_ns),
          ordered_f64(right.stats.best_ns),
          right.precision_ticks.unwrap_or(u64::MAX),
        ))
    })
    .map(|(index, _)| index);

  if let Some(index) = fastest {
    candidates[index].fastest_valid = true;
  }
}

fn ordered_f64(value: f64) -> u64 {
  (value * 1_000_000.0).round() as u64
}

fn measure_comparisons(config: BenchmarkConfig) -> Vec<ComparisonResult> {
  let clock = clock::MonotonicClock::default();
  let mut rows = Vec::new();

  push_comparison(&mut rows, config, "hotclock::Instant::now()", "now", || {
    let _ = black_box(Instant::now());
  });
  push_comparison(&mut rows, config, "hotclock::Instant (now + elapsed_ticks)", "elapsed", || {
    let start = Instant::now();
    let _ = black_box(start.elapsed_ticks());
  });
  push_comparison(&mut rows, config, "quanta::Instant::now()", "now", || {
    black_box(quanta::Instant::now());
  });
  push_comparison(&mut rows, config, "quanta::Instant (now + elapsed)", "elapsed", || {
    let start = quanta::Instant::now();
    black_box(start.elapsed());
  });
  push_comparison(&mut rows, config, "minstant::Instant::now()", "now", || {
    black_box(minstant::Instant::now());
  });
  push_comparison(&mut rows, config, "minstant::Instant (now + elapsed)", "elapsed", || {
    let start = minstant::Instant::now();
    black_box(start.elapsed());
  });
  push_comparison(&mut rows, config, "fastant::Instant::now()", "now", || {
    black_box(fastant::Instant::now());
  });
  push_comparison(&mut rows, config, "fastant::Instant (now + elapsed)", "elapsed", || {
    let start = fastant::Instant::now();
    black_box(start.elapsed());
  });
  push_comparison(&mut rows, config, "coarsetime::Instant::now()", "now", || {
    black_box(coarsetime::Instant::now());
  });
  push_comparison(&mut rows, config, "coarsetime::Instant (now + elapsed)", "elapsed", || {
    let start = coarsetime::Instant::now();
    black_box(start.elapsed());
  });
  push_comparison(&mut rows, config, "time::OffsetDateTime::now_utc()", "now", || {
    black_box(time::OffsetDateTime::now_utc());
  });
  push_comparison(&mut rows, config, "clock::MonotonicClock::now()", "now", || {
    black_box(clock.now());
  });
  push_comparison(&mut rows, config, "chrono::Utc::now()", "now", || {
    black_box(chrono::Utc::now());
  });
  push_comparison(&mut rows, config, "clocksource::precise::Instant::now()", "now", || {
    black_box(clocksource::precise::Instant::now());
  });
  push_comparison(
    &mut rows,
    config,
    "clocksource::precise::Instant (now + elapsed)",
    "elapsed",
    || {
      let start = clocksource::precise::Instant::now();
      black_box(start.elapsed());
    },
  );
  push_comparison(&mut rows, config, "tick_counter::start()", "now", || {
    black_box(tick_counter::start());
  });
  push_comparison(
    &mut rows,
    config,
    "tick_counter::TickCounter (current + elapsed)",
    "elapsed",
    || {
      let start = tick_counter::TickCounter::current();
      black_box(start.elapsed());
    },
  );
  push_comparison(&mut rows, config, "std::time::Instant::now()", "now", || {
    black_box(StdInstant::now());
  });
  push_comparison(&mut rows, config, "std::time::Instant (now + elapsed)", "elapsed", || {
    let start = StdInstant::now();
    black_box(start.elapsed());
  });

  rows
}

fn push_comparison<F>(
  rows: &mut Vec<ComparisonResult>,
  config: BenchmarkConfig,
  name: &'static str,
  operation: &'static str,
  f: F,
) where
  F: FnMut(),
{
  eprintln!("measuring {name}");
  rows.push(ComparisonResult { name, operation, stats: measure(config, f) });
}

fn measure<F>(config: BenchmarkConfig, mut f: F) -> Stats
where
  F: FnMut(),
{
  for _ in 0..config.warmup_iters {
    f();
  }

  let mut samples = Vec::with_capacity(config.samples);
  for _ in 0..config.samples {
    let started = StdInstant::now();
    for _ in 0..config.measure_iters {
      f();
    }
    samples.push(started.elapsed().as_nanos() as f64 / config.measure_iters as f64);
  }

  samples.sort_by(f64::total_cmp);
  let stats = sample_stats(&samples);
  Stats {
    best_ns: samples[0],
    mean_ns: stats.mean,
    median_ns: samples[samples.len() / 2],
    worst_ns: samples[samples.len() - 1],
    stddev_ns: stats.stddev,
    ci95_low_ns: stats.ci95_low,
    ci95_high_ns: stats.ci95_high,
    samples: samples.len() as u64,
  }
}

struct SampleStats {
  mean: f64,
  stddev: f64,
  ci95_low: f64,
  ci95_high: f64,
}

fn sample_stats(samples: &[f64]) -> SampleStats {
  let n = samples.len() as f64;
  let mean = samples.iter().sum::<f64>() / n;
  if samples.len() < 2 {
    return SampleStats { mean, stddev: 0.0, ci95_low: mean, ci95_high: mean };
  }

  let variance = samples
    .iter()
    .map(|sample| {
      let delta = *sample - mean;
      delta * delta
    })
    .sum::<f64>()
    / (n - 1.0);
  let stddev = variance.sqrt();
  let margin = t_critical_95(samples.len()) * stddev / n.sqrt();

  SampleStats { mean, stddev, ci95_low: (mean - margin).max(0.0), ci95_high: mean + margin }
}

fn t_critical_95(samples: usize) -> f64 {
  match samples {
    0 | 1 => 0.0,
    2 => 12.706,
    3 => 4.303,
    4 => 3.182,
    5 => 2.776,
    6 => 2.571,
    7 => 2.447,
    8 => 2.365,
    9 => 2.306,
    10 => 2.262,
    11 => 2.228,
    12 => 2.201,
    13 => 2.179,
    14 => 2.160,
    15 => 2.145,
    16 => 2.131,
    17 => 2.120,
    18 => 2.110,
    19 => 2.101,
    20 => 2.093,
    21 => 2.086,
    22 => 2.080,
    23 => 2.074,
    24 => 2.069,
    25 => 2.064,
    26 => 2.060,
    27 => 2.056,
    28 => 2.052,
    29 => 2.048,
    30 => 2.045,
    31 => 2.042,
    _ => 1.960,
  }
}

fn compile_identity() -> Value {
  let arch = std::env::consts::ARCH;
  let os = std::env::consts::OS;
  let family = std::env::consts::FAMILY;
  let env = target_env();
  let vendor = target_vendor();
  let pointer_width = target_pointer_width();
  let endian = target_endian();
  let key = format!("{arch}-{vendor}-{os}-{env}-{pointer_width}-{endian}");

  json!({
    "key": key,
    "arch": arch,
    "os": os,
    "family": family,
    "env": env,
    "vendor": vendor,
    "pointer_width": pointer_width,
    "endian": endian,
    "features": target_features(),
  })
}

fn runtime_identity() -> Value {
  json!({
    "kernel": command_output("uname", &["-a"]).or_else(|| command_output("cmd", &["/C", "ver"])),
    "cpu_model": cpu_model(),
    "container": container_hint(),
    "virtualization": virtualization_hint(),
    "linux_clocksource": linux_clocksource(),
    "macos_rosetta": macos_rosetta(),
    "windows_emulation": windows_emulation(),
    "os_release": os_release(),
  })
}

fn render_markdown(
  compile_identity: &Value,
  runtime_identity: &Value,
  proof_eligible: bool,
  candidates: &[CandidateResult],
  comparison_config: BenchmarkConfig,
  comparisons: &[ComparisonResult],
) -> String {
  let mut out = String::new();
  writeln!(
    out,
    "# hotclock runtime selection report\n\nCompile key: `{}`\nHotclock: `{}`\nFrequency: `{}` Hz\nProof eligible: `{}`\n",
    compile_identity["key"].as_str().unwrap_or("unknown"),
    Instant::implementation(),
    Instant::frequency(),
    proof_eligible
  )
  .unwrap();

  out.push_str("## Runtime identity\n\n");
  out.push_str("| Field | Value |\n");
  out.push_str("|---|---|\n");
  render_runtime_row(&mut out, "os_release", &runtime_identity["os_release"]);
  render_runtime_row(&mut out, "kernel", &runtime_identity["kernel"]);
  render_runtime_row(&mut out, "cpu_model", &runtime_identity["cpu_model"]);
  render_runtime_row(&mut out, "container", &runtime_identity["container"]);
  render_runtime_row(&mut out, "virtualization", &runtime_identity["virtualization"]);
  render_runtime_row(&mut out, "linux_clocksource", &runtime_identity["linux_clocksource"]);
  render_runtime_row(&mut out, "macos_rosetta", &runtime_identity["macos_rosetta"]);
  render_runtime_row(&mut out, "windows_emulation", &runtime_identity["windows_emulation"]);

  out.push_str("\n## Candidate clocks\n\n");
  out.push_str("| Clock | Kind | Candidate | Valid | Precision | Selected | Fastest | Samples | Mean ns | Median ns | 95% CI ns | Best ns | Worst ns |\n");
  out.push_str("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
  for row in candidates {
    writeln!(
      out,
      "| `{}` | {} | {} | {} | {} | {} | {} | {} | {:.3} | {:.3} | {:.3}..{:.3} | {:.3} | {:.3} |",
      row.candidate.name,
      row.candidate.kind.as_str(),
      if row.candidate.selected_by_hotclock { "✅" } else { "❌" },
      if row.valid { "✅" } else { "❌" },
      row.precision_ticks.map_or("n/a".to_string(), |value| value.to_string()),
      if row.selected { "✅" } else { "❌" },
      if row.fastest_valid { "✅" } else { "❌" },
      row.stats.samples,
      row.stats.mean_ns,
      row.stats.median_ns,
      row.stats.ci95_low_ns,
      row.stats.ci95_high_ns,
      row.stats.best_ns,
      row.stats.worst_ns
    )
    .unwrap();
  }

  if !comparisons.is_empty() {
    out.push_str("\n## Comparison timers\n\n");
    writeln!(
      out,
      "Warmup: `{}` calls, measure: `{}` calls/sample, samples: `{}`\n",
      comparison_config.warmup_iters, comparison_config.measure_iters, comparison_config.samples
    )
    .unwrap();
    out.push_str("| Timer | Operation | Samples | Mean ns/call | Median ns/call | 95% CI ns/call | Best ns/call | Worst ns/call |\n");
    out.push_str("|---|---|---:|---:|---:|---:|---:|---:|\n");
    for row in comparisons {
      writeln!(
        out,
        "| `{}` | {} | {} | {:.3} | {:.3} | {:.3}..{:.3} | {:.3} | {:.3} |",
        row.name,
        row.operation,
        row.stats.samples,
        row.stats.mean_ns,
        row.stats.median_ns,
        row.stats.ci95_low_ns,
        row.stats.ci95_high_ns,
        row.stats.best_ns,
        row.stats.worst_ns
      )
      .unwrap();
    }
  }

  out
}

fn render_runtime_row(out: &mut String, field: &str, value: &Value) {
  let value = match value {
    Value::String(value) => value.clone(),
    Value::Null => "n/a".to_string(),
    value => value.to_string(),
  };
  writeln!(out, "| `{field}` | `{}` |", value.replace('\n', " ")).unwrap();
}

fn render_json(
  compile_identity: &Value,
  runtime_identity: &Value,
  proof_eligible: bool,
  candidates: &[CandidateResult],
  comparison_config: BenchmarkConfig,
  comparisons: &[ComparisonResult],
) -> Value {
  json!({
    "schema_version": 1,
    "compile_identity": compile_identity,
    "runtime_identity": runtime_identity,
    "hotclock": {
      "implementation": Instant::implementation(),
      "frequency_hz": Instant::frequency(),
    },
    "proof_eligible": proof_eligible,
    "candidates": candidates.iter().map(candidate_json).collect::<Vec<_>>(),
    "comparison_benchmark": {
      "warmup_iters": comparison_config.warmup_iters,
      "measure_iters": comparison_config.measure_iters,
      "samples": comparison_config.samples,
    },
    "comparisons": comparisons.iter().map(comparison_json).collect::<Vec<_>>(),
  })
}

fn candidate_json(row: &CandidateResult) -> Value {
  json!({
    "name": row.candidate.name,
    "kind": row.candidate.kind.as_str(),
    "production_candidate": row.candidate.selected_by_hotclock,
    "valid": row.valid,
    "precision_ticks": row.precision_ticks,
    "selected": row.selected,
    "fastest_valid": row.fastest_valid,
    "best_ns": row.stats.best_ns,
    "mean_ns": row.stats.mean_ns,
    "median_ns": row.stats.median_ns,
    "worst_ns": row.stats.worst_ns,
    "stddev_ns": row.stats.stddev_ns,
    "ci95_low_ns": row.stats.ci95_low_ns,
    "ci95_high_ns": row.stats.ci95_high_ns,
    "samples": row.stats.samples,
  })
}

fn comparison_json(row: &ComparisonResult) -> Value {
  json!({
    "name": row.name,
    "operation": row.operation,
    "best_ns": row.stats.best_ns,
    "mean_ns": row.stats.mean_ns,
    "median_ns": row.stats.median_ns,
    "worst_ns": row.stats.worst_ns,
    "stddev_ns": row.stats.stddev_ns,
    "ci95_low_ns": row.stats.ci95_low_ns,
    "ci95_high_ns": row.stats.ci95_high_ns,
    "samples": row.stats.samples,
  })
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

fn target_vendor() -> &'static str {
  if cfg!(target_vendor = "apple") {
    "apple"
  } else if cfg!(target_vendor = "pc") {
    "pc"
  } else {
    "unknown"
  }
}

fn target_pointer_width() -> &'static str {
  if cfg!(target_pointer_width = "64") {
    "64"
  } else if cfg!(target_pointer_width = "32") {
    "32"
  } else {
    "unknown"
  }
}

fn target_endian() -> &'static str {
  if cfg!(target_endian = "little") { "little" } else { "big" }
}

fn target_features() -> Vec<&'static str> {
  let mut features = Vec::new();
  push_feature(&mut features, cfg!(target_feature = "sse2"), "sse2");
  push_feature(&mut features, cfg!(target_feature = "avx"), "avx");
  push_feature(&mut features, cfg!(target_feature = "avx2"), "avx2");
  push_feature(&mut features, cfg!(target_feature = "neon"), "neon");
  push_feature(&mut features, cfg!(target_feature = "lse"), "lse");
  features
}

fn push_feature(features: &mut Vec<&'static str>, present: bool, name: &'static str) {
  if present {
    features.push(name);
  }
}

fn os_release() -> Option<String> {
  #[cfg(target_os = "linux")]
  {
    read_os_release()
  }
  #[cfg(target_os = "macos")]
  {
    command_output("sw_vers", &["-productVersion"])
  }
  #[cfg(target_os = "windows")]
  {
    command_output("cmd", &["/C", "ver"])
  }
  #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
  {
    None
  }
}

fn cpu_model() -> Option<String> {
  #[cfg(target_os = "linux")]
  {
    read_cpuinfo_field("model name").or_else(|| read_cpuinfo_field("Processor"))
  }
  #[cfg(target_os = "macos")]
  {
    command_output("sysctl", &["-n", "machdep.cpu.brand_string"])
      .or_else(|| command_output("sysctl", &["-n", "hw.model"]))
  }
  #[cfg(target_os = "windows")]
  {
    command_output("powershell", &[
      "-NoProfile",
      "-Command",
      "(Get-CimInstance Win32_Processor).Name",
    ])
  }
  #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
  {
    None
  }
}

fn container_hint() -> Option<String> {
  #[cfg(target_os = "linux")]
  {
    let mut hints = Vec::new();
    if std::path::Path::new("/.dockerenv").exists() {
      hints.push("dockerenv".to_string());
    }
    if std::path::Path::new("/run/.containerenv").exists() {
      hints.push("containerenv".to_string());
    }
    if let Ok(cgroup) = std::fs::read_to_string("/proc/1/cgroup") {
      if cgroup.contains("docker") {
        hints.push("cgroup:docker".to_string());
      }
      if cgroup.contains("kubepods") {
        hints.push("cgroup:kubernetes".to_string());
      }
      if cgroup.contains("containerd") {
        hints.push("cgroup:containerd".to_string());
      }
    }
    if hints.is_empty() { None } else { Some(hints.join(",")) }
  }
  #[cfg(not(target_os = "linux"))]
  {
    None
  }
}

fn virtualization_hint() -> Option<String> {
  #[cfg(target_os = "linux")]
  {
    let mut hints = Vec::new();
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo")
      && cpuinfo.contains(" hypervisor")
    {
      hints.push("cpuinfo:hypervisor".to_string());
    }
    for path in ["/sys/class/dmi/id/sys_vendor", "/sys/class/dmi/id/product_name"] {
      if let Ok(value) = std::fs::read_to_string(path) {
        let value = value.trim();
        if !value.is_empty() {
          hints.push(format!("{path}:{value}"));
        }
      }
    }
    if hints.is_empty() { None } else { Some(hints.join(",")) }
  }
  #[cfg(target_os = "macos")]
  {
    command_output("sysctl", &["-n", "kern.hv_vmm_present"])
      .filter(|value| value == "1")
      .map(|_| "kern.hv_vmm_present=1".to_string())
  }
  #[cfg(target_os = "windows")]
  {
    std::env::var("PROCESSOR_IDENTIFIER").ok()
  }
  #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
  {
    None
  }
}

fn linux_clocksource() -> Value {
  #[cfg(target_os = "linux")]
  {
    json!({
      "current": read_trimmed("/sys/devices/system/clocksource/clocksource0/current_clocksource"),
      "available": read_trimmed("/sys/devices/system/clocksource/clocksource0/available_clocksource"),
    })
  }
  #[cfg(not(target_os = "linux"))]
  {
    Value::Null
  }
}

fn macos_rosetta() -> Value {
  #[cfg(target_os = "macos")]
  {
    let translated = command_output("sysctl", &["-in", "sysctl.proc_translated"]);
    json!({
      "translated": translated.as_deref() == Some("1"),
      "sysctl": translated,
      "machine": command_output("uname", &["-m"]),
    })
  }
  #[cfg(not(target_os = "macos"))]
  {
    Value::Null
  }
}

fn windows_emulation() -> Value {
  #[cfg(target_os = "windows")]
  {
    json!({
      "PROCESSOR_ARCHITECTURE": std::env::var("PROCESSOR_ARCHITECTURE").ok(),
      "PROCESSOR_ARCHITEW6432": std::env::var("PROCESSOR_ARCHITEW6432").ok(),
      "PROCESSOR_IDENTIFIER": std::env::var("PROCESSOR_IDENTIFIER").ok(),
    })
  }
  #[cfg(not(target_os = "windows"))]
  {
    Value::Null
  }
}

#[cfg(target_os = "linux")]
fn read_os_release() -> Option<String> {
  let contents = std::fs::read_to_string("/etc/os-release").ok()?;
  for line in contents.lines() {
    if let Some(value) = line.strip_prefix("PRETTY_NAME=") {
      return Some(value.trim_matches('"').to_string());
    }
  }
  None
}

#[cfg(target_os = "linux")]
fn read_cpuinfo_field(field: &str) -> Option<String> {
  let contents = std::fs::read_to_string("/proc/cpuinfo").ok()?;
  let prefix = format!("{field}\t:");
  for line in contents.lines() {
    if let Some(value) = line.strip_prefix(&prefix) {
      return Some(value.trim().to_string());
    }
  }
  None
}

#[cfg(target_os = "linux")]
fn read_trimmed(path: &str) -> Option<String> {
  std::fs::read_to_string(path).ok().map(|value| value.trim().to_string())
}

fn command_output(command: &str, args: &[&str]) -> Option<String> {
  let output = Command::new(command).args(args).output().ok()?;
  if !output.status.success() {
    return None;
  }

  let value = String::from_utf8(output.stdout).ok()?;
  let value = value.trim();
  if value.is_empty() { None } else { Some(value.to_string()) }
}

fn skip_comparisons() -> bool {
  std::env::var_os("HOTCLOCK_SKIP_COMPARISONS").is_some()
}

fn comparison_config() -> BenchmarkConfig {
  BenchmarkConfig {
    warmup_iters: env_usize("HOTCLOCK_COMPARE_WARMUP_ITERS", DEFAULT_WARMUP_ITERS),
    measure_iters: env_usize("HOTCLOCK_COMPARE_MEASURE_ITERS", DEFAULT_MEASURE_ITERS),
    samples: env_usize("HOTCLOCK_COMPARE_SAMPLES", DEFAULT_SAMPLES),
  }
}

fn env_usize(name: &str, default: usize) -> usize {
  std::env::var(name)
    .ok()
    .and_then(|value| value.parse().ok())
    .filter(|value| *value > 0)
    .unwrap_or(default)
}
