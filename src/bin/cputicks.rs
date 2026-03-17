use cputicks::Ticks;

fn main() {
  let freq = Ticks::frequency();

  println!("cputicks v{}", env!("CARGO_PKG_VERSION"));
  println!();
  println!("Implementation: {}", Ticks::implementation());
  println!("Frequency:      {} Hz ({:.2} MHz)", freq, freq as f64 / 1e6);
  println!("Overhead:       {:.0} ps per call", measure_overhead());
  println!("Resolution:     {}", format_resolution(freq));
}

fn measure_overhead() -> f64 {
  const N: usize = 1_000_000;
  let start = std::time::Instant::now();
  for _ in 0..N {
    std::hint::black_box(Ticks::now().as_raw());
  }
  start.elapsed().as_nanos() as f64 / N as f64 * 1000.0
}

fn format_resolution(freq: u64) -> String {
  let mut deltas: Vec<_> = (0..1000)
    .map(|_| {
      let a = Ticks::now();
      let b = Ticks::now();
      (b - a).as_raw()
    })
    .collect();
  deltas.sort();
  let median = deltas[500];
  format!("{} ticks ({:.0} ps)", median, median as f64 * 1e12 / freq as f64)
}
