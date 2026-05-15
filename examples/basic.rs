use tach::Instant;

fn main() {
  // Prime the lazy frequency calibration so the first measurement
  // doesn't include the one-time ~50ms calibration cost.
  let _ = Instant::now().elapsed();

  let start = Instant::now();
  let mut sum = 0u64;
  for i in 0..1_000_000 {
    sum = std::hint::black_box(sum.wrapping_add(i));
  }
  let elapsed = start.elapsed();

  println!("1M additions (sum = {sum}):");
  println!("  elapsed = {elapsed:?}");
}
