mod comparison_bench_common;

use std::hint::black_box;

fn main() -> std::io::Result<()> {
  let clock = clock::MonotonicClock::default();
  comparison_bench_common::write_report("clock", vec![comparison_bench_common::measure(
    "clock::MonotonicClock::now()",
    "now",
    || {
      black_box(clock.now());
    },
  )])
}
