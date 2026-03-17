use std::hint::black_box;
use std::time::Instant;

use cputicks::Ticks;
use criterion::{Criterion, criterion_group, criterion_main};

fn bench_ticks(c: &mut Criterion) {
  println!("Using: {}", Ticks::implementation());
  println!("Frequency: {:.2} MHz", Ticks::frequency() as f64 / 1e6);

  c.bench_function("Ticks::now()", |b| b.iter(|| black_box(Ticks::now())));
}

fn bench_ticks_elapsed(c: &mut Criterion) {
  c.bench_function("Ticks::now() + elapsed()", |b| {
    b.iter(|| {
      let start = Ticks::now();
      black_box(start.elapsed())
    })
  });
}

fn bench_std_instant(c: &mut Criterion) {
  c.bench_function("std::time::Instant::now()", |b| b.iter(|| black_box(Instant::now())));
}

fn bench_std_instant_elapsed(c: &mut Criterion) {
  c.bench_function("std::time::Instant (now + elapsed)", |b| {
    b.iter(|| {
      let start = Instant::now();
      black_box(start.elapsed())
    })
  });
}

criterion_group!(
  benches,
  bench_ticks,
  bench_ticks_elapsed,
  bench_std_instant,
  bench_std_instant_elapsed,
);
criterion_main!(benches);
