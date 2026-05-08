use tach::Instant;

fn main() {
  println!("tach v{}", env!("CARGO_PKG_VERSION"));
  println!("==================\n");

  println!("Implementation: {}", Instant::implementation());
  let freq = Instant::frequency();
  println!("Frequency: {freq} Hz ({:.2} MHz)\n", mhz(freq));

  let start = Instant::now();
  let end = Instant::now();
  println!("Two consecutive Instant::now() calls:");
  println!("  Start: {}", start.as_raw());
  println!("  End:   {}", end.as_raw());
  println!("  Delta: {}\n", end.ticks_since(start));

  let start = Instant::now();
  let mut sum = 0u64;
  for i in 0..1_000_000 {
    sum = std::hint::black_box(sum.wrapping_add(i));
  }
  let elapsed = start.elapsed();
  let elapsed_ticks = start.elapsed_ticks();

  println!("Instant measuring 1M additions:");
  println!("  Time: {elapsed:?}");
  println!("  Raw ticks: {elapsed_ticks}");
  println!("  (sum = {sum} to prevent optimization)\n");

  println!("Time conversions:");
  println!("  as_nanos:  {}", elapsed_ticks.as_nanos());
  println!("  as_micros: {}", elapsed_ticks.as_micros());
  println!("  as_millis: {}", elapsed_ticks.as_millis());
  println!("  as_secs:   {}", elapsed_ticks.as_secs_f64());
}

#[allow(clippy::cast_precision_loss)]
fn mhz(freq: u64) -> f64 {
  freq as f64 / 1e6
}
