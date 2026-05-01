# cputicks

The fastest cross-platform timer for hot paths

__cputicks__ is a zero-dependency crate for elapsed timing, automatically
choosing the fastest usable clock across bare metal, containers, and VMs.

## why cputicks?

Local Apple Silicon timings are from the benchmark table below. `Fast elapsed
delta` means sampling a start point and computing the cheapest elapsed delta the
crate exposes: raw ticks for `cputicks` and `tick_counter`, `Duration` for
Instant-style APIs.

| Feature / cost           | `cputicks` | `tick_counter` | `quanta` | `minstant` | `std::time` |
|--------------------------|------------|----------------|----------|------------|-------------|
| `now()`                  | 343 ps     | 346 ps         | 4.51 ns  | 26.76 ns   | 19.72 ns    |
| Fast elapsed delta       | 640 ps     | 643 ps         | 9.04 ns  | 59.32 ns   | 42.86 ns    |
| `Duration` elapsed       | 3.65 ns    | ❌             | 9.04 ns  | 59.32 ns   | 42.86 ns    |
| Zero normal dependencies | ✅         | ✅             | ❌       | ❌         | ✅          |
| Runtime-selected clock   | ✅         | ❌             | ✅       | ✅         | ❌          |
| Hardware counters        | ✅         | ✅             | ✅       | ✅         | ❌          |
| OS fallback              | ✅         | ❌             | ✅       | ✅         | n/a         |
| Instant-style API        | ✅         | ❌             | ✅       | ✅         | ✅          |
| Raw ticks                | ✅         | ✅             | ✅       | ❌         | ❌          |
| Monotonic across threads | ✅         | ❌             | ✅       | ✅         | ✅          |

## usage

