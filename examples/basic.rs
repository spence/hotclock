use cputicks::Ticks;

fn main() {
  println!("cputicks v0.2.0");
  println!("==================\n");

  println!("Implementation: {}", Ticks::implementation());
  let freq = Ticks::frequency();
  println!("Frequency: {} Hz ({:.2} MHz)\n", freq, freq as f64 / 1e6);

  let start = Ticks::now();
  let end = Ticks::now();
  println!("Two consecutive Ticks::now() calls:");
  println!("  Start: {}", start.as_raw());
  println!("  End:   {}", end.as_raw());
  println!("  Delta: {}\n", end - start);

  let start = Ticks::now();
  let mut sum = 0u64;
  for i in 0..1_000_000 {
    sum = std::hint::black_box(sum.wrapping_add(i));
  }
  let elapsed = start.elapsed();

  println!("Ticks measuring 1M additions:");
  println!("  Elapsed: {}", elapsed);
  println!("  Time: {:?}", elapsed.as_duration());
  println!("  (sum = {} to prevent optimization)\n", sum);

  println!("Time conversions:");
  println!("  as_nanos:  {}", elapsed.as_nanos());
  println!("  as_micros: {}", elapsed.as_micros());
  println!("  as_millis: {}", elapsed.as_millis());
  println!("  as_secs:   {}", elapsed.as_secs_f64());
}
