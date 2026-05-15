# tach

`tach` is an ultra-fast drop-in replacement for `std::time::Instant`, designed for hot loops, profiling, and benchmarks.

Each supported target compiles `Instant::now()` directly to the fastest wall-clock-rate hardware counter for that architecture — RDTSC on x86 / x86_64, CNTVCT_EL0 on aarch64, rdtime on riscv64 / loongarch64 — and falls back to a platform-native monotonic clock everywhere else.

## features

- `Instant`-compatible API
- Inlined hardware counter
- Zero dependencies

## performance

![Cross-target Instant benchmark](benches/assets/benchmark.png)
Methodology and per-target baselines: [BENCHMARKS.md](BENCHMARKS.md)

## usage

```rust
use tach::Instant;

let start = Instant::now();
// ... work ...
let elapsed = start.elapsed();
```

## semantics

`Instant` is **wall-clock-rate**: backed by RDTSC / CNTVCT_EL0 / rdtime, which count at a fixed architectural rate (the nominal CPU base frequency on invariant TSC; ~24 MHz on Graviton and Apple Silicon; platform timer rate on RISC-V).

- **Thread-state independent.** Keeps ticking during park / unpark, priority changes, descheduling, deep-sleep wake. The same number of ticks elapse per nanosecond whether your thread was scheduled or not.
- **Same source for every thread** in the process. All threads read from the same counter.
- **Not strictly cross-thread monotonic.** Raw hardware counters can disagree across CPUs by sub-microsecond sync slop on most hosts, and by larger margins on AMD Zen4 (CCX boundary effects). If your code requires that thread B's read be ≥ thread A's read with strict monotonicity, use `std::time::Instant` — its kernel-mediated vDSO bookkeeping enforces monotonicity at the cost of ~20–25 ns per call. tach's `Instant` is fast precisely because it does not pay that cost.

Use `Instant` for: timeouts, deadlines, latency measurements, request budgets — anywhere you want fast wall-clock time and aren't relying on strict cross-thread ordering for correctness.

## platform / architecture support

Dispatch is compile-time: `Instant::now()` compiles directly to the architectural counter on every supported target — no runtime check, no fallback path.

| Architecture        | `Instant::now()` counter |
|---------------------|--------------------------|
| x86_64              | RDTSC                    |
| x86                 | RDTSC                    |
| aarch64             | CNTVCT_EL0               |
| riscv64             | rdtime                   |
| loongarch64         | rdtime.d                 |

On any other target architecture, `Instant::now()` uses the platform monotonic clock instead: `mach_absolute_time` on macOS, `clock_gettime(CLOCK_MONOTONIC)` on Unix, `std::time::Instant` everywhere else.

The conversion factor from ticks to nanoseconds is read once at first use: from `cntfrq_el0` on aarch64, from `mach_timebase_info` on non-aarch64 macOS, from `QueryPerformanceFrequency` on non-aarch64 Windows, or via a one-time calibration loop on non-aarch64 Linux.

## changelog

### 0.2.0

- Initial published release.
- Minimal drop-in `Instant` API: `now()` + `elapsed()`.
- Compiles to a single architectural counter instruction on every supported target.
- Documented cross-thread semantics: same source for every thread, thread-state independent; not strictly cross-thread monotonic — see `std::time::Instant` for that guarantee.
