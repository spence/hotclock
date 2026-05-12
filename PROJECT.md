# Charter

tach provides a zero-dependency Rust API for reading fast CPU or platform tick
counters and converting elapsed ticks into wall-clock units. The crate is safe
to use in hot runtime paths, explicit about timing guarantees, and validated
on supported platforms before release.

tach makes four user-facing promises (set 2026-05-11):

1. **Fastest target-appropriate read per API per (target × env).** Within reason:
   98%+ of servers/envs by usage.
2. **Same inline performance as the chosen counter** via `Instant::now()` /
   `Cycles::now()`. No dispatch overhead on the hot path.
3. **Never crash, segfault, spawn processes, etc.** Permission-checked
   detection; no blind PMU instruction execution.
4. **Works safely across threads** under each API's documented contract.

## API surface

- **`Instant`**: wall-clock-rate, thread-state independent (keeps ticking
  through park, suspension, descheduling), same source across every thread.
  Not strictly cross-thread monotonic — use `std::time::Instant` if strict
  monotonicity is a correctness requirement.
- **`Cycles`**: fastest read for this target/env. Always returns a value. On
  Linux x86_64, runtime-selects between CPU cycle counters (RDPMC variants)
  and the Instant fallback; on every other target, `Cycles::now()`
  compile-time-resolves to the Instant tick reader. `Cycles ≤ Instant` on
  read cost, always.

# Released

## 0.2.0

- Direct hardware counter inlined per supported target for `Instant`.
- `Cycles` redesigned: always available, runtime selection on Linux x86_64
  picks between direct-RDPMC, perf-RDPMC, and RDTSC fallback; falls back to
  Instant elsewhere.
- Single-threaded selection only — no thread spawning at startup.
- Decision record: `benches/selection-falsification-2026-05-09.md`.

# Open work

Cycles selection is implemented for Linux x86_64 only in 0.2.0. Aarch64
(PMCCNTR_EL0) and RISC-V (rdcycle) Cycles selection is deferred to a
follow-up release — the May-9 falsification matrix found no production env
with PMU access enabled on these architectures, so the wall-clock fallback
matches the contract on every tested cell.
