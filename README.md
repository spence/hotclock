# tach

`tach` is an ultra-fast drop-in replacement for `Instant` designed for hot loops, profiling and benchmarks.

Each supported target compiles `Instant::now()` directly to the fastest wall-clock-rate hardware counter for that architecture — RDTSC on x86_64, CNTVCT_EL0 on aarch64, rdtime on riscv64/loongarch64 — and falls back to a platform-native monotonic clock everywhere else. No runtime dispatch, no microbenchmark at startup, no patching tricks for `Instant`.

For users who want the **fastest read available** regardless of clock semantics, `tach` exposes a separate `Cycles` API. On hosts where a CPU performance-monitor counter is available AND empirically faster than the wall-clock counter, `Cycles::now()` uses it; otherwise it falls back to the same wall-clock counter `Instant::now()` uses. `Cycles ≤ Instant` on read cost, always. `Cycles::now()` always returns a value — no `Option`, no feature flag.

## performance

![Cross-target Instant benchmark bar chart](benches/assets/benchmark.png)

![Simple Instant benchmark bar chart](benches/assets/benchmark-simple.png)

![Cross-target Instant benchmark heatmap](benches/assets/benchmark-heatmap.png)

The primary benchmark is `Instant::now()` read cost across target/environment pairs. The fastest measured `Instant`-compatible clock is the first bar, and its name appears in parentheses under each target. Bars use one shared broken scale; squiggles mark the compressed upper range for Docker outliers.

## feature comparison

| Feature                          | `tach` | `tick_counter@0.4.5` | `quanta@0.12.6` | `minstant@0.1.7` | `std::time` |
|----------------------------------|--------|----------------------|-----------------|------------------|-------------|
| `Instant`-compatible API         | ✅     | ❌                   | ✅              | ✅               | ✅          |
| Inlined hardware counter         | ✅     | ✅                   | partial         | partial          | ❌          |
| CPU cycle counter (`Cycles`)     | ✅     | ❌                   | ❌              | ❌               | ❌          |
| Documented cross-thread semantics | ✅    | ❌                   | partial         | ❌               | ✅          |
| Zero dependency                  | ✅     | ✅                   | ❌              | ❌               | ✅          |

## usage

```rust
use tach::Instant;

let start = Instant::now();
// ... work ...
let elapsed = start.elapsed_ticks();

println!("{} us", elapsed.as_micros());
println!("using {} @ {} Hz", Instant::implementation(), Instant::frequency());
```

## `Instant` vs `Cycles` — two distinct measurement primitives

These are **different kinds of counter at the hardware level**, not two performance tiers on the same counter. Picking the right one for the right job matters.

### `Instant` — wall-clock-rate timer

Backed by **RDTSC / CNTVCT_EL0 / rdtime**. These count at a fixed architectural rate (the nominal CPU base frequency on invariant TSC; ~24 MHz on Graviton and Apple Silicon; platform timer rate on RISC-V).

- **Thread-state independent.** Keeps ticking during park/unpark, priority changes, descheduling, deep-sleep wake. The same number of ticks elapse per nanosecond whether your thread was scheduled or not.
- **Same source for every thread** in the process. All threads read from the same counter.
- **NOT strictly cross-thread monotonic.** Raw hardware counters can disagree across CPUs by sub-microsecond sync slop on most hosts, and by larger margins on AMD Zen4 (CCX boundary effects). If your code requires that thread B's read be ≥ thread A's read with strict monotonicity, use `std::time::Instant` — its kernel-mediated vDSO bookkeeping enforces monotonicity at the cost of ~20–25 ns per call. tach's `Instant` is fast precisely because it does not pay that cost.

Use `Instant` for: timeouts, deadlines, latency measurements, request budgets — anywhere you want fast wall-clock time and aren't relying on strict cross-thread ordering for correctness.

### `Cycles` — fastest read available

`Cycles::now()` returns the fastest counter for this target/env. On hosts where a CPU performance-monitor counter is available AND empirically faster than the wall-clock counter, tach picks the PMU counter (RDPMC fixed cycle counter on Linux x86_64, PMCCNTR_EL0 on Linux aarch64, rdcycle on Linux riscv64). Otherwise `Cycles` uses the same wall-clock counter `Instant` uses — so `Cycles ≤ Instant` on read cost, always. **`Cycles::now()` always returns a value.**

When backed by a PMU counter, `Cycles` has cycle-counter semantics:

