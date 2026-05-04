#![cfg(feature = "bench-internals")]
#![allow(clippy::cast_precision_loss)]

use std::fmt::Write as _;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant as StdInstant;

use hotclock::bench_internals::{ClockCandidate, ClockCandidateKind, candidate_clocks};
use serde_json::{Value, json};

const DEFAULT_WARMUP_ITERS: usize = 10_000;
const DEFAULT_MEASURE_ITERS: usize = 50_000;
const DEFAULT_SAMPLES: usize = 31;
const DEFAULT_ATTEMPTS: usize = 3;
const DEFAULT_MAX_RELATIVE_MARGIN: f64 = 0.05;
const DEFAULT_MAX_ABSOLUTE_MARGIN_NS: f64 = 1.0;
const CHILD_CLOCK_ENV: &str = "HOTCLOCK_RAW_CLOCK_CHILD_CLOCK";
const CHILD_WRAPPER_ENV: &str = "HOTCLOCK_RAW_CLOCK_CHILD_WRAPPER";
const STDOUT_FORMAT_ENV: &str = "HOTCLOCK_RAW_CLOCK_STDOUT";

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
  kind: Option<ClockCandidateKind>,
  selected_by_hotclock: Option<bool>,
  stats: Option<Stats>,
  error: Option<String>,
}

pub fn main() -> std::io::Result<()> {
  let config = benchmark_config();

  if let Ok(clock_name) = std::env::var(CHILD_CLOCK_ENV) {
    return run_child_clock(config, &clock_name);
  }

  let compile_identity = compile_identity();
  let runtime_identity = runtime_identity();
  let environment_name = environment_name();
  let rows = measure_rows(config);
  let markdown = render_markdown(&environment_name, &compile_identity, &runtime_identity, &rows);
  let json = render_json(&environment_name, &compile_identity, &runtime_identity, config, &rows);

  print_report(&markdown, &json);

  if let Some(report_dir) = std::env::var_os("HOTCLOCK_RAW_CLOCK_REPORT_DIR") {
    let report_dir = PathBuf::from(report_dir);
    std::fs::create_dir_all(&report_dir)?;
    std::fs::write(report_dir.join("raw-clock-report.md"), &markdown)?;
    std::fs::write(report_dir.join("raw-clock-report.json"), json.to_string())?;
  }

  Ok(())
}

fn print_report(markdown: &str, json: &Value) {
  match std::env::var(STDOUT_FORMAT_ENV).as_deref() {
    Ok("json") => println!("{json}"),
    _ => println!("{markdown}"),
  }
}

fn measure_rows(config: BenchmarkConfig) -> Vec<ClockRow> {
  let mut rows = Vec::new();

  for candidate in candidate_clocks() {
    push_candidate(&mut rows, config, *candidate);
  }

  push_comparison_crates(&mut rows, config);

  rows
}

fn push_candidate(rows: &mut Vec<ClockRow>, config: BenchmarkConfig, candidate: ClockCandidate) {
  rows.push(measure_candidate_in_child(config, candidate));
}

fn run_child_clock(config: BenchmarkConfig, clock_name: &str) -> std::io::Result<()> {
  let candidate = candidate_clocks()
    .iter()
    .copied()
    .find(|candidate| candidate.name == clock_name)
    .ok_or_else(|| std::io::Error::other(format!("unknown hotclock candidate `{clock_name}`")))?;
  let row = measure_candidate_direct(config, candidate);
  println!("{}", row_json(&row));
  Ok(())
}

fn measure_candidate_in_child(_config: BenchmarkConfig, candidate: ClockCandidate) -> ClockRow {
  match run_candidate_child(candidate.name) {
    Ok(stats) => successful_candidate_row(candidate, stats),
    Err(error) => {
      eprintln!("failed {}: {error}", candidate.name);
      failed_candidate_row(candidate, error)
    }
  }
}

fn run_candidate_child(clock_name: &str) -> Result<Stats, String> {
  let current_exe =
    std::env::current_exe().map_err(|error| format!("current_exe failed: {error}"))?;
  let mut command = child_command(current_exe)?;
  let output = command
    .env(CHILD_CLOCK_ENV, clock_name)
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())
    .output()
    .map_err(|error| format!("child spawn failed: {error}"))?;

  if !output.status.success() {
    return Err(format!("child exited with {}", output.status));
  }

  let value: Value = serde_json::from_slice(&output.stdout)
    .map_err(|error| format!("child emitted invalid JSON: {error}"))?;
  stats_from_json(&value).ok_or_else(|| "child JSON did not contain stats".to_string())
}

