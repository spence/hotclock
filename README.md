# tach

`tach` is an ultra-fast drop-in replacement for `std::time::Instant`, designed for tracing, hot loops, profiling, and benchmarks.

Each supported target compiles `Instant::now()` directly to the fastest wall-clock-rate hardware counter for that architecture — RDTSC on x86 / x86_64, CNTVCT_EL0 on aarch64, rdtime on riscv64 / loongarch64 — and falls back to a platform-native monotonic clock everywhere else.

## features

- `Instant`-compatible API
- Inlined hardware counter
- Ordered counter reads via `OrderedInstant` — a contract no other Rust time crate offers
- Zero dependencies

## performance

![Cross-target Instant benchmark](benches/summary.png)
Methodology and per-target baselines: [BENCHMARKS.md](BENCHMARKS.md)

## usage

```rust
let start = tach::Instant::now();
let elapsed = start.elapsed();
```

## semantics

`Instant` is **wall-clock-rate**: backed by RDTSC / CNTVCT_EL0 / rdtime, which count at a fixed architectural rate (the nominal CPU base frequency on invariant TSC; ~24 MHz on Graviton and Apple Silicon; platform timer rate on RISC-V).

- **Thread-state independent.** Keeps ticking during park / unpark, priority changes, descheduling, deep-sleep wake. The same number of ticks elapse per nanosecond whether your thread was scheduled or not.
- **Same source for every thread** in the process. All threads read from the same counter.
- **Not strictly cross-thread monotonic.** Raw hardware counters can disagree across CPUs by sub-microsecond sync slop on most hosts, and by larger margins on AMD Zen4 (CCX boundary effects). If your code requires that thread B's read be ≥ thread A's read with strict monotonicity, use `std::time::Instant` — its kernel-mediated vDSO bookkeeping enforces monotonicity at the cost of ~20–25 ns per call. tach's `Instant` is fast precisely because it does not pay that cost.

Use `Instant` for: timeouts, deadlines, latency measurements, request budgets — anywhere you want fast wall-clock time and aren't relying on strict cross-thread ordering for correctness.

## ordered reads

Plain `Instant::now()` is intentionally minimal — a single counter instruction with no synchronization barrier. That's a hazard if you correlate timestamps with atomics:

```rust
let deadline = scheduler.load(Ordering::Acquire);
let now = tach::Instant::now();   // ← can be sampled BEFORE `deadline` is observed
```

On aarch64 `mrs cntvct_el0` is a system-register read; on x86 `rdtsc` is not a serializing instruction. Memory fences (including the `Acquire` load) do not constrain when those reads execute, so the timestamp can drift earlier than the synchronization point. No fast Rust time crate (`quanta`, `fastant`, `minstant`, or `tach`) addresses this; `std::time::Instant` only does on Windows (kernel boundary via `QueryPerformanceCounter`).

Use `OrderedInstant` when you need the contract *"my timestamp is sampled after any prior `Acquire`-or-stronger observation"*:

```rust
let deadline = scheduler.load(Ordering::Acquire);
let now = tach::OrderedInstant::now();  // safe to correlate with `deadline`
```

`OrderedInstant::now()` emits the arch-appropriate barrier before the counter read (`isb sy` on aarch64, `lfence` on x86, `fence ir, ir` on riscv64, `dbar 0` on loongarch64). Fallback paths (`clock_gettime`, `mach_absolute_time`, WASI `clock_time_get`, `Performance.now()`) already cross a kernel / runtime / JS boundary that serializes naturally.

Cost is ~5–20 ns more than `Instant::now()` depending on architecture; still substantially faster than `std::time::Instant::now()` on Linux and macOS, where std uses the vDSO / libsystem path but does not itself guarantee this ordering against atomics.

`OrderedInstant::elapsed_unordered()` is an explicit escape hatch for "ordered start, fast end" — only use it when the end of the measurement is for logging or coarse reporting and doesn't itself need to come after a synchronization point. `OrderedInstant::as_unordered()` returns a plain `Instant` carrying the same tick value (useful when storing in a struct field typed as `Instant`); there is no inverse, since an unordered read can't retroactively be ordered.

## platform / architecture support

Dispatch is compile-time: `Instant::now()` compiles directly to the architectural counter on every supported target — no runtime check, no fallback path.

| Architecture                    | `Instant::now()` counter   |
|---------------------------------|----------------------------|
| x86_64                          | RDTSC                      |
| x86                             | RDTSC                      |
| aarch64                         | CNTVCT_EL0                 |
| riscv64                         | rdtime                     |
| loongarch64                     | rdtime.d                   |
| wasm32 (browser / Node host)    | `Performance.now()`        |


| Platform / target       | `Instant` clock      | OS fallback                |
|-------------------------|----------------------|----------------------------|
| Linux (x86_64)          | RDTSC                | clock_gettime              |
| Linux (x86)             | RDTSC                | clock_gettime              |
| Linux (aarch64)         | CNTVCT_EL0           | clock_gettime              |
| Linux (riscv64)         | rdtime               | clock_gettime              |
| Linux (loongarch64)     | rdtime.d             | clock_gettime              |
| macOS (aarch64)         | CNTVCT_EL0           | —                          |
| macOS (x86_64)          | RDTSC                | mach_absolute_time         |
| Windows (x86_64)        | RDTSC                | QueryPerformanceCounter    |
| Windows (aarch64)       | CNTVCT_EL0           | QueryPerformanceCounter    |
| Unix / other            | OS timer             | clock_gettime              |


On any other target architecture, `Instant::now()` uses the platform monotonic clock: `mach_absolute_time` on macOS, `clock_gettime(CLOCK_MONOTONIC)` on Unix, `clock_time_get(MONOTONIC)` on WASI.

The crate is `#![no_std]`. On `wasm32-unknown-unknown` and `wasm32v1-none`, `Instant::now()` calls `globalThis.performance.now()` via `wasm-bindgen` — the host (browser / Node / embedder) must expose a JS `performance.now()` function. Browsers typically clamp the resolution to ~100 microseconds for Spectre mitigation; successive calls within that window return identical values (non-decreasing, not strictly increasing). The `wasm-bindgen` dependency is only pulled in for those two targets — every other target remains zero-dependency. WASI targets (`wasm32-wasip1`, `wasm32-wasip2`, `wasm32-wasip1-threads`) call `clock_time_get` directly via a `wasi_snapshot_preview1` import — no `wasi` crate dependency. Emscripten goes through POSIX `clock_gettime` like any other Unix target.

The conversion factor from ticks to nanoseconds is read once at first use: from `cntfrq_el0` on aarch64, from `mach_timebase_info` on non-aarch64 macOS, from `QueryPerformanceFrequency` on non-aarch64 Windows, fixed at 1 GHz on `wasm32` and WASI (those clocks already return nanoseconds), or via a one-time calibration loop on non-aarch64 Linux.

## changelog

### 0.2.0

- Initial published release.
- Minimal drop-in `Instant` API: `now()` + `elapsed()`.
- Compiles to a single architectural counter instruction on every supported target.
- Documented cross-thread semantics: same source for every thread, thread-state independent; not strictly cross-thread monotonic — see `std::time::Instant` for that guarantee.
