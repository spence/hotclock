# hotclock

The fastest cross-platform timer for hot paths

__hotclock__ is a zero-dependency crate for elapsed timing, automatically
choosing the fastest usable clock across bare metal, containers, and VMs.

## why hotclock?

Local Apple Silicon timings are from the benchmark table below. `Elapsed` means
sampling a start point and computing the cheapest elapsed delta the crate
exposes: raw ticks for `hotclock` and `tick_counter`, `Duration` for
Instant-style APIs. Slowdown values are relative to `hotclock` in the same row.

| Feature / cost           | `hotclock` | `tick_counter`          | `quanta`                | `minstant`              | `std::time`             |
|--------------------------|------------|-------------------------|-------------------------|-------------------------|-------------------------|
| `now()`                  | 343 ps     | 346 ps (1.0x)           | 4.51 ns (13.1x slower)  | 26.76 ns (78.0x slower) | 19.72 ns (57.5x slower) |
| elapsed                  | 640 ps     | 643 ps (1.0x)           | 9.04 ns (14.1x slower)  | 59.32 ns (92.7x slower) | 42.86 ns (67.0x slower) |
| `Instant` API            | ✅         | ❌                       | ✅                      | ✅                      | ✅                       |
| runtime-selected clock   | ✅         | ❌                       | ✅                      | ✅                      | ❌                       |
| OS fallback              | ✅         | ❌                       | ✅                      | ✅                      | n/a                      |
| CPU tick access          | ✅         | ✅                       | ✅                      | ❌                      | ❌                       |
| monotonic across threads | ✅         | ❌                       | ✅                      | ✅                      | ✅                       |
| zero dependency          | ✅         | ✅                       | ❌                      | ❌                      | ✅                       |

`CPU tick access` means the public API exposes CPU/platform counter samples or
elapsed tick deltas without forcing conversion through `Duration`.

## usage

```rust
use hotclock::Instant;

let start = Instant::now();
// ... work ...
let elapsed = start.elapsed();

println!("{} us", elapsed.as_micros());
println!("using {} @ {} Hz", Instant::implementation(), Instant::frequency());
```

## general features

- reads compiled-in hardware counters directly where the platform has one clear path
- validates and selects the fastest counter where runtime behavior can vary
- falls back to OS monotonic timers when runtime-selected hardware counters fail validation
- returns `std::time::Duration` from the familiar `Instant` API
- exposes raw tick access explicitly for hot paths that want hardware units
- caches frequency calibration for the lifetime of the process
- has zero runtime dependencies
- works across macOS, Linux, Windows, and other Rust targets with a timer fallback

## platform / architecture support

For common modern systems, hotclock uses a direct counter where the target has
one clear path and keeps runtime validation where the hardware counter can vary
by machine, kernel, or hypervisor.

| Platform             | OS fallback | Hardware counter? | CI tests? |
|----------------------|-------------|-------------------|-----------|
| Linux (x86/x86_64)   | ✅          | ✅ RDTSC          | ❌        |
| Linux (aarch64)      | ✅          | ✅ CNTVCT_EL0     | ❌        |
| Windows (x86/x86_64) | ✅          | ✅ RDTSC          | ❌        |
| Windows (aarch64)    | ✅          | ✅ CNTVCT_EL0     | ❌        |
| macOS (x86/x86_64)   | ✅          | ✅ RDTSC          | ❌        |
| macOS (aarch64)      | ❌          | ✅ CNTVCT_EL0     | ❌        |
| riscv64              | ✅          | ✅ rdcycle        | ❌        |
| powerpc64            | ✅          | ✅ mftb           | ❌        |
| s390x                | ✅          | ✅ stckf          | ❌        |
| loongarch64          | ✅          | ✅ rdtime.d       | ❌        |
| other                | ✅          | ❌                | ❌        |

OS timers are `mach_absolute_time` on macOS, `clock_gettime(CLOCK_MONOTONIC)`
on Unix, and `std::time::Instant` elsewhere. Unsupported architectures compile
directly to the relevant OS timer instead of running runtime selection.

## feature comparison

`✅` means the crate exposes the feature as part of its documented API or
feature set. `❌` means it does not.

