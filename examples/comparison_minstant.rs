mod comparison_bench_common;

use std::hint::black_box;

fn main() -> std::io::Result<()> {
  comparison_bench_common::write_report("minstant", vec![
    comparison_bench_common::measure("minstant::Instant::now()", "now", || {
      black_box(minstant::Instant::now());
    }),
    comparison_bench_common::measure("minstant::Instant (now + elapsed)", "elapsed", || {
      let start = minstant::Instant::now();
      black_box(start.elapsed());
    }),
  ])
}
