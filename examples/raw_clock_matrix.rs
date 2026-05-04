#![cfg(feature = "bench-internals")]
#![allow(clippy::cast_precision_loss)]

use std::fmt::Write as _;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant as StdInstant;

use hotclock::bench_internals::{ClockCandidate, candidate_clocks};
use serde_json::{Value, json};

const DEFAULT_WARMUP_ITERS: usize = 10_000;
const DEFAULT_MEASURE_ITERS: usize = 50_000;
const DEFAULT_SAMPLES: usize = 31;
const DEFAULT_ATTEMPTS: usize = 3;
const DEFAULT_MAX_RELATIVE_MARGIN: f64 = 0.05;
const DEFAULT_MAX_ABSOLUTE_MARGIN_NS: f64 = 1.0;

#[derive(Clone, Copy)]
struct BenchmarkConfig {
  warmup_iters: usize,
  measure_iters: usize,
  samples: usize,
  attempts: usize,
  max_relative_margin: f64,
  max_absolute_margin_ns: f64,
}

#[derive(Clone, Copy)]
struct Stats {
  ns_op: f64,
  best_ns: f64,
  mean_ns: f64,
  worst_ns: f64,
  stddev_ns: f64,
  ci95_low_ns: f64,
  ci95_high_ns: f64,
  confidence_margin_ns: f64,
  confidence_score: f64,
  confidence_high: bool,
  samples: u64,
}

struct ClockRow {
  source: &'static str,
  name: &'static str,
  operation: &'static str,
  stats: Stats,
}

fn main() -> std::io::Result<()> {
  let config = benchmark_config();
  let compile_identity = compile_identity();
  let runtime_identity = runtime_identity();
  let environment_name = environment_name();
  let rows = measure_rows(config);
  let markdown = render_markdown(&environment_name, &compile_identity, &runtime_identity, &rows);
  let json = render_json(&environment_name, &compile_identity, &runtime_identity, config, &rows);

  println!("{markdown}");

  if let Some(report_dir) = std::env::var_os("HOTCLOCK_RAW_CLOCK_REPORT_DIR") {
    let report_dir = PathBuf::from(report_dir);
    std::fs::create_dir_all(&report_dir)?;
    std::fs::write(report_dir.join("raw-clock-report.md"), &markdown)?;
    std::fs::write(report_dir.join("raw-clock-report.json"), json.to_string())?;
  }

  Ok(())
}

fn measure_rows(config: BenchmarkConfig) -> Vec<ClockRow> {
  let mut rows = Vec::new();

  for candidate in candidate_clocks() {
    push_candidate(&mut rows, config, *candidate);
  }

  black_box(quanta::Instant::now());
  black_box(minstant::Instant::now());
  black_box(fastant::Instant::now());
  let quanta_clock = quanta::Clock::new();

  push_measurement(&mut rows, config, "quanta", "quanta::Instant::now()", "now", || {
    black_box(quanta::Instant::now());
  });
  push_measurement(&mut rows, config, "quanta", "quanta::Clock::now()", "now", || {
    black_box(quanta_clock.now());
  });
  push_measurement(&mut rows, config, "quanta", "quanta::Clock::raw()", "raw", || {
    black_box(quanta_clock.raw());
  });
  push_measurement(&mut rows, config, "minstant", "minstant::Instant::now()", "now", || {
    black_box(minstant::Instant::now());
  });
  push_measurement(&mut rows, config, "fastant", "fastant::Instant::now()", "now", || {
    black_box(fastant::Instant::now());
  });

  rows
}

fn push_candidate(rows: &mut Vec<ClockRow>, config: BenchmarkConfig, candidate: ClockCandidate) {
  push_measurement(rows, config, "hotclock", candidate.name, "read", || {
    black_box((candidate.read)());
  });
}

fn push_measurement<F>(
  rows: &mut Vec<ClockRow>,
  config: BenchmarkConfig,
  source: &'static str,
  name: &'static str,
  operation: &'static str,
  mut f: F,
) where
  F: FnMut(),
{
  eprintln!("measuring {name}");
  rows.push(ClockRow { source, name, operation, stats: measure_stable(config, &mut f) });
}

fn measure_stable<F>(config: BenchmarkConfig, f: &mut F) -> Stats
where
  F: FnMut(),
{
  let mut best = measure_once(config, f);
  if best.confidence_high {
    return best;
  }

  for _ in 1..config.attempts {
    let candidate = measure_once(config, f);
    if candidate.confidence_high {
      return candidate;
    }
    if candidate.confidence_score > best.confidence_score {
      best = candidate;
    }
  }

  best
}

