mod comparison_bench_common;

use std::hint::black_box;

fn main() -> std::io::Result<()> {
  comparison_bench_common::write_report("quanta", vec![
    comparison_bench_common::measure("quanta::Instant::now()", "now", || {
      black_box(quanta::Instant::now());
    }),
    comparison_bench_common::measure("quanta::Instant (now + elapsed)", "elapsed", || {
      let start = quanta::Instant::now();
      black_box(start.elapsed());
    }),
  ])
}
