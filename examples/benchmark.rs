use tach::Instant;

const ITERATIONS: usize = 10_000_000;

fn main() {
  println!("tach Overhead Benchmark");
  println!("=================================\n");

  println!("Implementation: {}", Instant::implementation());
  let freq = Instant::frequency();
  println!("Frequency: {:.2} MHz\n", mhz(freq));

  // Warm up
  for _ in 0..1000 {
    let _ = Instant::now();
  }

  let start = std::time::Instant::now();
  for _ in 0..ITERATIONS {
    let _ = std::hint::black_box(Instant::now());
  }
  let elapsed = start.elapsed();

  let ns_per_call = nanos_per_call(elapsed.as_nanos(), ITERATIONS);
  println!("Instant::now() overhead:");
  println!("  {ITERATIONS} iterations in {elapsed:?}");
  println!("  {ns_per_call:.2} ns per call");

  // Measure delta between consecutive calls
  let mut deltas = Vec::with_capacity(1000);
  for _ in 0..1000 {
    let a = Instant::now();
    let b = Instant::now();
    deltas.push(b.ticks_since(a).as_raw());
  }

  deltas.sort_unstable();
  let median = deltas[deltas.len() / 2];
  let min = deltas[0];
  let max = deltas[deltas.len() - 1];
  let p99 = deltas[(deltas.len() * 99) / 100];

  println!("\nConsecutive call delta (ticks):");
  println!("  min: {min}");
  println!("  median: {median}");
  println!("  p99: {p99}");
  println!("  max: {max}");

  println!("\nConsecutive call delta (nanoseconds):");
  println!("  min: {:.1} ns", ticks_to_ns(min, freq));
  println!("  median: {:.1} ns", ticks_to_ns(median, freq));
}

#[allow(clippy::cast_precision_loss)]
fn mhz(freq: u64) -> f64 {
  freq as f64 / 1e6
}

#[allow(clippy::cast_precision_loss)]
fn nanos_per_call(total_nanos: u128, iterations: usize) -> f64 {
  total_nanos as f64 / iterations as f64
}

#[allow(clippy::cast_precision_loss)]
fn ticks_to_ns(ticks: u64, freq: u64) -> f64 {
  ticks as f64 * 1e9 / freq as f64
}
