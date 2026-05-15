# tach

`tach` is an ultra-fast drop-in replacement for `std::time::Instant`, designed for hot loops, profiling, and benchmarks.

Each supported target compiles `Instant::now()` directly to the fastest wall-clock-rate hardware counter for that architecture — RDTSC on x86 / x86_64, CNTVCT_EL0 on aarch64, rdtime on riscv64 / loongarch64 — and falls back to a platform-native monotonic clock everywhere else. No runtime dispatch, no microbenchmark at startup.

## performance

![Cross-target Instant benchmark bar chart](benches/assets/benchmark-instant.png)

![Cross-target Instant benchmark heatmap](benches/assets/benchmark-heatmap.png)

The benchmark is `Instant::now()` read cost across target / environment pairs. The fastest measured `Instant`-compatible clock is the first bar, and its name appears in parentheses under each target.

## feature comparison

| Feature                           | `tach` | `tick_counter@0.4.5` | `quanta@0.12.6` | `minstant@0.1.7` | `std::time` |
|-----------------------------------|--------|----------------------|-----------------|------------------|-------------|
| `Instant`-compatible API          | ✅     | ❌                   | ✅              | ✅               | ✅          |
| Inlined hardware counter          | ✅     | ✅                   | partial         | partial          | ❌          |
| Documented cross-thread semantics | ✅     | ❌                   | partial         | ❌               | ✅          |
| Zero dependency                   | ✅     | ✅                   | ❌              | ❌               | ✅          |

## usage

```rust
use tach::Instant;

let start = Instant::now();
// ... work ...
let elapsed = start.elapsed_ticks();

println!("{} us", elapsed.as_micros());
println!("using {} @ {} Hz", Instant::implementation(), Instant::frequency());
```

## semantics

`Instant` is **wall-clock-rate**: backed by RDTSC / CNTVCT_EL0 / rdtime, which count at a fixed architectural rate (the nominal CPU base frequency on invariant TSC; ~24 MHz on Graviton and Apple Silicon; platform timer rate on RISC-V).

- **Thread-state independent.** Keeps ticking during park / unpark, priority changes, descheduling, deep-sleep wake. The same number of ticks elapse per nanosecond whether your thread was scheduled or not.
- **Same source for every thread** in the process. All threads read from the same counter.
- **Not strictly cross-thread monotonic.** Raw hardware counters can disagree across CPUs by sub-microsecond sync slop on most hosts, and by larger margins on AMD Zen4 (CCX boundary effects). If your code requires that thread B's read be ≥ thread A's read with strict monotonicity, use `std::time::Instant` — its kernel-mediated vDSO bookkeeping enforces monotonicity at the cost of ~20–25 ns per call. tach's `Instant` is fast precisely because it does not pay that cost.

Use `Instant` for: timeouts, deadlines, latency measurements, request budgets — anywhere you want fast wall-clock time and aren't relying on strict cross-thread ordering for correctness.

## platform / architecture support

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

`Instant::now()` compiles directly to the listed hardware counter on every supported target — no runtime dispatch, same inline performance as the raw instruction.

## changelog

### 0.2.0

- Direct hardware counter inlined per supported target (RDTSC / CNTVCT_EL0 / rdtime).
- Honest documented cross-thread semantics (same source for every thread, thread-state independent; not strictly cross-thread monotonic — see `std::time::Instant` for that guarantee).
- Overflow-safe unit conversions.

### 0.1.0

- Initial release with CPU / platform tick counters, wall-time conversions, CLI diagnostics, examples, and Criterion benchmarks.
