# tach

A replacement for `std::time::Instant` that reads the architectural counter directly: RDTSC on x86, CNTVCT_EL0 on aarch64, rdtime on riscv64 / loongarch64.

[![docs.rs](https://docs.rs/tach/badge.svg)](https://docs.rs/tach)
[![crates.io](https://img.shields.io/crates/v/tach.svg)](https://crates.io/crates/tach)

## usage

```rust
use tach::{Instant, OrderedInstant};

// drop-in for std::time::Instant
let start = Instant::now();
let elapsed = start.elapsed();

// same API, sampled after prior Acquire loads
let ordered = OrderedInstant::now();
let elapsed = ordered.elapsed();
```

## benchmark

![benchmark](benches/summary-wide.png)

Methodology and per-target reports: [BENCHMARKS.md](BENCHMARKS.md).

## semantics

The counter is wall-clock-rate. It keeps ticking through park, suspension, and descheduling. All threads in the process read the same source. It is **not** strictly cross-thread monotonic: raw hardware counters can disagree across CPUs by sub-microsecond sync slop, and by larger margins on AMD Zen4 (CCX boundary effects). If that matters, use `std::time::Instant`, which the kernel coerces into per-thread monotonicity at the cost of ~20 ns per call.

## ordered reads

A plain counter read can be reordered earlier than a preceding `Acquire` load:

```rust
let deadline = scheduler.load(Ordering::Acquire);
let now = tach::Instant::now();   // may be sampled before `deadline` is observed
```

`mrs cntvct_el0` is a system-register read; `rdtsc` is not a serializing instruction. Memory fences don't constrain when either executes. `OrderedInstant` emits the per-arch barrier (`isb sy` on aarch64, `lfence` on x86) before the counter read, restoring the order:

```rust
let deadline = scheduler.load(Ordering::Acquire);
let now = tach::OrderedInstant::now();   // sampled after `deadline`
```

Cost is ~5–20 ns more than `Instant::now()`. `OrderedInstant::as_unordered()` downgrades to a plain `Instant` for storage; the reverse is not provided.

On riscv64 (`fence iorw, iorw`) and loongarch64 (`dbar 0`) the strongest available memory barrier is used; whether memory fences constrain CSR reads is implementation-defined on those targets, so the guarantee is best-effort.

## platform support

| Platform / target               | `Instant` clock                  |
|---------------------------------|----------------------------------|
| Linux (x86_64)                  | RDTSC                            |
| Linux (x86)                     | RDTSC                            |
| Linux (aarch64)                 | CNTVCT_EL0                       |
| Linux (riscv64)                 | rdtime                           |
| Linux (loongarch64)             | rdtime.d                         |
| macOS (aarch64)                 | CNTVCT_EL0                       |
| macOS (x86_64)                  | RDTSC                            |
| Windows (x86_64)                | RDTSC                            |
| Windows (aarch64)               | CNTVCT_EL0                       |
| wasm32 (browser / Node host)    | `Performance.now()`              |
| WASI (wasm32-wasip{1,2})        | `clock_time_get(MONOTONIC)`      |
| Unix / other                    | `clock_gettime(CLOCK_MONOTONIC)` |

The crate is `#![no_std]`. `wasm-bindgen` is the only dependency, pulled in only for `wasm32-unknown-unknown` and `wasm32v1-none` (the targets that go through `Performance.now()`).

## drift

`elapsed()` can diverge from true wall-clock time over long intervals. Drift is *per-interval* — a 1-minute measurement made 5 seconds into the process has the same drift as one made 100 days in. Numbers below assume room-temperature operation; rows marked kernel-corrected assume no NTP, with active discipline they drop another order of magnitude.

| Crate | 1-sec interval | 1-min interval | 1-hr interval | 1-day interval |
|---|---|---|---|---|
| `tach::Instant` (default, `#![no_std]`) | ~50 µs | ~3 ms | ~180 ms | ~4 s |
| `tach::Instant` + `recalibrate-background` (**requires `std`**) | ~1 µs | ~60 µs | ~4 ms | ~86 ms |
| `tach::OrderedInstant` (default, `#![no_std]`) | ~50 µs | ~3 ms | ~180 ms | ~4 s |
| `quanta::Instant` | ~500 µs | ~30 ms | ~1.8 s | ~43 s |
| `minstant::Instant` (Linux x86 only) | ~500 µs | ~30 ms | ~1.8 s | ~43 s |
| `fastant::Instant` (Linux x86 only) | ~500 µs | ~30 ms | ~1.8 s | ~43 s |
| `std::time::Instant` (Linux / Windows) | ~1 µs | ~60 µs | ~4 ms | ~86 ms |
| `std::time::Instant` (macOS / aarch64) | ~50 µs | ~3 ms | ~180 ms | ~4 s |

For sub-second timing, every row is below the precision floor of any practical measurement — the differentiation appears at minute/hour/day scale.

Two ways to close the gap on long-running services:

- **`tach::Instant::recalibrate()`** — manual, `#![no_std]`-compatible. Call from your own scheduler whenever you want to re-derive the scaling against `clock_gettime`. Costs ~10 ms of spin-loop time per call. Works on every supported target including embedded and SGX.
- **`recalibrate-background` Cargo feature** — automatic. Spawns a background thread that calls `recalibrate()` every 60 seconds (interval configurable via `tach::set_recalibration_interval`). **This feature requires `std` and is incompatible with `#![no_std]` targets** — it pulls in `std::thread` and `std::sync::OnceLock`. The default tach build is `#![no_std]`; enabling this feature is the only thing that promotes the crate to `std`. Use the manual path if you need both drift correction and no_std.

Within a single process, two tach measurements are mutually consistent — drift only shows up when comparing against an external reference (NTP-disciplined wall clock, another process, etc.).

## non-goals

- Strict cross-thread monotonicity. Use `std::time::Instant`.
- Clock-skew correction across machines. This is a per-process counter.

## msrv

Rust 1.85.

## license

MIT OR Apache-2.0.
