# Charter

tach provides a zero-dependency Rust API for reading fast CPU or platform tick counters and converting elapsed ticks into wall-clock units. The crate should be safe to use in hot runtime paths, explicit about timing guarantees and caveats, and validated on supported platforms before release.

# Milestones

## Cross-architecture inline selected clocks

- [x] Fix macOS x86_64 self-patching call-target corruption
- [x] Rename crate and GitHub repository to `tach`
- [x] Render README benchmark chart as grouped vertical bars
- [x] Tighten benchmark chart bar geometry and legend
- [x] Replace noisy m7i GNU row with pinned high-confidence benchmark
- [x] Keep heatmap and bar chart README benchmark graphics
- [x] Convert README performance graphic to a bar chart
- [x] Add cross-target `Instant` benchmark graphic to README
- [x] Document deferred signal-probed direct `RDPMC` plan for `Cycles`
- [x] Add GNU m7i metal benchmark row showing stronger `Cycles` performance
- [x] Surface the `Cycles`-only speedup without extra table columns
- [x] Stabilize i686 bare-metal RDTSC validation false negatives
- [x] README shows fresh runtime-selection benchmarks across selected targets
- [x] Stable fallback-only targets skip selected patch trampolines
- [x] Native ppc64 stable Rust builds without unstable inline asm
- [x] Runtime-selected `Instant` and `Cycles` clocks patch warmed call sites across supported targets
- [x] Release-mode elapsed tests tolerate same-tick reads

## Thread-safe inline Linux clock selection

- [x] Linux x86_64 runtime-selected clocks patch warmed hardware-counter calls to direct inline clock bytes

## Runtime-ready timing contract

- [x] Ship separate `Cycles` counter selection and validation
- [x] Prove selected clocks match fastest-known clocks per target environment
- [x] Clarify `Instant` and `Cycles` as separate clock contracts
- [x] Use RISC-V `rdtime` for Instant-class elapsed timing
- [x] Benchmark selected-clock dispatch overhead
- [x] Cut concise 0.2.0 release notes
- [x] Refine README performance framing
- [x] Reframe README rationale around when to choose tach
- [x] Move benchmark-generated assets under benches
- [x] Retarget crate keywords for timer and profiling search
- [x] Retarget crate positioning around benchmarks, profilers, and hot loops
- [x] Simplify the top README feature comparison
- [x] Use Node 24-compatible GitHub Actions checkout
- [x] Add GitHub Actions CI across supported platforms
- [x] Collapse unreleased changelog
- [x] Add spacing between benchmark bars and metrics
- [x] Compact benchmark image layout
- [x] Remove rendered benchmark title
- [x] Scale README benchmark bars by slowdown
- [x] Render README benchmark as a PNG image
- [x] Separate README benchmark image from feature comparison
- [x] Move comparator versions to the top README comparison
- [x] Add comparator versions to feature matrix
- [x] Reorder platform support columns
- [x] Clarify direct macOS aarch64 platform support
- [x] Sort platform table by OS priority
- [x] Clarify non-mainstream platform rows
- [x] Rename crate to `tach`
- [x] Remove redundant README detail sections
- [x] Treat tick_counter as performance-equivalent in README
- [x] Clarify raw tick access and relative benchmark scores
- [x] Remove cautious dependency wording from README
- [x] Clarify README elapsed timing labels
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