fn child_command(current_exe: PathBuf) -> Result<Command, String> {
  let Some(wrapper) = std::env::var_os(CHILD_WRAPPER_ENV) else {
    return Ok(Command::new(current_exe));
  };
  let wrapper = wrapper.to_string_lossy();
  let mut parts = wrapper.split_whitespace();
  let program = parts.next().ok_or_else(|| format!("{CHILD_WRAPPER_ENV} was empty"))?;
  let mut command = Command::new(program);
  command.args(parts);
  command.arg(current_exe);
  Ok(command)
}

fn measure_candidate_direct(config: BenchmarkConfig, candidate: ClockCandidate) -> ClockRow {
  if let Some(prepare) = candidate.prepare {
    prepare();
  }

  successful_candidate_row(
    candidate,
    measure_stable(config, &mut || {
      black_box((candidate.read)());
    }),
  )
}

fn successful_candidate_row(candidate: ClockCandidate, stats: Stats) -> ClockRow {
  ClockRow {
    source: "hotclock",
    name: candidate.name,
    operation: "read",
    kind: Some(candidate.kind),
    selected_by_hotclock: Some(candidate.selected_by_hotclock),
    stats: Some(stats),
    error: None,
  }
}

fn failed_candidate_row(candidate: ClockCandidate, error: String) -> ClockRow {
  ClockRow {
    source: "hotclock",
    name: candidate.name,
    operation: "read",
    kind: Some(candidate.kind),
    selected_by_hotclock: Some(candidate.selected_by_hotclock),
    stats: None,
    error: Some(error),
  }
}

#[cfg(all(
  feature = "comparison-crates",
  any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")
))]
fn push_comparison_crates(rows: &mut Vec<ClockRow>, config: BenchmarkConfig) {
  black_box(StdInstant::now());
  black_box(quanta::Instant::now());
  black_box(minstant::Instant::now());
  black_box(fastant::Instant::now());
  let quanta_clock = quanta::Clock::new();

  push_measurement(rows, config, "std", "std::time::Instant::now()", "now", None, None, || {
    black_box(StdInstant::now());
  });
  push_measurement(rows, config, "quanta", "quanta::Instant::now()", "now", None, None, || {
    black_box(quanta::Instant::now());
  });
  push_measurement(rows, config, "quanta", "quanta::Clock::now()", "now", None, None, || {
    black_box(quanta_clock.now());
  });
  push_measurement(rows, config, "quanta", "quanta::Clock::raw()", "raw", None, None, || {
    black_box(quanta_clock.raw());
  });
  push_measurement(rows, config, "minstant", "minstant::Instant::now()", "now", None, None, || {
    black_box(minstant::Instant::now());
  });
  push_measurement(rows, config, "fastant", "fastant::Instant::now()", "now", None, None, || {
    black_box(fastant::Instant::now());
  });
}

#[cfg(not(all(
  feature = "comparison-crates",
  any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")
)))]
fn push_comparison_crates(_rows: &mut Vec<ClockRow>, _config: BenchmarkConfig) {}

#[allow(dead_code, clippy::too_many_arguments)]
fn push_measurement<F>(
  rows: &mut Vec<ClockRow>,
  config: BenchmarkConfig,
  source: &'static str,
  name: &'static str,
  operation: &'static str,
  kind: Option<ClockCandidateKind>,
  selected_by_hotclock: Option<bool>,
  mut f: F,
) where
  F: FnMut(),
{
  eprintln!("measuring {name}");
  rows.push(ClockRow {
    source,
    name,
    operation,
    kind,
    selected_by_hotclock,
    stats: Some(measure_stable(config, &mut f)),
    error: None,
  });
}

fn row_kind(row: &ClockRow) -> &'static str {
  row.kind.map(ClockCandidateKind::as_str).unwrap_or("n/a")
}

fn row_selected_by_hotclock(row: &ClockRow) -> &'static str {
  match row.selected_by_hotclock {
    Some(true) => "yes",
    Some(false) => "no",
    None => "n/a",
  }
}

fn row_kind_json(row: &ClockRow) -> Value {
  row.kind.map_or(Value::Null, |kind| json!(kind.as_str()))
}

fn row_selected_by_hotclock_json(row: &ClockRow) -> Value {
  row.selected_by_hotclock.map_or(Value::Null, |selected| json!(selected))
}

