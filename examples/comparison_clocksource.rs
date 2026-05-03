mod comparison_bench_common;

use std::hint::black_box;

fn main() -> std::io::Result<()> {
  comparison_bench_common::write_report("clocksource", vec![
    comparison_bench_common::measure("clocksource::precise::Instant::now()", "now", || {
      black_box(clocksource::precise::Instant::now());
    }),
    comparison_bench_common::measure(
      "clocksource::precise::Instant (now + elapsed)",
      "elapsed",
      || {
        let start = clocksource::precise::Instant::now();
        black_box(start.elapsed());
      },
    ),
  ])
}
