#![allow(clippy::cast_precision_loss)]

use std::hint::black_box;
use std::time::Instant as StdInstant;

use cputicks::Instant;
use criterion::{Criterion, criterion_group, criterion_main};

fn bench_instant(c: &mut Criterion) {
  println!("Using: {}", Instant::implementation());
  println!("Frequency: {:.2} MHz", Instant::frequency() as f64 / 1e6);

  c.bench_function("Instant::now()", |b| b.iter(|| black_box(Instant::now())));
}

fn bench_instant_elapsed(c: &mut Criterion) {
  c.bench_function("Instant::now() + elapsed()", |b| {
    b.iter(|| {
      let start = Instant::now();
      black_box(start.elapsed())
    });
  });
}

fn bench_instant_elapsed_ticks(c: &mut Criterion) {
  c.bench_function("Instant::now() + elapsed_ticks()", |b| {
    b.iter(|| {
      let start = Instant::now();
      black_box(start.elapsed_ticks())
    });
  });
}

fn bench_quanta_instant(c: &mut Criterion) {
  quanta::Instant::now();
  c.bench_function("quanta::Instant::now()", |b| b.iter(|| black_box(quanta::Instant::now())));
}

fn bench_quanta_instant_elapsed(c: &mut Criterion) {
  quanta::Instant::now();
  c.bench_function("quanta::Instant (now + elapsed)", |b| {
    b.iter(|| {
      let start = quanta::Instant::now();
      black_box(start.elapsed())
    });
  });
}

fn bench_minstant(c: &mut Criterion) {
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

fn bench_fastant(c: &mut Criterion) {
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

fn bench_coarsetime_instant(c: &mut Criterion) {
  coarsetime::Instant::now();
  c.bench_function("coarsetime::Instant::now()", |b| {
    b.iter(|| black_box(coarsetime::Instant::now()));
  });
}

fn bench_coarsetime_instant_elapsed(c: &mut Criterion) {
  coarsetime::Instant::now();
  c.bench_function("coarsetime::Instant (now + elapsed)", |b| {
    b.iter(|| {
      let start = coarsetime::Instant::now();
      black_box(start.elapsed())
    });
  });
}

fn bench_time_offset_date_time(c: &mut Criterion) {
  c.bench_function("time::OffsetDateTime::now_utc()", |b| {
    b.iter(|| black_box(time::OffsetDateTime::now_utc()));
  });
}

fn bench_clock_monotonic(c: &mut Criterion) {
  let clock = clock::MonotonicClock::default();
  c.bench_function("clock::MonotonicClock::now()", |b| b.iter(|| black_box(clock.now())));
}

fn bench_chrono_utc(c: &mut Criterion) {
  c.bench_function("chrono::Utc::now()", |b| b.iter(|| black_box(chrono::Utc::now())));
}

fn bench_clocksource_precise(c: &mut Criterion) {
  c.bench_function("clocksource::precise::Instant::now()", |b| {
    b.iter(|| black_box(clocksource::precise::Instant::now()));
  });
}

fn bench_clocksource_precise_elapsed(c: &mut Criterion) {
  c.bench_function("clocksource::precise::Instant (now + elapsed)", |b| {
    b.iter(|| {
      let start = clocksource::precise::Instant::now();
      black_box(start.elapsed())
    });
  });
}

fn bench_tick_counter(c: &mut Criterion) {
  c.bench_function("tick_counter::start()", |b| b.iter(|| black_box(tick_counter::start())));
}

fn bench_tick_counter_elapsed(c: &mut Criterion) {
  c.bench_function("tick_counter::TickCounter (current + elapsed)", |b| {
    b.iter(|| {
      let start = tick_counter::TickCounter::current();
      black_box(start.elapsed())
    });
  });
}

fn bench_std_instant(c: &mut Criterion) {
  c.bench_function("std::time::Instant::now()", |b| b.iter(|| black_box(StdInstant::now())));
}

fn bench_std_instant_elapsed(c: &mut Criterion) {
  c.bench_function("std::time::Instant (now + elapsed)", |b| {
    b.iter(|| {
      let start = StdInstant::now();
      black_box(start.elapsed())
    });
  });
}

criterion_group!(
  benches,
  bench_instant,
  bench_instant_elapsed,
  bench_instant_elapsed_ticks,
  bench_quanta_instant,
  bench_quanta_instant_elapsed,
  bench_minstant,
  bench_minstant_elapsed,
  bench_fastant,
  bench_fastant_elapsed,
  bench_coarsetime_instant,
  bench_coarsetime_instant_elapsed,
  bench_time_offset_date_time,
  bench_clock_monotonic,
  bench_chrono_utc,
  bench_clocksource_precise,
  bench_clocksource_precise_elapsed,
  bench_tick_counter,
  bench_tick_counter_elapsed,
  bench_std_instant,
  bench_std_instant_elapsed,
);
criterion_main!(benches);