fn stats_from_json(value: &Value) -> Option<Stats> {
  Some(Stats {
    ns_op: value["ns_op"].as_f64()?,
    best_ns: value["best_ns"].as_f64()?,
    mean_ns: value["mean_ns"].as_f64()?,
    worst_ns: value["worst_ns"].as_f64()?,
    stddev_ns: value["stddev_ns"].as_f64()?,
    ci95_low_ns: value["ci95_low_ns"].as_f64()?,
    ci95_high_ns: value["ci95_high_ns"].as_f64()?,
    confidence_margin_ns: value["confidence_margin_ns"].as_f64()?,
    confidence_score: value["confidence_score"].as_f64()?,
    confidence_high: value["confidence_high"].as_bool()?,
    samples: value["samples"].as_u64()?,
  })
}

fn ns_op_cell(row: &ClockRow) -> String {
  match row.stats {
    Some(stats) => format!("{:.3}", stats.ns_op),
    None => "failed".to_string(),
  }
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
  render_runtime_row(
    &mut out,
    "linux_perf_event_paranoid",
    &runtime_identity["linux_perf_event_paranoid"],
  );
  render_runtime_row(
    &mut out,
    "linux_perf_user_access",
    &runtime_identity["linux_perf_user_access"],
  );
  render_runtime_row(&mut out, "linux_rdpmc", &runtime_identity["linux_rdpmc"]);
  render_runtime_row(&mut out, "child_wrapper", &runtime_identity["child_wrapper"]);
  render_runtime_row(&mut out, "macos_rosetta", &runtime_identity["macos_rosetta"]);
  render_runtime_row(&mut out, "windows_emulation", &runtime_identity["windows_emulation"]);

  out.push_str("\n## Clocks\n\n");
  out.push_str("| Clock | Source | Kind | Hotclock selectable | Operation | ns/op |\n");
  out.push_str("|---|---|---|---|---|---:|\n");
  for row in rows {
    writeln!(
      out,
      "| `{}` | {} | {} | {} | {} | {} |",
      row.name,
      row.source,
      row_kind(row),
      row_selected_by_hotclock(row),
      row.operation,
      ns_op_cell(row)
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
  let mut value = json!({
    "source": row.source,
    "name": row.name,
    "operation": row.operation,
    "kind": row_kind_json(row),
    "selected_by_hotclock": row_selected_by_hotclock_json(row),
    "error": row.error,
  });

  if let Some(stats) = row.stats {
    value["ns_op"] = json!(stats.ns_op);
    value["best_ns"] = json!(stats.best_ns);
    value["mean_ns"] = json!(stats.mean_ns);
    value["worst_ns"] = json!(stats.worst_ns);
    value["stddev_ns"] = json!(stats.stddev_ns);
    value["ci95_low_ns"] = json!(stats.ci95_low_ns);
    value["ci95_high_ns"] = json!(stats.ci95_high_ns);
    value["confidence_margin_ns"] = json!(stats.confidence_margin_ns);
    value["confidence_score"] = json!(stats.confidence_score);
    value["confidence_high"] = json!(stats.confidence_high);
    value["samples"] = json!(stats.samples);
  } else {
    value["ns_op"] = Value::Null;
    value["confidence_high"] = json!(false);
  }

  value
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
    "linux_perf_event_paranoid": linux_perf_event_paranoid(),
    "linux_perf_user_access": linux_perf_user_access(),
    "linux_rdpmc": linux_rdpmc(),
    "child_wrapper": std::env::var(CHILD_WRAPPER_ENV).ok(),
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

fn linux_perf_event_paranoid() -> Value {
  #[cfg(target_os = "linux")]
  {
    json!(read_trimmed("/proc/sys/kernel/perf_event_paranoid"))
  }
  #[cfg(not(target_os = "linux"))]
  {
    Value::Null
  }
}

fn linux_perf_user_access() -> Value {
  #[cfg(target_os = "linux")]
  {
    json!(read_trimmed("/proc/sys/kernel/perf_user_access"))
  }
  #[cfg(not(target_os = "linux"))]
  {
    Value::Null
  }
}

fn linux_rdpmc() -> Value {
  #[cfg(target_os = "linux")]
  {
    json!({
      "event_source": read_trimmed("/sys/bus/event_source/devices/cpu/rdpmc"),
      "devices": read_trimmed("/sys/devices/cpu/rdpmc"),
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
