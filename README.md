# hotclock

`hotclock` is an ultra-fast drop-in replacement for `Instant` designed for hot loops, profiling and benchmarks.

Internally, it mirrors [cpucycles](https://cpucycles.cr.yp.to), so whether you're running
in a container, a VM or on bare metal, it automatically selects the fastest machine-level timer at runtime.

## performance

![Benchmark comparison](benches/assets/benchmark.png)

Full target/environment results: [inline selected-clock benchmark](benches/inline-selection-benchmark-2026-05-08.md).

## feature comparison

| Feature                 | `hotclock` | `tick_counter@0.4.5` | `quanta@0.12.6` | `minstant@0.1.7` | `std::time` |
|-------------------------|------------|----------------------|-----------------|------------------|-------------|
| `Instant` API           | ✅         | ❌                   | ✅              | ✅               | ✅          |
| runtime clock selection | ✅         | ❌                   | ✅              | ✅               | ❌          |
| CPU tick access         | ✅         | ✅                   | ✅              | ❌               | ❌          |
| zero dependency         | ✅         | ✅                   | ❌              | ❌               | ✅          |

## usage

```rust
use hotclock::Instant;

let start = Instant::now();
// ... work ...
let elapsed = start.elapsed_ticks();

println!("{} us", elapsed.as_micros());
println!("using {} @ {} Hz", Instant::implementation(), Instant::frequency());
```

## `Instant` vs `Cycles`

`Instant` is the safe elapsed-time clock. It uses the fastest counter that stays
monotonic across OS thread migration and cross-thread handoffs. Use it for
deadlines, budgets, runtime scheduling, cross-thread timestamps, and measurements
that must survive descheduling, suspend/resume, or VM movement.

`Cycles` is the lower-level hot-loop clock contract. It is an `Instant`-shaped
counter for taking a sample, taking another sample, and subtracting them, but it
can use faster machine counters such as RDPMC, PMCCNTR_EL0, rdcycle, or the
fastest equivalent source for the target. It does not carry `Instant`'s
cross-thread or OS-thread-event guarantees. Use it for same-thread
microbenchmarks, profilers, tight polling loops, and short measurements where
clock read cost dominates.

## platform / architecture support

For common modern systems, hotclock uses a direct counter where the target has
one clear path and uses runtime selection when the hardware counter can vary
by machine, kernel, or hypervisor.

Runtime selection is thread-safe on the first racing call. Selected targets with
crate-owned patchpoints rewrite warmed `Instant` and `Cycles` call sites to the
chosen counter or fallback trampoline, so later reads do not keep selected-index
dispatch on the hot path.

| Platform               | Hardware counter | OS fallback | CI tests |
|------------------------|------------------|-------------|----------|
| macOS (x86/x86_64)     | ✅ RDTSC         | ✅          | ✅       |
| macOS (aarch64)        | ✅ CNTVCT_EL0    | n/a         | ✅       |
| Windows (x86/x86_64)   | ✅ RDTSC         | ✅          | ✅       |
| Windows (aarch64)      | ✅ CNTVCT_EL0    | ✅          | ✅       |
| Linux (x86/x86_64)     | ✅ RDTSC         | ✅          | ✅       |
| Linux (aarch64)        | ✅ CNTVCT_EL0    | ✅          | ✅       |
| Linux (s390x)          | ❌              | ✅          | ✅       |
| Linux (loongarch64)    | ✅ rdtime.d      | ✅          | ✅       |
| Unix/other (riscv64)   | ✅ rdtime        | ✅          | ✅       |
| Unix/other (powerpc64) | ❌               | ✅          | ✅       |
| other                  | ❌               | ✅          | ✅       |

## changelog

### 0.2.0

- `Instant` API compatability
- skip selection for known fast hardware counters
- thread-safe `OnceLock` timer selection
- overflow-safe unit conversions

### 0.1.0

- initial release with CPU/platform tick counters, wall-time conversions, CLI diagnostics, examples, and Criterion benchmarks