- **Counts CPU work, not wall time.** Stops during park, sleep, idle, halt.
- **Scales with frequency.** A P-state change to a higher clock makes the counter tick faster; this is observable as throttling.
- **Per-core, never synchronized** across cores. If a thread migrates mid-measurement, the counter jumps by an arbitrary amount.

When backed by the wall-clock fallback (most targets, or any host with PMU access denied), `Cycles` has `Instant`'s semantics. Use `Cycles::implementation()` if you need to know which counter was selected for the current process.

Use `Cycles` for: profilers, cryptographic micro-benchmarks (cpucycles-style), JIT hotspot accounting, perf-aware CI — wherever you want the fastest read and are comfortable with the cycle-counter semantics on hosts where a PMU counter is available.

## platform / architecture support

| Platform / target       | `Instant` clock      | `Cycles` selection                          | OS fallback                |
|-------------------------|----------------------|---------------------------------------------|----------------------------|
| Linux (x86_64)          | RDTSC                | RDPMC fixed / perf-RDPMC vs RDTSC           | clock_gettime              |
| Linux (x86)             | RDTSC                | RDPMC fixed / perf-RDPMC vs RDTSC           | clock_gettime              |
| Linux (aarch64)         | CNTVCT_EL0           | PMCCNTR_EL0 / perf-PMCCNTR vs CNTVCT_EL0    | clock_gettime              |
| Linux (riscv64)         | rdtime               | rdcycle / perf-rdcycle vs rdtime            | clock_gettime              |
| macOS (aarch64)         | CNTVCT_EL0           | = Instant (no user-mode PMU)                | —                          |
| macOS (x86_64)          | RDTSC                | = Instant (no user-mode RDPMC)              | mach_absolute_time         |
| Windows (x86_64)        | RDTSC                | = Instant (no user-mode PMU)                | QueryPerformanceCounter    |
| Windows (aarch64)       | CNTVCT_EL0           | = Instant (no user-mode PMU)                | QueryPerformanceCounter    |
| Linux (loongarch64)     | rdtime.d             | = Instant (no user PMU)                     | clock_gettime              |
| Unix/other              | OS timer / CNTVCT_EL0| = Instant                                   | clock_gettime              |

`Instant::now()` compiles directly to the listed hardware counter on every supported target — no runtime dispatch, no patchpoints, same inline performance as the raw instruction.

`Cycles::now()` on rows showing `= Instant` compile-time-resolves to the same instruction as `Instant`. On the Linux rows with multiple candidates, the first call runs single-threaded selection, picks the fastest counter that beats `Instant`'s clock on this specific host, and patches every callsite to inline the winner's bytes. Subsequent reads have the same inline cost as the raw instruction. If no PMU candidate beats `Instant`'s clock (e.g., on virtualized hosts where perf-RDPMC reads through the hypervisor), selection picks `Instant`'s clock as the fallback — `Cycles::now()` still returns a value, it just returns the same one `Instant::now()` would.

## design rationale

The May-9 falsification matrix in [`benches/selection-falsification-2026-05-09.md`](benches/selection-falsification-2026-05-09.md) measured 25 (target × environment) cells across AWS bare-metal, virtualized, Lambda, GitHub-hosted runners, macOS, Windows, and Docker. The data drove two decisions:

1. **No runtime selection for `Instant`.** Every measured cell of every supported target picked the same wall-clock-rate counter. Selection's multi-candidate latency comparison adds startup cost and code complexity without ever changing the winner on any measured host.
2. **`Cycles` as the "fastest read for this target/env" API with `Instant` as the guaranteed fallback.** Where a PMU cycle counter is available AND empirically faster than the wall-clock counter, `Cycles::now()` uses it; otherwise it uses the same wall-clock counter `Instant::now()` uses. The wall-clock counter is always one of the selection candidates, so `Cycles ≤ Instant` on read cost is guaranteed. `Cycles::implementation()` exposes which counter was selected so callers can introspect.

The deletion was a net 4,000+ LOC reduction; the simpler design honors the same four user-facing promises (fastest target-appropriate clock, inline performance, never crash/segfault/spawn, safe cross-thread use under each API's documented semantic).

## changelog

### 0.2.0

- Direct hardware counter inlined per supported target (RDTSC / CNTVCT_EL0 / rdtime).
- `Cycles` API for true CPU-cycle counting, gated behind host OS permission.
- Honest documented cross-thread semantics (same source for every thread, thread-state independent; not strictly cross-thread monotonic — see `std::time::Instant` for that guarantee).
- Overflow-safe unit conversions.

### 0.1.0

- Initial release with CPU/platform tick counters, wall-time conversions, CLI diagnostics, examples, and Criterion benchmarks.
