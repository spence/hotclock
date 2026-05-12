# tach Agent Rules

## Benchmarking Standard

Benchmark data for a target/environment combination must include:

- `tach::Instant::now()` and `tach::Cycles::now()` through the public API.
- The active comparison crates (quanta, minstant, fastant, std::time, etc.).
- Every direct clock primitive available for that architecture (RDTSC,
  CNTVCT_EL0, rdtime, etc., plus PMU primitives where applicable).

For each (target × environment) run:

- Verify `Cycles ≤ Instant + 0.5 ns`. The contract is `Cycles ≤ Instant` on
  read cost (selection always includes the Instant counter as a candidate).
  A violation indicates a selection or patching bug, not measurement noise —
  sharpen the measurement instead of widening the tolerance.
- Confirm `Instant::now()` cost tracks the direct primitive cost closely
  enough to support the inline-performance claim (Promise 2).
- Confirm `Cycles::now()` cost tracks the chosen primitive cost (which may
  be the Instant counter on targets without a faster PMU path).

Record clocks that are unavailable, permission-blocked, unsupported,
panicking, or faulting as explicit results with the reason. Do not omit
them silently.

Record the target triple, environment (hypervisor/container/bare-metal
class), CPU model, OS/kernel version, Rust version, sample settings, and
benchmark command alongside the results.
