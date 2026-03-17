# cputicks

A Rust port of [libcpucycles](https://cpucycles.cr.yp.to/) for high-resolution timing.

Provides sub-nanosecond timing by directly reading hardware counters (RDTSC, CNTVCT_EL0, etc.) with automatic runtime selection of the best available source. ~30x faster than `std::time::Instant`.

```rust
use cputicks::Ticks;

let start = Ticks::now();
// ... work ...
println!("{:?}", start.elapsed().as_duration());

println!("Using: {} @ {} Hz", Ticks::implementation(), Ticks::frequency());
```

## CLI

```
$ cargo run --bin cputicks --release
cputicks v0.1.0

Implementation: aarch64-cntvct
Frequency:      24000000 Hz (24.00 MHz)
Overhead:       650 ps per call
Resolution:     0 ticks (0 ps)
```

## Platform Support

| Architecture    | Primary       | Fallback     |
| --------------- | ------------- | ------------ |
| x86_64          | ✓ RDTSC       | ✓ OS timer   |
| x86             | ✓ RDTSC       | ✓ OS timer   |
| aarch64         | ✓ CNTVCT_EL0  | ✓ OS timer   |
| aarch64 (Linux) | ✓ PMCCNTR_EL0 | ✓ CNTVCT_EL0 |
| riscv64         | ✓ rdcycle     | ✓ OS timer   |
| powerpc64       | ✓ mftb        | ✓ OS timer   |
| s390x           | ✓ stckf       | ✓ OS timer   |
| loongarch64     | ✓ rdtime.d    | ✓ OS timer   |
| other           |               | ✓ OS timer   |

OS timers: `mach_absolute_time` (macOS), `clock_gettime` (Unix), `Instant` (other)

## Benchmarks

Apple M1 (aarch64-cntvct @ 24 MHz):

| Operation                            | Time    |
| ------------------------------------ | ------- |
| `Ticks::now()`                       | 658 ps  |
| `Ticks::now() + elapsed()`           | 882 ps  |
| `std::time::Instant::now()`          | 20.0 ns |
| `std::time::Instant (now + elapsed)` | 43.3 ns |
