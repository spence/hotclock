mod comparison_bench_common;

use std::hint::black_box;

use hotclock::Instant;

fn main() -> std::io::Result<()> {
  comparison_bench_common::write_report("hotclock", vec![
    comparison_bench_common::measure("hotclock::Instant::now()", "now", || {
      let _ = black_box(Instant::now());
    }),
    comparison_bench_common::measure("hotclock::Instant (now + elapsed_ticks)", "elapsed", || {
      let start = Instant::now();
      let _ = black_box(start.elapsed_ticks());
    }),
  ])
}