```rust
use cputicks::Instant;

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
- exposes raw counter ticks explicitly for hot paths that want hardware units
- caches frequency calibration for the lifetime of the process
- has zero normal runtime dependencies
- works across macOS, Linux, Windows, and other Rust targets with a timer fallback

## timing contract

`cputicks::Instant` is a `Copy + Send + Sync` opaque `u64` wrapper around a
counter sample. Its raw value is not a civil-time or OS timestamp. Use it as a
point in the process-wide counter timeline.

`cputicks::Ticks` is the elapsed/raw counter-delta type. It represents an amount
of counter movement, not a sampled point in time. `Ticks` converts elapsed ticks
to nanoseconds, microseconds, milliseconds, seconds, and `Duration`.
Nanosecond/microsecond/millisecond conversions return `u128` so large elapsed
tick counts do not silently truncate. `Ticks::checked_duration()` returns
`None` when the converted value does not fit in `std::time::Duration`;
`Ticks::as_duration()` saturates at `Duration::MAX`.

`Instant::now()` reads the process-wide counter. Direct targets use the
compiled-in counter path. Runtime-selected targets require the selected counter
to be nondecreasing for repeated reads on one thread and for reads ordered across
threads by a release/acquire handoff. Selection falls back to the OS timer when
the preferred hardware counter does not satisfy that contract during validation.

`Instant::elapsed()`, `Instant::duration_since()`, and `end - start` return
`std::time::Duration` to match the familiar Rust timing API. Use
`Instant::elapsed_ticks()`, `Instant::ticks_since()`,
`Instant::checked_ticks_since()`, or `Instant::wrapping_ticks_since()` when raw
counter deltas are required.

Runtime counter selection and `Instant::frequency()` calibration are lazy and
thread-safe. Calling `Instant::frequency()` during runtime startup pre-warms
calibration and, on runtime-selected targets, counter selection for later `Ticks`
conversions. Tick-to-duration conversion methods return integer wall-clock units
and truncate fractional units toward zero.

## platform / architecture support

For common modern systems, cputicks uses a direct counter where the target has
one clear path and keeps runtime validation where the hardware counter can vary
by machine, kernel, or hypervisor.

| Platform / architecture | OS fallback | Hardware counter support | Notes                         |
|-------------------------|-------------|--------------------------|-------------------------------|
| Linux (x86/x86_64)      | yes         | RDTSC                    | selected after validation     |
| Linux (aarch64)         | yes         | CNTVCT_EL0               | PMCCNTR_EL0 is not probed     |
| macOS (x86/x86_64)      | yes         | RDTSC                    | falls back to mach time       |
| macOS (aarch64)         | no          | CNTVCT_EL0               | direct Apple Silicon counter  |
| Windows (x86/x86_64)    | yes         | RDTSC                    | falls back to `Instant`       |
| Windows (aarch64)       | yes         | CNTVCT_EL0               | falls back to `Instant`       |
| riscv64                 | yes         | rdcycle                  | selected after validation     |
| powerpc64               | yes         | mftb                     | selected after validation     |
| s390x                   | yes         | stckf                    | selected after validation     |
| loongarch64             | yes         | rdtime.d                 | selected after validation     |
| other                   | yes         | none                     | OS timer only                 |

OS timers are `mach_absolute_time` on macOS, `clock_gettime(CLOCK_MONOTONIC)`
on Unix, and `std::time::Instant` elsewhere. Unsupported architectures compile
directly to the relevant OS timer instead of running runtime selection.

## selection overhead

Counter selection is lazy, thread-safe, and process-wide on runtime-selected
targets. The first call that needs a raw counter runs selection once behind
`OnceLock`; later calls reuse the cached counter index. Selection tests each
candidate with two initial reads inside `catch_unwind`, 1,001 same-thread
monotonic reads, and 1,000 release/acquire cross-thread handoffs.
Selection validates candidates that execute successfully; it is not a signal
handler for OS-level illegal-instruction or privilege faults.

Frequency calibration is separate. The first `Instant::frequency()` call, or the
first `Ticks` conversion that needs wall-clock units, runs five 10 ms calibration
samples and caches the median result. Plain `Instant::now()` does not calibrate
frequency after selection has completed.

| Target family | First-use candidates              | Lazy selection role                             | Steady `Instant::now()` work             | Direct path policy                                |
|---------------|-----------------------------------|-------------------------------------------------|------------------------------------------|---------------------------------------------------|
| x86/x86_64    | RDTSC + OS timer                  | validates TSC behavior, VM behavior, fallback   | atomic load, branch/match, selected read | keep runtime-selected by default                  |
| aarch64 macOS | CNTVCT_EL0 only                   | none                                            | direct counter read                      | compiled-in Apple Silicon path                    |
| aarch64 Linux | CNTVCT_EL0 + `clock_gettime`      | validates successful reads and VM behavior      | atomic load, branch/match, selected read | keep runtime-selected by default                  |
| aarch64 other | CNTVCT_EL0 + `std::time::Instant` | validates successful reads                      | atomic load, branch/match, selected read | keep runtime-selected by default                  |
| riscv64       | rdcycle + OS timer                | validates successful reads and ordering         | atomic load, branch/match, selected read | keep runtime-selected by default                  |
| powerpc64     | mftb + OS timer                   | validates successful reads and ordering         | atomic load, branch/match, selected read | keep runtime-selected by default                  |
| s390x         | stckf + OS timer                  | validates successful reads and ordering         | atomic load, branch/match, selected read | keep runtime-selected by default                  |
| loongarch64   | rdtime.d + OS timer               | validates successful reads and ordering         | atomic load, branch/match, selected read | keep runtime-selected by default                  |
| other         | OS timer only                     | none                                            | direct fallback read                     | compiled-in fallback path                         |

The default API now uses direct reads where the target has one useful answer and
runtime selection where the fastest compiled counter can vary by deployment.
Frequency calibration remains lazy on all targets.

## performance

Local Apple Silicon run (`aarch64-cntvct`, 24 MHz), measured with Criterion
(`cargo bench --bench instant -- --sample-size 10 --measurement-time 1`):

`Fast elapsed delta` is the cheapest start-to-elapsed measurement exposed by
the crate. For `cputicks`, that is an explicit raw tick delta; `elapsed()`
returns `Duration` and is shown separately.

| Crate / API                     | `now()`  | Fast elapsed delta | `Duration` elapsed | Notes                                           |
|---------------------------------|----------|--------------------|--------------------|-------------------------------------------------|
| `cputicks`                      | 343 ps   | 640 ps             | 3.65 ns            | direct Apple Silicon counter; raw ticks explicit |
| `tick_counter`                  | 346 ps   | 643 ps             | n/a                | raw tick reads; no wall-unit conversion         |
| `quanta`                        | 4.51 ns  | n/a                | 9.04 ns            | std fallback on this macOS aarch64 host         |
| `coarsetime`                    | 5.83 ns  | n/a                | 11.37 ns           | speed-oriented coarse/recent-time API           |
| `std::time`                     | 19.72 ns | n/a                | 42.86 ns           | baseline OS monotonic timer                     |
| `clock`                         | 19.98 ns | n/a                | n/a                | `MonotonicClock::now()`, wraps std on this host |
| `clocksource::precise`          | 24.25 ns | n/a                | 46.71 ns           | fixed-size precise monotonic instant            |
| `minstant`                      | 26.76 ns | n/a                | 59.32 ns           | std fallback on this macOS aarch64 host         |
| `fastant`                       | 27.18 ns | n/a                | 58.48 ns           | std fallback on this macOS aarch64 host         |
| `time::OffsetDateTime::now_utc` | 34.50 ns | n/a                | n/a                | wall-clock UTC timestamp, not monotonic timing  |
| `chrono::Utc::now`              | 41.74 ns | n/a                | n/a                | timezone-aware wall-clock timestamp             |

Comparator versions: `quanta` 0.12.6, `coarsetime` 0.1.37, `minstant` 0.1.7,
`fastant` 0.1.11, `clocksource` 1.0.0, `clock` 0.4.0, `time` 0.3.47,
`chrono` 0.4.44, and `tick_counter` 0.4.5. These numbers are host-specific:
`cputicks` uses a direct Apple Silicon path on macOS aarch64 and runtime
selection elsewhere. `minstant` and `fastant` use their TSC path on Linux
x86/x86_64, while `quanta` uses TSC on x86/x86_64 platforms and falls back
elsewhere.

Use `cargo run --bin cputicks --release` to print the selected implementation,
calibrated frequency, call overhead, and observed resolution on the current
machine.

## feature comparison

`✅` means the crate exposes the feature as part of its documented API or
feature set. `❌` means it does not.

| Crate          | CPU counter | Raw ticks | Auto fallback | Monotonic elapsed | Cross-thread order | Send + Sync | Thread-safe init | Elapsed `Duration` | Instant-style API | Coarse/recent | Wall timestamp | Calendar types | Format/parse | Offsets/zones | Test clock mocking | Atomic storage | Serde/archive | Cross-platform | no_std | No ext deps |
|----------------|-------------|-----------|---------------|-------------------|--------------------|-------------|------------------|--------------------|-------------|---------------|----------------|----------------|--------------|---------------|--------------------|----------------|---------------|----------------|--------|-------------|
| `cputicks`     | ✅          | ✅        | ✅            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅          | ❌            | ❌             | ❌             | ❌           | ❌            | ❌                 | ❌             | ❌            | ✅             | ❌     | ✅          |
| `tick_counter` | ✅          | ✅        | ❌            | ✅                | ❌                 | ✅          | ✅               | ❌                 | ❌          | ❌            | ❌             | ❌             | ❌           | ❌            | ❌                 | ❌             | ❌            | ❌             | ❌     | ✅          |
| `quanta`       | ✅          | ✅        | ✅            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅          | ✅            | ❌             | ❌             | ❌           | ❌            | ✅                 | ❌             | ❌            | ✅             | ❌     | ❌          |
| `coarsetime`   | ❌          | ❌        | ❌            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅          | ✅            | ✅             | ❌             | ❌           | ❌            | ❌                 | ❌             | ❌            | ✅             | ❌     | ❌          |
| `minstant`     | ✅          | ❌        | ✅            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅          | ✅            | ✅             | ❌             | ❌           | ❌            | ❌                 | ✅             | ❌            | ✅             | ❌     | ❌          |
| `fastant`      | ✅          | ❌        | ✅            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅          | ✅            | ✅             | ❌             | ❌           | ❌            | ❌                 | ✅             | ❌            | ✅             | ❌     | ❌          |
| `clocksource`  | ❌          | ❌        | ❌            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅          | ✅            | ✅             | ✅             | ❌           | ❌            | ❌                 | ✅             | ❌            | ✅             | ❌     | ❌          |
| `clock`        | ❌          | ❌        | ❌            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅          | ❌            | ✅             | ❌             | ❌           | ❌            | ✅                 | ❌             | ❌            | ✅             | ❌     | ✅          |
| `time`         | ❌          | ❌        | ❌            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅          | ❌            | ✅             | ✅             | ✅           | ✅            | ❌                 | ❌             | ✅            | ✅             | ✅     | ❌          |
| `chrono`       | ❌          | ❌        | ❌            | ❌                | ❌                 | ✅          | ✅               | ❌                 | ❌          | ❌            | ✅             | ✅             | ✅           | ✅            | ❌                 | ❌             | ✅            | ✅             | ✅     | ❌          |
| `std::time`    | ❌          | ❌        | ❌            | ✅                | ✅                 | ✅          | ✅               | ✅                 | ✅          | ❌            | ✅             | ❌             | ❌           | ❌            | ❌                 | ❌             | ❌            | ✅             | ❌     | ✅          |

Notes: `Cross-thread order` means ordered reads across threads are expected to
be nondecreasing for the monotonic API; it is stricter than `Send + Sync`.
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
- documented the timing contract, calibration behavior, platform support, and deployment caveats
- documented one-time lazy selection, steady-state call overhead, and raw fast-path tradeoffs by architecture
- added comparator benchmarks for `quanta`, `coarsetime`, `minstant`, `fastant`, `clocksource`, `clock`, `time`, `chrono`, `tick_counter`, and `std::time::Instant`
- added performance and emoji feature-matrix comparisons for popular timing and date-time crates, including cross-thread, thread-safety, test-clock mocking, and no-std behavior
- fixed stale example version and `Cycles::now()` wording

### 0.1.0

- initial release with CPU/platform tick counters, wall-time conversions, CLI diagnostics, examples, and Criterion benchmarks
