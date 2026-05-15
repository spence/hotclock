#![allow(clippy::cast_precision_loss)]

use std::hint::black_box;
use std::time::Instant as StdInstant;

use criterion::{Criterion, criterion_group, criterion_main};
use tach::Instant;

fn bench_tach_now(c: &mut Criterion) {
  c.bench_function("tach::Instant::now()", |b| b.iter(|| black_box(Instant::now())));
}

fn bench_tach_elapsed(c: &mut Criterion) {
  c.bench_function("tach::Instant (now + elapsed)", |b| {
    b.iter(|| {
      let start = Instant::now();
      black_box(start.elapsed())
    });
  });
}

fn bench_tach_elapsed_fast(c: &mut Criterion) {
  c.bench_function("tach::Instant (now + elapsed_fast)", |b| {
    b.iter(|| {
      let start = Instant::now();
      black_box(start.elapsed_fast())
    });
  });
}

fn bench_quanta_now(c: &mut Criterion) {
  quanta::Instant::now();
  c.bench_function("quanta::Instant::now()", |b| b.iter(|| black_box(quanta::Instant::now())));
}

fn bench_quanta_elapsed(c: &mut Criterion) {
  quanta::Instant::now();
  c.bench_function("quanta::Instant (now + elapsed)", |b| {
    b.iter(|| {
      let start = quanta::Instant::now();
      black_box(start.elapsed())
    });
  });
}

fn bench_fastant_now(c: &mut Criterion) {
  fastant::Instant::now();
  c.bench_function("fastant::Instant::now()", |b| b.iter(|| black_box(fastant::Instant::now())));
}

fn bench_fastant_elapsed(c: &mut Criterion) {
  fastant::Instant::now();
  c.bench_function("fastant::Instant (now + elapsed)", |b| {
    b.iter(|| {
      let start = fastant::Instant::now();
      black_box(start.elapsed())
    });
  });
}

fn bench_minstant_now(c: &mut Criterion) {
  minstant::Instant::now();
  c.bench_function("minstant::Instant::now()", |b| b.iter(|| black_box(minstant::Instant::now())));
}

fn bench_minstant_elapsed(c: &mut Criterion) {
  minstant::Instant::now();
  c.bench_function("minstant::Instant (now + elapsed)", |b| {
    b.iter(|| {
      let start = minstant::Instant::now();
      black_box(start.elapsed())
    });
  });
}

fn bench_std_now(c: &mut Criterion) {
  c.bench_function("std::time::Instant::now()", |b| b.iter(|| black_box(StdInstant::now())));
}

fn bench_std_elapsed(c: &mut Criterion) {
  c.bench_function("std::time::Instant (now + elapsed)", |b| {
    b.iter(|| {
      let start = StdInstant::now();
      black_box(start.elapsed())
    });
  });
}

criterion_group!(
  benches,
  bench_tach_now,
  bench_tach_elapsed,
  bench_tach_elapsed_fast,
  bench_quanta_now,
  bench_quanta_elapsed,
  bench_fastant_now,
  bench_fastant_elapsed,
  bench_minstant_now,
  bench_minstant_elapsed,
  bench_std_now,
  bench_std_elapsed,
);
criterion_main!(benches);
