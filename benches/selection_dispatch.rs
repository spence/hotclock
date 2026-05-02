#![allow(clippy::inline_always)]

use std::hint::black_box;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use criterion::{Criterion, criterion_group, criterion_main};

const FAST_COUNTER: u8 = 0;
const SLOW_COUNTER: u8 = 1;
const UNSELECTED: u8 = u8::MAX;

static CURRENT_SELECTED: AtomicU8 = AtomicU8::new(FAST_COUNTER);
static CURRENT_INIT: OnceLock<()> = OnceLock::new();
static mut CONSTRUCTOR_SELECTED: u8 = FAST_COUNTER;

struct ColdState {
  selected: AtomicU8,
  init: OnceLock<()>,
}

impl ColdState {
  fn new() -> Self {
    Self { selected: AtomicU8::new(UNSELECTED), init: OnceLock::new() }
  }

  #[inline]
  fn selected(&self) -> u8 {
    let selected = self.selected.load(Ordering::Acquire);
    if selected != UNSELECTED {
      return selected;
    }

    self.init.get_or_init(|| {
      self.selected.store(FAST_COUNTER, Ordering::Release);
    });

    self.selected.load(Ordering::Acquire)
  }
}

#[inline(never)]
fn install_constructor_selected(selected: u8) {
  // SAFETY: This benchmark writes once during setup, before the measured single-threaded reads.
  unsafe {
    CONSTRUCTOR_SELECTED = selected;
  }
}

#[inline(always)]
fn constructor_selected() -> u8 {
  // SAFETY: This mirrors the old hot path: a plain load from startup-installed global state.
  unsafe { CONSTRUCTOR_SELECTED }
}

#[inline(always)]
fn current_selected() -> u8 {
  let selected = CURRENT_SELECTED.load(Ordering::Acquire);
  if selected != UNSELECTED {
    return selected;
  }

  CURRENT_INIT.get_or_init(|| {
    CURRENT_SELECTED.store(FAST_COUNTER, Ordering::Release);
  });

  CURRENT_SELECTED.load(Ordering::Acquire)
}

#[inline(always)]
fn read_selected(selected: u8) -> u64 {
  match selected {
    FAST_COUNTER => platform_counter(),
    SLOW_COUNTER => platform_counter().wrapping_add(1),
    _ => {
      // SAFETY: All benchmark call paths install only the declared counter indices.
      unsafe { std::hint::unreachable_unchecked() }
    }
  }
}

#[inline(always)]
fn constructor_style_now() -> u64 {
  read_selected(constructor_selected())
}

#[inline(always)]
fn current_style_now() -> u64 {
  read_selected(current_selected())
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn platform_counter() -> u64 {
  let value: u64;
  // SAFETY: CNTVCT_EL0 is the architectural virtual count register.
  unsafe {
    core::arch::asm!(
      "mrs {value}, cntvct_el0",
      value = out(reg) value,
      options(nostack, nomem, preserves_flags),
    );
  }
  value
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline(always)]
fn platform_counter() -> u64 {
  let high: u32;
  let low: u32;
  // SAFETY: RDTSC reads the processor timestamp counter and does not touch memory or the stack.
  unsafe {
    core::arch::asm!(
      "rdtsc",
      out("edx") high,
      out("eax") low,
      options(nostack, nomem, preserves_flags),
    );
  }
  (u64::from(high) << 32) | u64::from(low)
}

#[cfg(not(any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")))]
#[inline(always)]
fn platform_counter() -> u64 {
  static START: OnceLock<std::time::Instant> = OnceLock::new();
  START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64
}

fn bench_selected_index(c: &mut Criterion) {
  install_constructor_selected(black_box(FAST_COUNTER));
  let _ = current_selected();

  let mut group = c.benchmark_group("selected index");
  group.bench_function("old constructor global", |b| b.iter(|| black_box(constructor_selected())));
  group.bench_function("current atomic fast path", |b| b.iter(|| black_box(current_selected())));
  group.bench_function("current cold init machinery", |b| {
    b.iter(|| {
      let state = ColdState::new();
      black_box(state.selected())
    });
  });
  group.finish();
}

fn bench_selected_counter(c: &mut Criterion) {
  install_constructor_selected(black_box(FAST_COUNTER));
  let _ = current_style_now();

  let mut group = c.benchmark_group("selected counter");
  group.bench_function("direct counter", |b| b.iter(|| black_box(platform_counter())));
  group
    .bench_function("old constructor dispatch", |b| b.iter(|| black_box(constructor_style_now())));
  group.bench_function("current atomic dispatch", |b| b.iter(|| black_box(current_style_now())));
  group.finish();
}

criterion_group!(benches, bench_selected_index, bench_selected_counter);
criterion_main!(benches);