fn measure_once<F>(config: BenchmarkConfig, f: &mut F) -> Stats
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
  let ns_op = samples[samples.len() / 2];
  let sample_stats = sample_stats(&samples);
  let confidence_margin_ns = (sample_stats.ci95_high - sample_stats.ci95_low) / 2.0;
  let confidence_ratio = if ns_op > 0.0 { confidence_margin_ns / ns_op } else { 0.0 };
  let confidence_score = (1.0 - confidence_ratio).clamp(0.0, 1.0);
  let max_margin = config.max_absolute_margin_ns.max(ns_op * config.max_relative_margin);

  Stats {
    ns_op,
    best_ns: samples[0],
    mean_ns: sample_stats.mean,
    worst_ns: samples[samples.len() - 1],
    stddev_ns: sample_stats.stddev,
    ci95_low_ns: sample_stats.ci95_low,
    ci95_high_ns: sample_stats.ci95_high,
    confidence_margin_ns,
    confidence_score,
    confidence_high: confidence_margin_ns <= max_margin,
    samples: samples.len() as u64,
  }
}

#[derive(Clone, Copy)]
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

fn render_markdown(
  environment_name: &str,
  compile_identity: &Value,
  runtime_identity: &Value,
  rows: &[ClockRow],
) -> String {
  let mut out = String::new();
  writeln!(
    out,
    "# hotclock raw clock benchmark\n\nEnvironment: `{}`\nCompile key: `{}`\n",
    environment_name,
    compile_identity["key"].as_str().unwrap_or("unknown")
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

  out.push_str("\n## Clocks\n\n");
  out.push_str("| Clock | Source | Operation | ns/op |\n");
  out.push_str("|---|---|---|---:|\n");
  for row in rows {
    writeln!(
      out,
      "| `{}` | {} | {} | {:.3} |",
      row.name, row.source, row.operation, row.stats.ns_op
    )
    .unwrap();
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
  environment_name: &str,
  compile_identity: &Value,
  runtime_identity: &Value,
  config: BenchmarkConfig,
  rows: &[ClockRow],
) -> Value {
  json!({
    "schema_version": 1,
    "environment_name": environment_name,
    "compile_identity": compile_identity,
    "runtime_identity": runtime_identity,
    "benchmark": {
      "warmup_iters": config.warmup_iters,
      "measure_iters": config.measure_iters,
      "samples": config.samples,
      "attempts": config.attempts,
      "max_relative_margin": config.max_relative_margin,
      "max_absolute_margin_ns": config.max_absolute_margin_ns,
    },
    "clocks": rows.iter().map(row_json).collect::<Vec<_>>(),
  })
}

fn row_json(row: &ClockRow) -> Value {
  json!({
    "source": row.source,
    "name": row.name,
    "operation": row.operation,
    "ns_op": row.stats.ns_op,
    "best_ns": row.stats.best_ns,
    "mean_ns": row.stats.mean_ns,
    "worst_ns": row.stats.worst_ns,
    "stddev_ns": row.stats.stddev_ns,
    "ci95_low_ns": row.stats.ci95_low_ns,
    "ci95_high_ns": row.stats.ci95_high_ns,
    "confidence_margin_ns": row.stats.confidence_margin_ns,
    "confidence_score": row.stats.confidence_score,
    "confidence_high": row.stats.confidence_high,
    "samples": row.stats.samples,
  })
}

fn environment_name() -> String {
  std::env::var("HOTCLOCK_RAW_CLOCK_ENVIRONMENT").unwrap_or_else(|_| "local".to_string())
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
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
      if cpuinfo.contains(" hypervisor") {
        hints.push("cpuinfo:hypervisor".to_string());
      }
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

fn benchmark_config() -> BenchmarkConfig {
  BenchmarkConfig {
    warmup_iters: env_usize("HOTCLOCK_RAW_CLOCK_WARMUP_ITERS", DEFAULT_WARMUP_ITERS),
    measure_iters: env_usize("HOTCLOCK_RAW_CLOCK_MEASURE_ITERS", DEFAULT_MEASURE_ITERS),
    samples: env_usize("HOTCLOCK_RAW_CLOCK_SAMPLES", DEFAULT_SAMPLES),
    attempts: env_usize("HOTCLOCK_RAW_CLOCK_ATTEMPTS", DEFAULT_ATTEMPTS),
    max_relative_margin: env_f64(
      "HOTCLOCK_RAW_CLOCK_MAX_RELATIVE_MARGIN",
      DEFAULT_MAX_RELATIVE_MARGIN,
    ),
    max_absolute_margin_ns: env_f64(
      "HOTCLOCK_RAW_CLOCK_MAX_ABSOLUTE_MARGIN_NS",
      DEFAULT_MAX_ABSOLUTE_MARGIN_NS,
    ),
  }
}

fn env_usize(name: &str, default: usize) -> usize {
  std::env::var(name)
    .ok()
    .and_then(|value| value.parse().ok())
    .filter(|value| *value > 0)
    .unwrap_or(default)
}

fn env_f64(name: &str, default: f64) -> f64 {
  std::env::var(name)
    .ok()
    .and_then(|value| value.parse().ok())
    .filter(|value| *value > 0.0)
    .unwrap_or(default)
}
