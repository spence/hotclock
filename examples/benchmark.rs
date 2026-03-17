use cputicks::Ticks;

fn main() {
  println!("Cycle Counter Overhead Benchmark");
  println!("=================================\n");

  println!("Implementation: {}", Ticks::implementation());
  let freq = Ticks::frequency();
  println!("Frequency: {:.2} MHz\n", freq as f64 / 1e6);

  // Warm up
  for _ in 0..1000 {
    let _ = Ticks::now();
  }

  // Measure overhead of Cycles::now()
  const ITERATIONS: usize = 10_000_000;

  let start = std::time::Instant::now();
  for _ in 0..ITERATIONS {
    let _ = std::hint::black_box(Ticks::now());
  }
  let elapsed = start.elapsed();

  let ns_per_call = elapsed.as_nanos() as f64 / ITERATIONS as f64;
  println!("Cycles::now() overhead:");
  println!("  {} iterations in {:?}", ITERATIONS, elapsed);
  println!("  {:.2} ns per call", ns_per_call);

  // Measure delta between consecutive calls
  let mut deltas = Vec::with_capacity(1000);
  for _ in 0..1000 {
    let a = Ticks::now();
    let b = Ticks::now();
    deltas.push((b - a).as_raw());
  }

  deltas.sort();
  let median = deltas[deltas.len() / 2];
  let min = deltas[0];
  let max = deltas[deltas.len() - 1];
  let p99 = deltas[(deltas.len() * 99) / 100];

  println!("\nConsecutive call delta (cycles):");
  println!("  min: {}", min);
  println!("  median: {}", median);
  println!("  p99: {}", p99);
  println!("  max: {}", max);

  println!("\nConsecutive call delta (nanoseconds):");
  println!("  min: {:.1} ns", min as f64 * 1e9 / freq as f64);
  println!("  median: {:.1} ns", median as f64 * 1e9 / freq as f64);
}
