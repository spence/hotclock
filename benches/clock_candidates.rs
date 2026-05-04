#![cfg(feature = "bench-internals")]
#![allow(clippy::cast_precision_loss)]

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use hotclock::Instant;
use hotclock::bench_internals::candidate_clocks;

fn bench_candidate_clocks(c: &mut Criterion) {
  println!("Target: {}-{}", std::env::consts::OS, std::env::consts::ARCH);
  println!("Hotclock implementation: {}", Instant::implementation());
  println!("Hotclock frequency: {:.2} MHz", Instant::frequency() as f64 / 1e6);

  let mut group = c.benchmark_group("clock candidates");
  group.bench_function("hotclock-selected", |b| b.iter(|| black_box(Instant::now().as_raw())));

  for candidate in candidate_clocks() {
    if candidate.requires_child_process {
      continue;
    }

    let read = candidate.read;
    group.bench_function(candidate.name, |b| b.iter(|| black_box(read())));
  }

  group.finish();
}

criterion_group!(benches, bench_candidate_clocks);
criterion_main!(benches);
