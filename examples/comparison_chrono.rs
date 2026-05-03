mod comparison_bench_common;

use std::hint::black_box;

fn main() -> std::io::Result<()> {
  comparison_bench_common::write_report("chrono", vec![comparison_bench_common::measure(
    "chrono::Utc::now()",
    "now",
    || {
      black_box(chrono::Utc::now());
    },
  )])
}
