# Handoff: Cross-architecture clock patching and validation

This document captures the exact state of the branch, the implementation intent, the commits, the
validation evidence, and the commands needed for another agent to continue or recreate the work.

## Repository and branch

Remote:

```bash
git@github-spence:spence/hotclock.git
```

Branch:

```bash
linux-x86-clock-callsite-followup
```

Implementation head before this handoff document:

```bash
94bd786caab43430bdf744ca64b612c497debb3b
```

Local worktree used for this work:

```bash
/Users/spence/src/cputicks-inline-work
```

Original crate worktree:

```bash
/Users/spence/src/cputicks
```

Merge base with `main`:

```bash
8ce421d9cb7c8b9e6ded6dc7c6b8913d5098fd67
```

## Fetching the branch

From the original crate worktree:

```bash
cd /Users/spence/src/cputicks
git fetch origin linux-x86-clock-callsite-followup
git switch -c linux-x86-clock-callsite-followup --track origin/linux-x86-clock-callsite-followup
```

To merge the completed branch into `main`:

```bash
cd /Users/spence/src/cputicks
git fetch origin linux-x86-clock-callsite-followup
git switch main
git merge origin/linux-x86-clock-callsite-followup
```

To recreate the branch in a new worktree from the merge base:

```bash
cd /Users/spence/src/cputicks
git fetch origin linux-x86-clock-callsite-followup
git worktree add ../cputicks-inline-work-recreate 8ce421d9cb7c8b9e6ded6dc7c6b8913d5098fd67
cd ../cputicks-inline-work-recreate
git switch -c linux-x86-clock-callsite-followup-recreate
git cherry-pick 25e4a03 1dc535e 61cfd13 2446f77 70fa1c0 94bd786
```

## Commit chain

The branch has these commits after `main`, in order:

```text
25e4a03 Harden Linux x86_64 patchpoint proof
1dc535e Patch selected clocks across architectures
61cfd13 Add selected clock validation benchmark
2446f77 fix(ci): restore arm windows and linux arm validation
70fa1c0 fix(bench): allow cold clock selection variance
94bd786 chore(project): complete clock validation milestone
```

## Product outcome

The completed milestone is:

```text
Clock validation benchmark proves cold warmup and warmed selected-clock cost against direct
selected-clock baselines across native CI environments
```

`PROJECT.md` marks this as complete.

## Implementation summary

The branch moves `hotclock` from selected-index dispatch toward warmed direct clock access across
supported runtime-selected targets.

The core implementation is split across these files:

```text
src/arch/selected.rs
src/arch/patch.rs
src/selection.rs
src/arch/x86_64_linux.rs
benches/clock_validation.rs
.github/workflows/ci.yml
PROJECT.md
README.md
```

Important implementation details:

- `src/arch/patch.rs` is the shared code-patching substrate. It handles page protection, cache
  flushing, and atomic patch commits for Unix, macOS, and Windows.
- `src/arch/selected.rs` emits patchable call gates for runtime-selected clocks.
- ELF targets register callsites in a `hotclock_cs` section and patch all registered callsites
  before publishing the selected clock.
- Mach-O and COFF targets self-patch the current callsite on first use after selection. This avoids
  unsupported cross-section branch relocation issues on those object formats.
- x86 and x86_64 use an 8-byte atomic commit for patched gate bytes.
- aarch64, riscv64, powerpc64, and loongarch64 patch one 32-bit branch or counter instruction.
- s390x patches to a hardware trampoline when the selected hardware counter is STCKF, because STCKF
  writes through memory rather than returning directly in a scalar register.
- `ensure_selected()` serializes initial selection with atomics. Other threads spin while one thread
  selects, patches, and publishes the selected clock.
- `Instant::implementation()` forces selection, but an ELF integration test must not initialize the
  clock without emitting a hot `Instant::now()` callsite. That invariant is why
  `tests/x86_64_linux_patch.rs` no longer calls `Instant::implementation()` on non-x86_64 Linux.

## Benchmark validation

The validation benchmark is:

```text
benches/clock_validation.rs
```

It does two jobs:

1. It spawns cold child processes with `HOTCLOCK_CLOCK_VALIDATION_CHILD=1` and reports cold first
   call timing plus which clock each child selected.
2. It warms the parent process, measures steady-state `Instant::now()` in a tight loop, measures the
   direct selected-clock baseline loop, and fails if warmed `hotclock` exceeds the direct selected
   baseline by more than:

```text
HOTCLOCK_CLOCK_VALIDATION_MAX_RATIO=1.35
HOTCLOCK_CLOCK_VALIDATION_MAX_DELTA_NS=3.0
```

Defaults:

```text
HOTCLOCK_CLOCK_VALIDATION_COLD_SAMPLES=9
HOTCLOCK_CLOCK_VALIDATION_STEADY_SAMPLES=7
HOTCLOCK_CLOCK_VALIDATION_ITERS=1000000
```

The benchmark intentionally follows the parent process's selected clock. Cold child processes can
legitimately select different clocks on unstable or noisy runners. The benchmark now reports that
distribution instead of asserting process-to-process determinism.

## CI wiring

`.github/workflows/ci.yml` runs:

