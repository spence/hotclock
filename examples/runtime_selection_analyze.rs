#![cfg(feature = "bench-internals")]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

#[derive(Clone, Debug)]
struct ReportSummary {
  path: PathBuf,
  compile_key: String,
  proof_eligible: bool,
  selection_matches_winner: bool,
  fastest_candidate: Option<String>,
  selected_candidate: Option<String>,
  selected_candidate_valid: bool,
  candidate_benchmarks: Vec<CandidateBenchmark>,
  environment: String,
}

#[derive(Clone, Debug)]
struct CandidateBenchmark {
  name: String,
  production_candidate: bool,
  valid: bool,
  selected: bool,
  fastest_valid: bool,
  mean_ns: Option<f64>,
  median_ns: Option<f64>,
  ci95_low_ns: Option<f64>,
  ci95_high_ns: Option<f64>,
  best_ns: Option<f64>,
  worst_ns: Option<f64>,
  samples: Option<u64>,
}

#[derive(Clone, Debug)]
struct ProofGroup {
  compile_key: String,
  reports: Vec<ReportSummary>,
  winners: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct ProofResult {
  proven: bool,
  groups: Vec<ProofGroup>,
}

fn main() -> std::io::Result<()> {
  let roots = roots_from_args();
  let reports = load_reports(&roots)?;
  let result = analyze_reports(reports);
  let markdown = render_markdown(&result);
  let json = render_json(&result);

  println!("{markdown}");

  if let Some(out_dir) = std::env::var_os("HOTCLOCK_PROOF_DIR") {
    let out_dir = PathBuf::from(out_dir);
    std::fs::create_dir_all(&out_dir)?;
    std::fs::write(out_dir.join("runtime-selection-proof.md"), &markdown)?;
    std::fs::write(out_dir.join("runtime-selection-proof.json"), json.to_string())?;
  }

  Ok(())
}

fn roots_from_args() -> Vec<PathBuf> {
  let roots: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
  if roots.is_empty() { vec![PathBuf::from("target")] } else { roots }
}

fn load_reports(roots: &[PathBuf]) -> std::io::Result<Vec<ReportSummary>> {
  let mut reports = Vec::new();
  for root in roots {
    collect_reports(root, &mut reports)?;
  }
  Ok(reports)
}

fn collect_reports(path: &Path, reports: &mut Vec<ReportSummary>) -> std::io::Result<()> {
  if path.is_file() {
    if path.file_name().and_then(|name| name.to_str()) == Some("runtime-selection-report.json") {
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

fn load_report(path: &Path) -> std::io::Result<ReportSummary> {
  let contents = std::fs::read_to_string(path)?;
  let value: Value = serde_json::from_str(&contents).map_err(std::io::Error::other)?;
  let proof_eligible = value["proof_eligible"].as_bool().unwrap_or(false);
  let fastest_candidate = fastest_candidate_name(&value);
  let (selected_candidate, selected_candidate_valid) = selected_candidate_name(&value);
  let selection_matches_winner = proof_eligible
    && selected_candidate_valid
    && fastest_candidate.is_some()
    && fastest_candidate == selected_candidate;

  Ok(ReportSummary {
    path: path.to_path_buf(),
    compile_key: value["compile_identity"]["key"].as_str().unwrap_or("unknown").to_string(),
    proof_eligible,
    selection_matches_winner,
    fastest_candidate,
    selected_candidate,
    selected_candidate_valid,
    candidate_benchmarks: candidate_benchmarks(&value),
    environment: environment_summary(&value),
  })
}

fn fastest_candidate_name(value: &Value) -> Option<String> {
  value["candidates"].as_array()?.iter().find_map(|candidate| {
    let marked = candidate["fastest_valid"].as_bool().unwrap_or(false);
    let valid = candidate["valid"].as_bool().unwrap_or(false);
    let production = candidate["production_candidate"].as_bool().unwrap_or(false);
    if marked && valid && production {
      candidate["name"].as_str().map(ToString::to_string)
    } else {
      None
    }
  })
}

fn selected_candidate_name(value: &Value) -> (Option<String>, bool) {
  let Some(candidate) = value["candidates"].as_array().and_then(|candidates| {
    candidates.iter().find(|candidate| {
      candidate["selected"].as_bool().unwrap_or(false)
        && candidate["production_candidate"].as_bool().unwrap_or(false)
    })
  }) else {
    return (None, false);
  };

  (
    candidate["name"].as_str().map(ToString::to_string),
    candidate["valid"].as_bool().unwrap_or(false),
  )
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

fn candidate_benchmarks(value: &Value) -> Vec<CandidateBenchmark> {
  value["candidates"]
    .as_array()
    .into_iter()
    .flatten()
    .map(|candidate| CandidateBenchmark {
      name: candidate["name"].as_str().unwrap_or("unknown").to_string(),
      production_candidate: candidate["production_candidate"].as_bool().unwrap_or(false),
      valid: candidate["valid"].as_bool().unwrap_or(false),
      selected: candidate["selected"].as_bool().unwrap_or(false),
      fastest_valid: candidate["fastest_valid"].as_bool().unwrap_or(false),
      mean_ns: candidate["mean_ns"].as_f64(),
      median_ns: candidate["median_ns"].as_f64(),
      ci95_low_ns: candidate["ci95_low_ns"].as_f64(),
      ci95_high_ns: candidate["ci95_high_ns"].as_f64(),
      best_ns: candidate["best_ns"].as_f64(),
      worst_ns: candidate["worst_ns"].as_f64(),
      samples: candidate["samples"].as_u64(),
    })
    .collect()
}

fn analyze_reports(reports: Vec<ReportSummary>) -> ProofResult {
  let mut grouped: BTreeMap<String, Vec<ReportSummary>> = BTreeMap::new();
  for report in reports {
    grouped.entry(report.compile_key.clone()).or_default().push(report);
  }

  let groups: Vec<_> = grouped
    .into_iter()
    .map(|(compile_key, reports)| {
      let winners = reports
        .iter()
        .filter(|report| report.proof_eligible)
        .filter_map(|report| report.fastest_candidate.clone())
        .collect();
      ProofGroup { compile_key, reports, winners }
    })
    .collect();

  let proven = groups.iter().any(|group| group.winners.len() > 1);
  ProofResult { proven, groups }
}

fn render_markdown(result: &ProofResult) -> String {
  let mut out = String::new();
  writeln!(
    out,
    "# hotclock runtime selection proof\n\nStatus: `{}`\n",
    if result.proven { "PROVED" } else { "NOT PROVEN" }
  )
  .unwrap();

  for group in &result.groups {
    writeln!(out, "## `{}`\n", group.compile_key).unwrap();
    writeln!(
      out,
      "Winners: `{}`\n",
      group.winners.iter().cloned().collect::<Vec<_>>().join("`, `")
    )
    .unwrap();
    out.push_str("| Report | Eligible | Fastest | Selected | Selection match | Significant | Clock benchmarks | Environment |\n");
    out.push_str("|---|---:|---|---|---:|---|---|---|\n");
    for report in &group.reports {
      writeln!(
        out,
        "| `{}` | {} | `{}` | `{}` | {} | {} | {} | `{}` |",
        report.path.display(),
        report.proof_eligible,
        report.fastest_candidate.as_deref().unwrap_or("n/a"),
        selected_label(report),
        report.selection_matches_winner,
        significance_label(report),
        benchmark_label(report),
        report.environment.replace('|', "/")
      )
      .unwrap();
    }
    out.push('\n');
  }

  out
}

fn significance_label(report: &ReportSummary) -> &'static str {
  let mut valid = report
    .candidate_benchmarks
    .iter()
    .filter(|candidate| candidate.production_candidate && candidate.valid)
    .filter(|candidate| candidate.ci95_low_ns.is_some() && candidate.ci95_high_ns.is_some())
    .collect::<Vec<_>>();

  valid.sort_by(|left, right| {
    left
      .median_ns
      .unwrap_or(f64::INFINITY)
      .total_cmp(&right.median_ns.unwrap_or(f64::INFINITY))
  });

  match valid.as_slice() {
    [] => "`n/a`",
    [_] => "`validation`",
    [fastest, next, ..] => {
      if fastest.ci95_high_ns.unwrap_or(f64::INFINITY)
        < next.ci95_low_ns.unwrap_or(f64::NEG_INFINITY)
      {
        "`✅`"
      } else {
        "`overlap`"
      }
    }
  }
}

fn benchmark_label(report: &ReportSummary) -> String {
  let labels = report
    .candidate_benchmarks
    .iter()
    .filter(|candidate| candidate.production_candidate)
    .map(candidate_benchmark_label)
    .collect::<Vec<_>>();

  if labels.is_empty() { "`n/a`".to_string() } else { labels.join("<br>") }
}

fn candidate_benchmark_label(candidate: &CandidateBenchmark) -> String {
  let role = candidate_role(candidate);
  if !candidate.valid {
    return format!("`{}`{}: invalid", candidate.name, role);
  }

  let median = candidate.median_ns.or(candidate.mean_ns);
  let Some(median) = median else {
    return format!("`{}`{}: valid, no benchmark", candidate.name, role);
  };

  let interval = match (candidate.ci95_low_ns, candidate.ci95_high_ns) {
    (Some(low), Some(high)) => format!("95% CI {:.3}..{:.3}", low, high),
    _ => match (candidate.best_ns, candidate.worst_ns) {
      (Some(best), Some(worst)) => format!("range {:.3}..{:.3}", best, worst),
      _ => "no interval".to_string(),
    },
  };
  let mean = candidate.mean_ns.map_or(String::new(), |mean| format!(", mean {:.3}", mean));
  let samples = candidate.samples.map_or(String::new(), |samples| format!(", n={samples}"));

  format!("`{}`{}: median {:.3} ns{} ({}{})", candidate.name, role, median, mean, interval, samples)
}

fn candidate_role(candidate: &CandidateBenchmark) -> &'static str {
  match (candidate.selected, candidate.fastest_valid) {
    (true, true) => " selected fastest",
    (true, false) => " selected",
    (false, true) => " fastest",
    (false, false) => "",
  }
}

fn selected_label(report: &ReportSummary) -> String {
  match (&report.selected_candidate, report.selected_candidate_valid) {
    (Some(name), true) => name.clone(),
    (Some(name), false) => format!("{name} (invalid)"),
    (None, _) => "n/a".to_string(),
  }
}

fn render_json(result: &ProofResult) -> Value {
  json!({
    "schema_version": 1,
    "status": if result.proven { "PROVED" } else { "NOT_PROVEN" },
    "groups": result.groups.iter().map(|group| {
      json!({
        "compile_key": group.compile_key,
        "winners": group.winners.iter().cloned().collect::<Vec<_>>(),
        "reports": group.reports.iter().map(|report| {
          json!({
            "path": report.path,
            "proof_eligible": report.proof_eligible,
            "selection_matches_winner": report.selection_matches_winner,
            "fastest_candidate": report.fastest_candidate,
            "selected_candidate": report.selected_candidate,
            "selected_candidate_valid": report.selected_candidate_valid,
            "benchmark_significance": significance_label(report).trim_matches('`'),
            "candidate_benchmarks": report.candidate_benchmarks.iter().map(|candidate| {
              json!({
                "name": candidate.name,
                "production_candidate": candidate.production_candidate,
                "valid": candidate.valid,
                "selected": candidate.selected,
                "fastest_valid": candidate.fastest_valid,
                "mean_ns": candidate.mean_ns,
                "median_ns": candidate.median_ns,
                "ci95_low_ns": candidate.ci95_low_ns,
                "ci95_high_ns": candidate.ci95_high_ns,
                "best_ns": candidate.best_ns,
                "worst_ns": candidate.worst_ns,
                "samples": candidate.samples,
              })
            }).collect::<Vec<_>>(),
            "environment": report.environment,
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
  fn proves_when_same_compile_key_has_different_winners() {
    let result = analyze_reports(vec![
      report("x86_64-unknown-linux-gnu-64-little", "x86_64-rdtsc", "x86_64-rdtsc", true),
      report("x86_64-unknown-linux-gnu-64-little", "unix-monotonic", "unix-monotonic", true),
    ]);

    assert!(result.proven);
  }

  #[test]
  fn does_not_prove_when_winner_is_the_same() {
    let result = analyze_reports(vec![
      report("x86_64-unknown-linux-gnu-64-little", "x86_64-rdtsc", "x86_64-rdtsc", true),
      report("x86_64-unknown-linux-gnu-64-little", "x86_64-rdtsc", "x86_64-rdtsc", true),
    ]);

    assert!(!result.proven);
  }

  #[test]
  fn does_not_prove_when_compile_keys_differ() {
    let result = analyze_reports(vec![
      report("x86_64-unknown-linux-gnu-64-little", "x86_64-rdtsc", "x86_64-rdtsc", true),
      report("aarch64-unknown-linux-gnu-64-little", "unix-monotonic", "unix-monotonic", true),
    ]);

    assert!(!result.proven);
  }

  #[test]
  fn proves_measured_divergence_when_selection_mismatches_winner() {
    let result = analyze_reports(vec![
      report("x86_64-pc-windows-msvc-64-little", "std-instant", "x86_64-rdtsc", false),
      report("x86_64-pc-windows-msvc-64-little", "x86_64-rdtsc", "x86_64-rdtsc", true),
    ]);

    assert!(result.proven);
    assert!(!result.groups[0].reports[0].selection_matches_winner);
    assert!(result.groups[0].reports[1].selection_matches_winner);
  }

  fn report(
    compile_key: &str,
    fastest_candidate: &str,
    selected_candidate: &str,
    selected_candidate_valid: bool,
  ) -> ReportSummary {
    ReportSummary {
      path: PathBuf::from(format!("{compile_key}/{fastest_candidate}.json")),
      compile_key: compile_key.to_string(),
      proof_eligible: true,
      selection_matches_winner: fastest_candidate == selected_candidate && selected_candidate_valid,
      fastest_candidate: Some(fastest_candidate.to_string()),
      selected_candidate: Some(selected_candidate.to_string()),
      selected_candidate_valid,
      candidate_benchmarks: vec![
        CandidateBenchmark {
          name: fastest_candidate.to_string(),
          production_candidate: true,
          valid: true,
          selected: fastest_candidate == selected_candidate,
          fastest_valid: true,
          mean_ns: Some(10.0),
          median_ns: Some(10.0),
          ci95_low_ns: Some(9.0),
          ci95_high_ns: Some(11.0),
          best_ns: Some(9.0),
          worst_ns: Some(11.0),
          samples: Some(31),
        },
        CandidateBenchmark {
          name: "fallback".to_string(),
          production_candidate: true,
          valid: true,
          selected: selected_candidate == "fallback",
          fastest_valid: false,
          mean_ns: Some(20.0),
          median_ns: Some(20.0),
          ci95_low_ns: Some(19.0),
          ci95_high_ns: Some(21.0),
          best_ns: Some(19.0),
          worst_ns: Some(21.0),
          samples: Some(31),
        },
      ],
      environment: "fixture".to_string(),
    }
  }
}
