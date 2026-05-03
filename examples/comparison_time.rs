mod comparison_bench_common;

use std::hint::black_box;

fn main() -> std::io::Result<()> {
  comparison_bench_common::write_report("time", vec![comparison_bench_common::measure(
    "time::OffsetDateTime::now_utc()",
    "now",
    || {
      black_box(time::OffsetDateTime::now_utc());
    },
  )])
}
