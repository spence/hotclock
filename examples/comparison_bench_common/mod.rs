use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant as StdInstant;

use serde_json::{Value, json};

const DEFAULT_WARMUP_ITERS: usize = 1_000;
const DEFAULT_MEASURE_ITERS: usize = 5_000;
const DEFAULT_SAMPLES: usize = 5;

#[derive(Clone, Copy)]
pub struct BenchmarkConfig {
  warmup_iters: usize,
  measure_iters: usize,
  samples: usize,
}

#[derive(Clone, Copy)]
struct Stats {
  best_ns: f64,
  median_ns: f64,
  worst_ns: f64,
}

pub fn measure<F>(name: &'static str, operation: &'static str, mut f: F) -> Value
where
  F: FnMut(),
{
  eprintln!("measuring {name}");
  let config = benchmark_config();

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
  let stats = Stats {
    best_ns: samples[0],
    median_ns: samples[samples.len() / 2],
    worst_ns: samples[samples.len() - 1],
  };

  json!({
    "name": name,
    "operation": operation,
    "best_ns": stats.best_ns,
    "median_ns": stats.median_ns,
    "worst_ns": stats.worst_ns,
  })
}

pub fn write_report(timer: &'static str, comparisons: Vec<Value>) -> std::io::Result<()> {
  let config = benchmark_config();
  let report = json!({
    "schema_version": 1,
    "timer": timer,
    "status": "ok",
    "compile_identity": compile_identity(),
    "runtime_identity": runtime_identity(),
    "comparison_benchmark": {
      "warmup_iters": config.warmup_iters,
      "measure_iters": config.measure_iters,
      "samples": config.samples,
    },
    "comparisons": comparisons,
  });
  let markdown = render_markdown(&report);

  println!("{markdown}");

  if let Some(report_dir) = std::env::var_os("HOTCLOCK_COMPARISON_DIR") {
    let report_dir = PathBuf::from(report_dir);
    std::fs::create_dir_all(&report_dir)?;
    std::fs::write(report_dir.join("comparison-report.md"), &markdown)?;
    std::fs::write(report_dir.join("comparison-report.json"), report.to_string())?;
  }

  Ok(())
}

fn render_markdown(report: &Value) -> String {
  let mut out = String::new();
  writeln!(
    out,
    "# hotclock comparison benchmark\n\nTimer: `{}`\nCompile key: `{}`\n",
    report["timer"].as_str().unwrap_or("unknown"),
    report["compile_identity"]["key"].as_str().unwrap_or("unknown")
  )
  .unwrap();

  out.push_str("| Operation | Best ns/call | Median ns/call | Worst ns/call |\n");
  out.push_str("|---|---:|---:|---:|\n");
  if let Some(comparisons) = report["comparisons"].as_array() {
    for row in comparisons {
      writeln!(
        out,
        "| `{}` | {:.3} | {:.3} | {:.3} |",
        row["name"].as_str().unwrap_or("unknown"),
        row["best_ns"].as_f64().unwrap_or(0.0),
        row["median_ns"].as_f64().unwrap_or(0.0),
        row["worst_ns"].as_f64().unwrap_or(0.0),
      )
      .unwrap();
    }
  }

  out
}

fn benchmark_config() -> BenchmarkConfig {
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
  })
}

fn runtime_identity() -> Value {
  json!({
    "kernel": command_output("uname", &["-a"]),
    "os_release": os_release(),
    "container": container_hint(),
    "virtualization": virtualization_hint(),
    "linux_clocksource": linux_clocksource(),
  })
}

fn target_env() -> &'static str {
  if cfg!(target_env = "gnu") {
    "gnu"
  } else if cfg!(target_env = "musl") {
    "musl"
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
  if cfg!(target_pointer_width = "64") { "64" } else { "32" }
}

fn target_endian() -> &'static str {
  if cfg!(target_endian = "little") { "little" } else { "big" }
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

fn os_release() -> Option<String> {
  let contents = std::fs::read_to_string("/etc/os-release").ok()?;
  contents.lines().find_map(|line| {
    line
      .strip_prefix("PRETTY_NAME=")
      .map(|value| value.trim_matches('"').to_string())
  })
}

fn container_hint() -> Option<String> {
  if std::path::Path::new("/.dockerenv").exists() { Some("dockerenv".to_string()) } else { None }
}

fn virtualization_hint() -> Option<String> {
  let mut hints = Vec::new();
  push_trimmed(&mut hints, "/sys/class/dmi/id/sys_vendor");
  push_trimmed(&mut hints, "/sys/class/dmi/id/product_name");
  if hints.is_empty() { None } else { Some(hints.join(",")) }
}

fn push_trimmed(hints: &mut Vec<String>, path: &str) {
  if let Ok(value) = std::fs::read_to_string(path) {
    let value = value.trim();
    if !value.is_empty() {
      hints.push(format!("{path}:{value}"));
    }
  }
}

fn linux_clocksource() -> Value {
  json!({
    "current": read_trimmed("/sys/devices/system/clocksource/clocksource0/current_clocksource"),
    "available": read_trimmed("/sys/devices/system/clocksource/clocksource0/available_clocksource"),
  })
}

fn read_trimmed(path: &str) -> Option<String> {
  std::fs::read_to_string(path).ok().map(|value| value.trim().to_string())
}
