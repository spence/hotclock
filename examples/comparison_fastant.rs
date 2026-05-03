mod comparison_bench_common;

use std::hint::black_box;

fn main() -> std::io::Result<()> {
  comparison_bench_common::write_report("fastant", vec![
    comparison_bench_common::measure("fastant::Instant::now()", "now", || {
      black_box(fastant::Instant::now());
    }),
    comparison_bench_common::measure("fastant::Instant (now + elapsed)", "elapsed", || {
      let start = fastant::Instant::now();
      black_box(start.elapsed());
    }),
  ])
}
