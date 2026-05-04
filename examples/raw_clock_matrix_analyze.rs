#![cfg(feature = "bench-internals")]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

#[derive(Clone, Debug)]
struct RawClockReport {
  path: PathBuf,
  environment_name: String,
  compile_key: String,
  runtime: String,
  clocks: Vec<RawClockRow>,
}

#[derive(Clone, Debug)]
struct RawClockRow {
  source: String,
  name: String,
  operation: String,
  ns_op: f64,
  confidence_high: bool,
}

#[derive(Clone, Debug)]
struct RawClockGroup {
  compile_key: String,
  reports: Vec<RawClockReport>,
}

fn main() -> std::io::Result<()> {
  let roots = roots_from_args();
  let reports = load_reports(&roots)?;
  let groups = group_reports(reports);
  let markdown = render_markdown(&groups);
  let json = render_json(&groups);

  println!("{markdown}");

  if let Some(out_dir) = std::env::var_os("HOTCLOCK_RAW_CLOCK_MATRIX_DIR") {
    let out_dir = PathBuf::from(out_dir);
    std::fs::create_dir_all(&out_dir)?;
    std::fs::write(out_dir.join("raw-clock-matrix.md"), &markdown)?;
    std::fs::write(out_dir.join("raw-clock-matrix.json"), json.to_string())?;
  }

  Ok(())
}

fn roots_from_args() -> Vec<PathBuf> {
  let roots: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
  if roots.is_empty() { vec![PathBuf::from("target")] } else { roots }
}

fn load_reports(roots: &[PathBuf]) -> std::io::Result<Vec<RawClockReport>> {
  let mut reports = Vec::new();
  for root in roots {
    collect_reports(root, &mut reports)?;
  }
  Ok(reports)
}

fn collect_reports(path: &Path, reports: &mut Vec<RawClockReport>) -> std::io::Result<()> {
  if path.is_file() {
    if path.file_name().and_then(|name| name.to_str()) == Some("raw-clock-report.json") {
      reports.push(load_report(path)?);
    }
    return Ok(());
  }

  if !path.is_dir() {
    return Ok(());
  }

  for entry in std::fs::read_dir(path)? {
    collect_reports(&entry?.path(), reports)?;
  }
  Ok(())
}

fn load_report(path: &Path) -> std::io::Result<RawClockReport> {
  let contents = std::fs::read_to_string(path)?;
  let value: Value = serde_json::from_str(&contents).map_err(std::io::Error::other)?;

  Ok(RawClockReport {
    path: path.to_path_buf(),
    environment_name: value["environment_name"].as_str().unwrap_or("unknown").to_string(),
    compile_key: value["compile_identity"]["key"].as_str().unwrap_or("unknown").to_string(),
    runtime: environment_summary(&value),
    clocks: clock_rows(&value),
  })
}

fn environment_summary(value: &Value) -> String {
  let runtime = &value["runtime_identity"];
  let mut parts = Vec::new();
  push_json_string(&mut parts, "os", &runtime["os_release"]);
  push_json_string(&mut parts, "kernel", &runtime["kernel"]);
  push_json_string(&mut parts, "container", &runtime["container"]);
  push_json_string(&mut parts, "virtualization", &runtime["virtualization"]);
  push_json_string(&mut parts, "rosetta", &runtime["macos_rosetta"]);
  push_json_string(&mut parts, "windows", &runtime["windows_emulation"]);
  parts.join(" | ")
}

fn push_json_string(parts: &mut Vec<String>, label: &str, value: &Value) {
  if value.is_null() {
    return;
  }
  let value = match value {
    Value::String(value) => value.clone(),
    value => value.to_string(),
  };
  if !value.is_empty() {
    parts.push(format!("{label}={}", value.replace('\n', " ")));
  }
}

