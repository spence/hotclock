# Charter

cputicks provides a zero-dependency Rust API for reading fast CPU or platform tick counters and converting elapsed ticks into wall-clock units. The crate should be safe to use in hot runtime paths, explicit about timing guarantees and caveats, and validated on supported platforms before release.

# Milestones

## Runtime-ready timing contract

- [x] Make public API invariants and conversions idiomatic
- [x] Make `Instant` duration-first with explicit raw tick access
- [x] Add direct fast paths and tighten safety commentary
- [x] Document lazy selection and hot-path overhead by architecture
- [x] Split sampled instants from elapsed tick deltas in the public API
- [x] Clarify test-clock mocking and no-std support in the README matrix
- [x] Add cross-thread and thread-safety features to the README matrix
- [x] Convert README feature comparison into a checkbox-style feature matrix
- [x] Add requested timing crate benchmarks and feature comparison table
- [x] Rename public timestamp type from `Ticks` to `Instant`
- [x] Add README changelog for release-facing changes
- [x] Harden counter selection, documentation, and validation for runtime-wide elapsed-time use
