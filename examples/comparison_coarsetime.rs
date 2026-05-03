mod comparison_bench_common;

use std::hint::black_box;

fn main() -> std::io::Result<()> {
  comparison_bench_common::write_report("coarsetime", vec![
    comparison_bench_common::measure("coarsetime::Instant::now()", "now", || {
      black_box(coarsetime::Instant::now());
    }),
    comparison_bench_common::measure("coarsetime::Instant (now + elapsed)", "elapsed", || {
      let start = coarsetime::Instant::now();
      black_box(start.elapsed());
    }),
  ])
}