| Crate          | CPU counter | Raw tick access | Auto fallback | Monotonic elapsed | Cross-thread order | Send + Sync | Thread-safe init | Elapsed `Duration` | Instant-style API | Coarse/recent | Wall timestamp | Calendar types | Format/parse | Offsets/zones | Test clock mocking | Atomic storage | Serde/archive | Cross-platform | no_std | No ext deps |
|----------------|-------------|-----------------|---------------|-------------------|--------------------|-------------|------------------|--------------------|-------------------|---------------|----------------|----------------|--------------|---------------|--------------------|----------------|---------------|----------------|--------|-------------|
| `hotclock`     | ✅          | ✅              | ✅            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅                | ❌            | ❌             | ❌             | ❌           | ❌            | ❌                 | ❌             | ❌            | ✅             | ❌     | ✅          |
| `tick_counter` | ✅          | ✅              | ❌            | ✅                | ❌                 | ✅          | ✅               | ❌                 | ❌                | ❌            | ❌             | ❌             | ❌           | ❌            | ❌                 | ❌             | ❌            | ❌             | ❌     | ✅          |
| `quanta`       | ✅          | ✅              | ✅            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅                | ✅            | ❌             | ❌             | ❌           | ❌            | ✅                 | ❌             | ❌            | ✅             | ❌     | ❌          |
| `coarsetime`   | ❌          | ❌              | ❌            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅                | ✅            | ✅             | ❌             | ❌           | ❌            | ❌                 | ❌             | ❌            | ✅             | ❌     | ❌          |
| `minstant`     | ✅          | ❌              | ✅            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅                | ✅            | ✅             | ❌             | ❌           | ❌            | ❌                 | ✅             | ❌            | ✅             | ❌     | ❌          |
| `fastant`      | ✅          | ❌              | ✅            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅                | ✅            | ✅             | ❌             | ❌           | ❌            | ❌                 | ✅             | ❌            | ✅             | ❌     | ❌          |
| `clocksource`  | ❌          | ❌              | ❌            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅                | ✅            | ✅             | ✅             | ❌           | ❌            | ❌                 | ✅             | ❌            | ✅             | ❌     | ❌          |
| `clock`        | ❌          | ❌              | ❌            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅                | ❌            | ✅             | ❌             | ❌           | ❌            | ✅                 | ❌             | ❌            | ✅             | ❌     | ✅          |
| `time`         | ❌          | ❌              | ❌            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅                | ❌            | ✅             | ✅             | ✅           | ✅            | ❌                 | ❌             | ✅            | ✅             | ✅     | ❌          |
| `chrono`       | ❌          | ❌              | ❌            | ❌                | ❌                 | ✅          | ✅               | ❌                 | ❌                | ❌            | ✅             | ✅             | ✅           | ✅            | ❌                 | ❌             | ✅            | ✅             | ✅     | ❌          |
| `std::time`    | ❌          | ❌              | ❌            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅                | ❌            | ✅             | ❌             | ❌           | ❌            | ❌                 | ❌             | ❌            | ✅             | ❌     | ✅          |

Notes: `Raw tick access` means the crate exposes raw counter samples or deltas.
`Cross-thread order` means ordered reads across threads are expected to be
nondecreasing for the monotonic API; it is stricter than `Send + Sync`.
`Thread-safe init` means concurrent first use is safe, including crates with no
shared initialization path. `Test clock mocking` means the crate provides a
test-controlled clock API. `no_std` means the crate can be built without
`std` using documented feature settings. `quanta` hardware counter support is
TSC-only on x86/x86_64. `minstant` and `fastant` use TSC only on Linux
x86/x86_64 and otherwise fall back to `std::time`. `time`'s monotonic `Instant`
support is a deprecated re-export; its primary offering is civil date/time.

## caveats

Hardware counters depend on CPU, firmware, kernel, and hypervisor behavior.
Modern Apple Silicon and modern invariant-TSC x86 systems are expected to work
well. Old multi-socket machines, unsynchronized TSC systems, and some virtualized
environments can still have counter behavior that is not representative of local
developer hardware. The crate validates runtime-selected counters, but unusual
production hardware should still be tested under its real deployment conditions.

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
