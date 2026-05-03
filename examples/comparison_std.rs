mod comparison_bench_common;

use std::hint::black_box;
use std::time::Instant;

fn main() -> std::io::Result<()> {
  comparison_bench_common::write_report("std", vec![
    comparison_bench_common::measure("std::time::Instant::now()", "now", || {
      black_box(Instant::now());
    }),
    comparison_bench_common::measure("std::time::Instant (now + elapsed)", "elapsed", || {
      let start = Instant::now();
      black_box(start.elapsed());
    }),
  ])
}
