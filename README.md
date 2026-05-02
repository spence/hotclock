# hotclock

`hotclock` is an ultra-fast drop-in replacement for `Instant` designed for hot loops, profiling and benchmarks.

Internally, it mirrors [cpucycles](https://cpucycles.cr.yp.to), so whether you're running
in a container, a VM or on bare metal, it automatically selects the fastest machine-level timer at runtime.

## performance

![Benchmark comparison](benches/assets/benchmark.png)

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

## platform / architecture support

For common modern systems, hotclock uses a direct counter where the target has
one clear path and uses runtime selection when the hardware counter can vary
by machine, kernel, or hypervisor.

| Platform               | Hardware counter | OS fallback | CI tests |
|------------------------|------------------|-------------|----------|
| macOS (x86/x86_64)     | ✅ RDTSC         | ✅          | ✅       |
| macOS (aarch64)        | ✅ CNTVCT_EL0    | n/a         | ✅       |
| Windows (x86/x86_64)   | ✅ RDTSC         | ✅          | ✅       |
| Windows (aarch64)      | ✅ CNTVCT_EL0    | ✅          | ✅       |
| Linux (x86/x86_64)     | ✅ RDTSC         | ✅          | ✅       |
| Linux (aarch64)        | ✅ CNTVCT_EL0    | ✅          | ✅       |
| Linux (s390x)          | ✅ stckf         | ✅          | ✅       |
| Linux (loongarch64)    | ✅ rdtime.d      | ✅          | ✅       |
| Unix/other (riscv64)   | ✅ rdcycle       | ✅          | ✅       |
| Unix/other (powerpc64) | ✅ mftb          | ✅          | ✅       |
| other                  | ❌               | ✅          | ✅       |

## changelog

### unreleased

- renamed the crate from `cputicks` to `hotclock`
- split the public API into `Instant` for sampled points and `Ticks` for elapsed counter deltas
- made `Instant` and `Ticks` opaque wrappers with explicit `from_raw()` / `as_raw()` access
- made standard `Instant` methods duration-first and moved raw deltas to explicit `*_ticks` APIs
- made `Ticks` wall-unit conversions return `u128` and added checked/saturating duration conversion
- split public type implementations into focused modules and tightened lint coverage
- renamed the Criterion bench target from `ticks` to `instant`
- added direct fast paths for Apple Silicon macOS and fallback-only targets
- tightened unsafe-block safety comments and removed unused `rdtscp` helpers
- hardened counter selection with same-thread and cross-thread monotonicity validation
- switched selected-counter state from mutable globals to thread-safe lazy initialization
- stopped probing Linux aarch64 `PMCCNTR_EL0` by default because it can trap when unavailable
- documented platform support and deployment caveats
- added comparator benchmarks for `quanta`, `coarsetime`, `minstant`, `fastant`, `clocksource`, `clock`, `time`, `chrono`, `tick_counter`, and `std::time::Instant`
- added performance and emoji feature-matrix comparisons for popular timing and date-time crates, including cross-thread, thread-safety, test-clock mocking, and no-std behavior
- fixed stale example version and `Cycles::now()` wording

### 0.1.0

- initial release with CPU/platform tick counters, wall-time conversions, CLI diagnostics, examples, and Criterion benchmarks
