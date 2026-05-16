#![allow(clippy::cast_precision_loss)]

use std::hint::black_box;
use std::time::Instant as StdInstant;

use criterion::{Criterion, criterion_group, criterion_main};
use tach::Instant;

fn bench_now(c: &mut Criterion) {
  // Prime the lazy frequency calibration so it doesn't land in the first
  // measured sample.
  quanta::Instant::now();
  fastant::Instant::now();
  minstant::Instant::now();

  let mut g = c.benchmark_group("Instant::now()");
  g.bench_function("tach", |b| b.iter(|| black_box(Instant::now())));
  g.bench_function("quanta", |b| b.iter(|| black_box(quanta::Instant::now())));
  g.bench_function("fastant", |b| b.iter(|| black_box(fastant::Instant::now())));
  g.bench_function("minstant", |b| b.iter(|| black_box(minstant::Instant::now())));
  g.bench_function("std", |b| b.iter(|| black_box(StdInstant::now())));
  g.finish();
}

fn bench_elapsed(c: &mut Criterion) {
  quanta::Instant::now();
  fastant::Instant::now();
  minstant::Instant::now();

  let mut g = c.benchmark_group("Instant::now() + elapsed()");
  g.bench_function("tach", |b| {
    b.iter(|| {
      let start = Instant::now();
      black_box(start.elapsed())
    });
  });
  g.bench_function("quanta", |b| {
    b.iter(|| {
      let start = quanta::Instant::now();
      black_box(start.elapsed())
    });
  });
  g.bench_function("fastant", |b| {
    b.iter(|| {
      let start = fastant::Instant::now();
      black_box(start.elapsed())
    });
  });
  g.bench_function("minstant", |b| {
    b.iter(|| {
      let start = minstant::Instant::now();
      black_box(start.elapsed())
    });
  });
  g.bench_function("std", |b| {
    b.iter(|| {
      let start = StdInstant::now();
      black_box(start.elapsed())
    });
  });
  g.finish();
}

criterion_group!(benches, bench_now, bench_elapsed);
criterion_main!(benches);
