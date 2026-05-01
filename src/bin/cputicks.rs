use cputicks::Instant;

fn main() {
  let freq = Instant::frequency();

  println!("cputicks v{}", env!("CARGO_PKG_VERSION"));
  println!();
  println!("Implementation: {}", Instant::implementation());
  println!("Frequency:      {freq} Hz ({:.2} MHz)", mhz(freq));
  println!("Overhead:       {:.0} ps per call", measure_overhead());
  println!("Resolution:     {}", format_resolution(freq));
}

fn measure_overhead() -> f64 {
  const N: usize = 1_000_000;
  let start = std::time::Instant::now();
  for _ in 0..N {
    std::hint::black_box(Instant::now().as_raw());
  }
  nanos_per_call(start.elapsed().as_nanos(), N) * 1000.0
}

fn format_resolution(freq: u64) -> String {
  let mut deltas = Vec::with_capacity(1000);

  for _ in 0..1000 {
    let start = Instant::now();
    for _ in 0..10_000 {
      let delta = Instant::now().ticks_since(start).as_raw();
      if delta > 0 {
        deltas.push(delta);
        break;
      }
      std::hint::spin_loop();
    }
  }

  if deltas.is_empty() {
    return "unobserved".to_string();
  }

  deltas.sort_unstable();
  let median = deltas[deltas.len() / 2];
  format!("{median} ticks ({:.0} ps)", ticks_to_ps(median, freq))
}

#[allow(clippy::cast_precision_loss)]
fn mhz(freq: u64) -> f64 {
  freq as f64 / 1e6
}

#[allow(clippy::cast_precision_loss)]
fn nanos_per_call(total_nanos: u128, calls: usize) -> f64 {
  total_nanos as f64 / calls as f64
}

#[allow(clippy::cast_precision_loss)]
fn ticks_to_ps(ticks: u64, freq: u64) -> f64 {
  ticks as f64 * 1e12 / freq as f64
}