fn clock_rows(value: &Value) -> Vec<RawClockRow> {
  value["clocks"]
    .as_array()
    .into_iter()
    .flatten()
    .map(|clock| RawClockRow {
      source: clock["source"].as_str().unwrap_or("unknown").to_string(),
      name: clock["name"].as_str().unwrap_or("unknown").to_string(),
      operation: clock["operation"].as_str().unwrap_or("unknown").to_string(),
      ns_op: clock["ns_op"].as_f64().unwrap_or(0.0),
      confidence_high: clock["confidence_high"].as_bool().unwrap_or(false),
    })
    .collect()
}

fn group_reports(reports: Vec<RawClockReport>) -> Vec<RawClockGroup> {
  let mut grouped: BTreeMap<String, Vec<RawClockReport>> = BTreeMap::new();
  for report in reports {
    grouped.entry(report.compile_key.clone()).or_default().push(report);
  }

  grouped
    .into_iter()
    .map(|(compile_key, mut reports)| {
      reports.sort_by(|left, right| left.environment_name.cmp(&right.environment_name));
      RawClockGroup { compile_key, reports }
    })
    .collect()
}

fn render_markdown(groups: &[RawClockGroup]) -> String {
  let mut out = String::new();
  out.push_str("# hotclock raw clock benchmark matrix\n\n");

  for group in groups {
    writeln!(out, "## `{}`\n", group.compile_key).unwrap();
    out.push_str("| Environment | Clock | Source | Operation | ns/op |\n");
    out.push_str("|---|---|---|---|---:|\n");
    for report in &group.reports {
      let mut clocks = report.clocks.clone();
      clocks.sort_by(|left, right| {
        (left.source.as_str(), left.name.as_str(), left.operation.as_str()).cmp(&(
          right.source.as_str(),
          right.name.as_str(),
          right.operation.as_str(),
        ))
      });
      for clock in clocks {
        writeln!(
          out,
          "| `{}` | `{}` | {} | {} | {:.3} |",
          report.environment_name, clock.name, clock.source, clock.operation, clock.ns_op
        )
        .unwrap();
      }
    }
    out.push('\n');
  }

  out
}

fn render_json(groups: &[RawClockGroup]) -> Value {
  json!({
    "schema_version": 1,
    "groups": groups.iter().map(|group| {
      json!({
        "compile_key": group.compile_key,
        "reports": group.reports.iter().map(|report| {
          json!({
            "path": report.path,
            "environment_name": report.environment_name,
            "runtime": report.runtime,
            "clocks": report.clocks.iter().map(|clock| {
              json!({
                "source": clock.source,
                "name": clock.name,
                "operation": clock.operation,
                "ns_op": clock.ns_op,
                "confidence_high": clock.confidence_high,
              })
            }).collect::<Vec<_>>(),
          })
        }).collect::<Vec<_>>(),
      })
    }).collect::<Vec<_>>(),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn groups_reports_by_compile_key() {
    let groups = group_reports(vec![
      report("linux-b", "x86_64-unknown-linux-gnu-64-little"),
      report("linux-a", "x86_64-unknown-linux-gnu-64-little"),
      report("macos", "x86_64-apple-macos-unknown-64-little"),
    ]);

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].compile_key, "x86_64-apple-macos-unknown-64-little");
    assert_eq!(groups[1].reports[0].environment_name, "linux-a");
    assert_eq!(groups[1].reports[1].environment_name, "linux-b");
  }

  fn report(environment_name: &str, compile_key: &str) -> RawClockReport {
    RawClockReport {
      path: PathBuf::from(format!("{environment_name}/raw-clock-report.json")),
      environment_name: environment_name.to_string(),
      compile_key: compile_key.to_string(),
      runtime: String::new(),
      clocks: vec![RawClockRow {
        source: "hotclock".to_string(),
        name: "x86_64-rdtsc".to_string(),
        operation: "read".to_string(),
        ns_op: 10.0,
        confidence_high: true,
      }],
    }
  }
}
