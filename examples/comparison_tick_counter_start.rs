mod comparison_bench_common;

use std::hint::black_box;

fn main() -> std::io::Result<()> {
  comparison_bench_common::write_report("tick_counter", vec![comparison_bench_common::measure(
    "tick_counter::start()",
    "now",
    || {
      black_box(tick_counter::start());
    },
  )])
}