```bash
cargo bench --bench clock_validation
```

on each native runner:

```text
linux-x86_64
linux-aarch64
macos-x86_64
macos-aarch64
windows-x86_64
windows-aarch64
```

The workflow also checks these cross targets:

```text
i686-unknown-linux-gnu
i686-pc-windows-msvc
aarch64-unknown-linux-gnu
aarch64-pc-windows-msvc
s390x-unknown-linux-gnu
loongarch64-unknown-linux-gnu
riscv64gc-unknown-linux-gnu
powerpc64-unknown-linux-gnu
armv7-unknown-linux-gnueabihf
```

## CI evidence

Green head run:

```text
https://github.com/spence/hotclock/actions/runs/25531589335
```

The code-bearing run that produced the native benchmark summaries:

```text
https://github.com/spence/hotclock/actions/runs/25531475497
```

Observed native benchmark summaries from run `25531475497`:

```text
macos-x86_64:
  selected: x86_64-rdtsc
  cold median: 749026 ns
  hotclock raw best: 6.696 ns/call
  baseline raw best: 6.739 ns/call
  ratio: 0.994x

macos-aarch64:
  selected: aarch64-cntvct
  cold median: 42 ns
  hotclock raw best: 0.312 ns/call
  baseline raw best: 0.312 ns/call
  ratio: 1.000x

windows-x86_64:
  selected: x86_64-rdtsc
  cold median: 659500 ns
  cold selected clocks: std-instant=6, x86_64-rdtsc=3
  hotclock raw best: 11.342 ns/call
  baseline raw best: 11.416 ns/call
  ratio: 0.994x

windows-aarch64:
  selected: aarch64-cntvct
  cold median: 1195100 ns
  hotclock raw best: 13.354 ns/call
  baseline raw best: 13.363 ns/call
  ratio: 0.999x

linux-aarch64:
  selected: aarch64-cntvct
  cold median: 900260 ns
  hotclock raw best: 13.357 ns/call
  baseline raw best: 13.355 ns/call
  ratio: 1.000x

linux-x86_64:
  selected: unix-monotonic
  cold median: 651365 ns
  hotclock raw best: 33.305 ns/call
  baseline raw best: 30.184 ns/call
  ratio: 1.103x
```

All ratios passed the benchmark threshold.

## Local validation commands used

Run from `/Users/spence/src/cputicks-inline-work`:

```bash
cargo fmt --all --check
git diff --check
cargo clippy --all-targets -- -D warnings
cargo test --lib --bins --examples --tests
cargo test --doc
cargo check --benches
cargo bench --bench clock_validation
```

Windows ARM64 codegen checks:

```bash
cargo build --lib --target aarch64-pc-windows-msvc
cargo build --lib --target aarch64-pc-windows-msvc --release
```

Linux ARM64 Docker validation:

```bash
docker run --rm --platform linux/arm64 -v "$PWD":/work -w /work rust:latest \
  bash -lc '/usr/local/cargo/bin/cargo test --lib --bins --tests'
```

CI validation:

```bash
gh run watch 25531589335 --exit-status
```

## Failure and fix history

First CI failure: Linux aarch64 integration test.

Failure:

```text
tests/x86_64_linux_patch.rs called Instant::implementation() on non-x86_64 Linux.
That initialized selected clock logic without a hot Instant::now() callsite in that integration
test crate. ELF selected-clock patching requires at least one registered callsite when selection is
forced.
```

Fix:

```text
tests/x86_64_linux_patch.rs now imports hotclock::Instant only on linux-x86_64 and the non-target
test asserts only that the current target is not linux-x86_64.
```

Second CI failure: Windows ARM64 native compile.

Failure:

```text
rustc-LLVM ERROR: Failed to evaluate function length in SEH unwind info
```

Cause:

```text
.p2align directives inside the aarch64 Windows inline assembly block caused LLVM to fail while
emitting SEH unwind metadata.
```

Fix:

```text
Removed the .p2align directives from the aarch64 Windows inline assembly gate. AArch64 instructions
are fixed-width 4-byte instructions, so the patch gate remains aligned without those directives.
```

Third CI failure: Windows x86_64 validation benchmark.

Failure:

```text
The benchmark asserted that all cold child processes selected the same clock.
Windows x86_64 CI selected x86_64-rdtsc in some child processes and std-instant in others.
```

Fix:

```text
The benchmark now records and reports the cold selected-clock distribution. The steady-state
assertion uses the parent process's selected clock and direct baseline.
```

## Notes for the next agent

- Do not weaken the warmed steady-state contract. The important promise is that after selection and
  patching, `Instant::now()` benchmarks against the direct selected-clock baseline.
- Do not reintroduce process-to-process deterministic selection assertions. Selection is based on
  measured behavior and can differ between fresh processes on noisy runners.
- Do not call `Instant::implementation()` from target-exclusion tests on ELF selected-clock targets
  unless the test crate also emits a hot selected-clock callsite.
- Do not reintroduce `.p2align` inside the aarch64 Windows inline assembly gate.
- If changing patch bytes or callsite layout, rerun the native CI matrix. Cross-checks prove
  compilation only; they do not prove native page protection, icache flushing, or benchmark parity.
