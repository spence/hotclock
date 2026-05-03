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

On Linux x86_64, runtime selection is thread-safe on the first racing call and
warmed RDTSC selections patch crate-owned call sites to direct clock bytes, so
later hardware-counter reads do not keep selected-index dispatch on the hot
path.

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

### 0.2.0

- `Instant` API compatability
- skip selection for known fast hardware counters
- thread-safe timer selection, including patched warmed RDTSC call sites on Linux x86_64
- overflow-safe unit conversions

### 0.1.0

- initial release with CPU/platform tick counters, wall-time conversions, CLI diagnostics, examples, and Criterion benchmarks
