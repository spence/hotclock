#![allow(clippy::cast_precision_loss)]

use std::hint::black_box;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Instant as StdInstant;

use hotclock::{Cycles, Instant};

const DEFAULT_WARMUP_ITERS: usize = 10_000;
const DEFAULT_MEASURE_ITERS: usize = 100_000;
const DEFAULT_SAMPLES: usize = 31;

#[derive(Clone, Copy)]
struct Stats {
  ns_op: f64,
}

struct Row {
  target: String,
  environment: String,
  expected_instant: String,
  expected_cycles: String,
  selected_instant: &'static str,
  selected_cycles: &'static str,
  hotclock_instant: Stats,
  hotclock_cycles: Stats,
  quanta: Stats,
  minstant: Stats,
  fastant: Stats,
  std_instant: Stats,
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
  let expected_instant =
    std::env::var("HOTCLOCK_EXPECT_INSTANT").unwrap_or_else(|_| "unset".into());
  let expected_cycles = std::env::var("HOTCLOCK_EXPECT_CYCLES").unwrap_or_else(|_| "unset".into());
  let expected_set = expected_instant != "unset" && expected_cycles != "unset";

  let selected_instant = Instant::implementation();
  let selected_cycles = Cycles::implementation();

  let hotclock_instant = measure(|| {
    let _ = black_box(Instant::now());
  });
  let hotclock_cycles = measure(|| {
    let _ = black_box(Cycles::now());
  });
  let quanta = measure(|| {
    let _ = black_box(quanta::Instant::now());
  });
  let minstant = measure(|| {
    let _ = black_box(minstant::Instant::now());
  });
  let fastant = measure(|| {
    let _ = black_box(fastant::Instant::now());
  });
  let std_instant = measure(|| {
    let _ = black_box(StdInstant::now());
  });
  let fastest_instant_api = hotclock_instant.ns_op <= quanta.ns_op
    && hotclock_instant.ns_op <= minstant.ns_op
    && hotclock_instant.ns_op <= fastant.ns_op
    && hotclock_instant.ns_op <= std_instant.ns_op;

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
    hotclock_instant,
    hotclock_cycles,
    quanta,
    minstant,
    fastant,
    std_instant,
    fastest_instant_api,
  };

  let ok = std::env::var("HOTCLOCK_ENFORCE_EXPECTED").as_deref() != Ok("1")
    || (row.expected_set && row.matches_expected);
  (render_row(&row), ok)
}

fn measure<F>(mut f: F) -> Stats
where
  F: FnMut(),
{
  let warmup_iters = env_usize("HOTCLOCK_VALIDATION_WARMUP_ITERS", DEFAULT_WARMUP_ITERS);
  let measure_iters = env_usize("HOTCLOCK_VALIDATION_MEASURE_ITERS", DEFAULT_MEASURE_ITERS);
  let samples = env_usize("HOTCLOCK_VALIDATION_SAMPLES", DEFAULT_SAMPLES);

  for _ in 0..warmup_iters {
    f();
  }

  let mut timings = Vec::with_capacity(samples);
  for _ in 0..samples {
    let start = StdInstant::now();
    for _ in 0..measure_iters {
      f();
    }
    timings.push(start.elapsed().as_nanos() as f64 / measure_iters as f64);
  }

  timings.sort_by(f64::total_cmp);
  Stats { ns_op: timings[timings.len() / 2] }
}

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
  push_field(&mut out, "hotclock-instant-bench", &format_ns(row.hotclock_instant));
  push_field(&mut out, "hotclock-cycles-bench", &format_ns(row.hotclock_cycles));
  push_field(&mut out, "quanta-bench", &format_ns(row.quanta));
  push_field(&mut out, "minstant-bench", &format_ns(row.minstant));
  push_field(&mut out, "fastant-bench", &format_ns(row.fastant));
  push_field(&mut out, "std-instant-bench", &format_ns(row.std_instant));
  push_field(&mut out, "fastest-instant-api", yes_no(row.fastest_instant_api));
  push_field(&mut out, "matches-expected", expected_status(row));
  out.push_str("└────────────────────────────────┴────────────────────────────────────────┘\n");
  out
}

fn push_field(out: &mut String, field: &str, value: &str) {
  out.push_str(&format!("│ {field:<30} │ {value:<38} │\n"));
}

fn format_ns(stats: Stats) -> String {
  format!("{:.3} ns/op", stats.ns_op)
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

fn yes_no(value: bool) -> &'static str {
  if value { "yes" } else { "no" }
}

fn env_usize(name: &str, default: usize) -> usize {
  std::env::var(name).ok().and_then(|value| value.parse().ok()).unwrap_or(default)
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
  if let Ok(name) = std::env::var("HOTCLOCK_ENV_NAME") {
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
        "{{\"errorMessage\":\"{}\",\"errorType\":\"HotclockSelectionMismatch\"}}",
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
