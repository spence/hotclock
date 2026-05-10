# Selection falsification, 2026-05-09

This run extends the May-8 runtime selection validation
(`benches/runtime-selection-validation-2026-05-08.md`) with **falsification
cells**: target × environment combinations chosen to test whether tach's
runtime clock selection would pick a different winner per environment, and
whether `Cycles` is meaningfully faster than `Instant` on enough production
targets to keep maintaining it.

The May-8 data — 19 measured cells — picked the same `Instant` clock for every
benchmarked target × environment, and Cycles only beat Instant by ~19–23 % on
one cell (AWS m7i metal x86_64-linux). This run was designed to break that
pattern.

## Decision rules (locked before data)

- **Rule 1 — `Instant` selection.** Keep iff at least one benchmarked target
  shows env-dependent winner variance (the same target triple picks different
  winners under different envs). Otherwise drop selection; use a direct
  compile-time path per target.
- **Rule 2 — `Cycles` API.** Keep iff at least one production-relevant cell
  shows Cycles latency ≤ 70 % of Instant latency (≥ 30 % reduction).
  Otherwise drop the `Cycles` API.
- **Rule 3 — `Cycles` selection.** Only meaningful if Rule 2 passes and the
  same target picks different cycle winners across envs.

## Implementation note: `aarch64-perf-pmccntr` candidate added

To give the aarch64 cells a fair test against
[libcpucycles](https://cpucycles.cr.yp.to/counters.html)'s candidate set, this
run introduces `aarch64-perf-pmccntr` (`src/arch/perf_pmccntr_linux.rs`) — a
`perf_event_open + mmap` reader for the PMUv3 cycle counter, mirroring the
existing `x86_64-perf-rdpmc`. It is wired in as a `Cycles` candidate ahead of
`aarch64-cntvct` and behind `aarch64-pmccntr` direct. On aarch64 Linux,
`/proc/sys/kernel/perf_user_access` gates PMUSERENR_EL0 system-wide, so this
new candidate has the same availability as direct PMCCNTR_EL0 in practice;
it remains valuable for parity with cpucycles' enumeration and for any
environment where direct register access is blocked but perf-mapped reads
are not.

## Results — falsification matrix

Phase B numbers (`TACH_VALIDATION_MEASURE_ITERS=5_000_000`,
`TACH_VALIDATION_SAMPLES=101`, `taskset -c {0,2}` where supported). Cycles vs
Instant ratio is `tach-cycles-bench / tach-instant-bench`. Lower is better.

| Environment        | Target              | Instant        | Cycles         | Instant ns | Cycles ns | Cycles/Instant | quanta  | minstant | fastant | std    |
|--------------------|---------------------|----------------|----------------|------------|-----------|----------------|---------|----------|---------|--------|
| AWS c7g.metal      | aarch64-linux-gnu   | aarch64-cntvct | aarch64-cntvct | 6.668      | 6.668     | 1.000          | 7.045   | 38.980   | 38.784  | 31.426 |
| AWS c7g.4xlarge    | aarch64-linux-gnu   | aarch64-cntvct | aarch64-cntvct | 6.676      | 6.675     | 1.000          | 7.064   | 38.956   | 38.874  | 31.472 |
| AWS m7i.4xlarge    | x86_64-linux-gnu    | x86_64-rdtsc   | x86_64-rdtsc   | 14.718     | 14.718    | 1.000          | 17.529  | 15.032   | 15.032  | 26.069 |
| AWS c7a.4xlarge †  | x86_64-linux-gnu    | x86_64-rdtsc   | x86_64-rdtsc   | 9.486      | 9.486     | 1.000          | 9.486   | 9.486    | 9.486   | 23.040 |
| AWS c7a.metal-48xl | x86_64-linux-gnu    | x86_64-rdtsc   | x86_64-rdtsc   | 9.469      | 9.468     | 1.000          | 9.469   | 9.469    | 9.468   | 22.998 |
| GitHub ubuntu-arm  | aarch64-linux-gnu   | aarch64-cntvct | aarch64-cntvct | 13.357     | 13.357    | 1.000          | 13.381  | 38.016   | 37.912  | 31.087 |
| GitHub ubuntu      | x86_64-linux-gnu ‡  | x86_64-rdtsc   | x86_64-rdtsc   | 12.296     | 12.294    | 1.000          | 12.303  | 12.295   | 12.295  | 29.882 |

Notes:
- † `c7a.4xlarge` (AMD virtualized) was added in addition to the planned
  `c7a.metal-48xl` cell because the 192-vCPU metal exceeded the 256-vCPU
  bucket while the other cells were running. The metal cell ran sequentially
  after `c7g.metal` terminated; both AMD rows are reported.
- ‡ GitHub's `ubuntu-24.04` x86_64 hosted runner reports
  `vendor_id: AuthenticAMD` — the row is a free additional AMD virtualized
  data point.

## Per-candidate trace

For each cell, every clock candidate registered in `src/selection.rs` was
prepared, validated, and timed. `prepared=false, failed_stage=prepare` means
the availability gate (sysfs/sysctl/perf_event_open) returned false before any
read. `latency` is total nanoseconds for 100 000 reads in the measurement
phase; divide by 100 000 for ns/op.

### AWS c7g.metal — `perf_user_access=0`, `rdpmc=not present`, kernel 6.1.170 aarch64

```
class=instant candidate=aarch64-cntvct       prepared=true   valid=true latency=666705   selected=true
class=instant candidate=unix-monotonic       prepared=true   valid=true latency=3050228  selected=false
class=cycles  candidate=aarch64-pmccntr      prepared=false                              selected=false
class=cycles  candidate=aarch64-perf-pmccntr prepared=false                              selected=false
class=cycles  candidate=aarch64-cntvct       prepared=true   valid=true latency=666706   selected=true
class=cycles  candidate=unix-monotonic       prepared=true   valid=true latency=3000105  selected=false
```

### AWS c7g.4xlarge — `perf_user_access=0`, kernel 6.1.170 aarch64

```
class=instant candidate=aarch64-cntvct       prepared=true   valid=true latency=666689   selected=true
class=instant candidate=unix-monotonic       prepared=true   valid=true latency=3083354  selected=false
class=cycles  candidate=aarch64-pmccntr      prepared=false                              selected=false
class=cycles  candidate=aarch64-perf-pmccntr prepared=false                              selected=false
class=cycles  candidate=aarch64-cntvct       prepared=true   valid=true latency=666688   selected=true
class=cycles  candidate=unix-monotonic       prepared=true   valid=true latency=3096251  selected=false
```

### AWS m7i.4xlarge — `rdpmc=not present`, kernel 6.12.83 x86_64 (GenuineIntel, Sapphire Rapids virtualized)

```
class=instant candidate=x86_64-rdtsc         prepared=true   valid=true latency=1468648   selected=true
class=instant candidate=unix-monotonic       prepared=true   valid=true latency=2548406   selected=false
class=cycles  candidate=x86_64-direct-rdpmc  prepared=false                               selected=false
class=cycles  candidate=x86_64-perf-rdpmc    prepared=true   valid=true latency=159984896 selected=false
class=cycles  candidate=x86_64-rdtsc         prepared=true   valid=true latency=1468651   selected=true
class=cycles  candidate=unix-monotonic       prepared=true   valid=true latency=2554574   selected=false
```

`x86_64-perf-rdpmc` is *available* (the perf event opens and mmaps), but at
~1600 ns/op it is two orders of magnitude slower than RDTSC. The
user-mode-counter index is not exposed through the perf metadata page in a
Nitro VM, so reads fall through a syscall path. Selection correctly skips it.

### AWS c7a.4xlarge — `rdpmc=not present`, kernel 6.12.83 x86_64 (AuthenticAMD Zen4 virtualized)

```
class=instant candidate=x86_64-rdtsc         prepared=true   attempts=3 valid=false failed_stage=cross-thread-ordering selected=false
class=instant candidate=unix-monotonic       prepared=true   valid=true latency=2378421   selected=true (this round)
class=cycles  candidate=x86_64-direct-rdpmc  prepared=false                               selected=false
class=cycles  candidate=x86_64-perf-rdpmc    prepared=true   valid=true latency=97793193  selected=false
class=cycles  candidate=x86_64-rdtsc         prepared=true   valid=true latency=945974    selected=true
class=cycles  candidate=unix-monotonic       prepared=true   valid=true latency=2378422   selected=false
```

**Anomaly worth flagging:** on AMD c7a.4xlarge, the `Instant` cross-thread
ordering test failed RDTSC three times in a row in this Phase A round, and
selection fell back to `unix-monotonic` for `Instant` in that round. Phase B
(reported in the table above) re-ran selection and picked RDTSC successfully
— so the cross-thread test is intermittent on AMD virtualized. This is exactly
the failure mode that justifies validation in selection: if RDTSC is in fact
not safely cross-thread monotonic on AMD virtualized, a direct compile-time
path that does no validation could install an unsafe `Instant`. See the
"Implications" section below.

### GitHub ubuntu-24.04-arm — VM aarch64

```
class=instant candidate=aarch64-cntvct       prepared=true   valid=true latency=1334809  selected=true
class=instant candidate=unix-monotonic       prepared=true   valid=true latency=3070192  selected=false
class=cycles  candidate=aarch64-pmccntr      prepared=false                              selected=false
class=cycles  candidate=aarch64-perf-pmccntr prepared=false                              selected=false
class=cycles  candidate=aarch64-cntvct       prepared=true   valid=true latency=1334992  selected=true
class=cycles  candidate=unix-monotonic       prepared=true   valid=true latency=3068304  selected=false
```

### GitHub ubuntu-24.04 — VM x86_64 AuthenticAMD

```
class=instant candidate=x86_64-rdtsc         prepared=true   valid=true latency=1227519  selected=true
class=instant candidate=unix-monotonic       prepared=true   valid=true latency=3088883  selected=false
class=cycles  candidate=x86_64-direct-rdpmc  prepared=false                              selected=false
class=cycles  candidate=x86_64-perf-rdpmc    prepared=false                              selected=false
class=cycles  candidate=x86_64-rdtsc         prepared=true   valid=true latency=1228181  selected=true
class=cycles  candidate=unix-monotonic       prepared=true   valid=true latency=3088352  selected=false
```

### AWS c7a.metal-48xl — `rdpmc=not present`, kernel 6.12.83 x86_64 (AuthenticAMD Zen4 bare metal)

```
class=instant candidate=x86_64-rdtsc         prepared=true   attempts=3 valid=false failed_stage=cross-thread-ordering selected=false
class=instant candidate=unix-monotonic       prepared=true   valid=true latency=2378411   selected=true (this round)
class=cycles  candidate=x86_64-direct-rdpmc  prepared=false                               selected=false
class=cycles  candidate=x86_64-perf-rdpmc    prepared=true   valid=true latency=1108165   selected=false
class=cycles  candidate=x86_64-rdtsc         prepared=true   valid=true latency=945974    selected=true
class=cycles  candidate=unix-monotonic       prepared=true   valid=true latency=2378410   selected=false
```

On AMD Zen4 bare metal `perf-RDPMC` is *prepared=true* and ~11.08 ns/op —
much faster than the ~1600 ns/op virtualized variant on m7i.4xlarge, but
still slower than RDTSC at ~9.46 ns/op. The Intel m7i.metal Cycles win
(perf-RDPMC < RDTSC) does **not** generalize to AMD bare metal: AMD's
RDTSC is fast enough that the perf-event mmap path's bookkeeping is
strictly overhead.

The same AMD cross-thread-ordering anomaly observed on c7a.4xlarge appears
here too: RDTSC failed Phase A's `Instant` cross-thread test three attempts
in a row before falling back to `unix-monotonic`. Phase B re-ran selection
and picked RDTSC successfully (numbers in the table above). The
intermittency is consistent across virtualized and bare metal AMD Zen4 in
this run.

## Hypothesis tests

| # | Hypothesis | Outcome |
|---|------------|---------|
| 1 | Graviton metal kernel exposes `/proc/sys/kernel/perf_user_access ≥ 1`, enabling PMCCNTR_EL0 user reads | **Rejected.** c7g.metal reports `perf_user_access=0`. Both `aarch64-pmccntr` direct and the new `aarch64-perf-pmccntr` are gated off. |
| 2 | Graviton virtualized differs from metal in PMU exposure | **Rejected.** c7g.4xlarge and c7g.metal both report `perf_user_access=0` and pick the same `Instant` clock (cntvct). |
| 3 | Intel virtualized (Nitro) exposes `/sys/bus/event_source/devices/cpu/rdpmc ≥ 1` and perf-RDPMC wins outside `.metal` | **Rejected.** m7i.4xlarge has the rdpmc sysfs file *missing entirely* and perf-RDPMC, while available, is ~1600 ns/op vs ~14 ns/op for RDTSC. RDTSC selected. Same `Instant` winner as the existing m7i.metal data — Rule 1 has no aarch64 fact, no x86_64 fact. |
| 4 | AMD bare-metal Linux permits perf-RDPMC; m7i.metal Cycles win generalizes off Intel | **Rejected.** c7a.metal-48xl: perf-RDPMC available but at 11.08 ns/op vs RDTSC's 9.46 ns/op. Cycles selects RDTSC; Cycles ≈ Instant; Rule 2 fails. The 19 % m7i.metal Cycles win is Intel-Sapphire-Rapids-specific. |
| 5 | GitHub-hosted CI runners permit user-mode RDPMC or PMCCNTR | **Rejected.** Both ubuntu-24.04 (AMD x86_64) and ubuntu-24.04-arm prepare every PMU candidate as `false`. RDTSC / CNTVCT only. |

## Decision-rule outcomes

- **Rule 1 — Instant selection.** Across every benchmarked target × env in
  this run + the May-8 set: **same Instant winner per target** (RDTSC for
  every x86_64-linux row, CNTVCT for every aarch64-linux row). No
  env-dependent variance observed in 25 cells. **Rule 1 fails.**
- **Rule 2 — Cycles ≥ 30 % reduction.** Strongest measured Cycles win is
  m7i.metal x86_64-linux-gnu at 19 % (5.526 ns / 6.842 ns) from the May-8
  data. Every cell in this run measured Cycles within 0.1 % of Instant.
  **Rule 2 fails.**
- **Rule 3.** Moot.

All seven cells in this falsification matrix confirmed the May-8 pattern;
none falsified Rule 1 or Rule 2.

## Implications

The principal value of `Instant` selection in the data we now have is **not**
"different envs pick different winners." It is the cross-thread monotonicity
test catching a clock that the OS exposes but isn't safe — exactly what
happened intermittently on c7a.4xlarge (AMD Zen4 virtualized). A pure
compile-time path for `Instant` per-target would skip that validation and
could install RDTSC as `Instant` on AMD virtualized despite cross-thread
ordering occasionally failing.

This shifts the framing: the question "do we need Instant selection?"
decomposes into

1. *Do envs pick different fastest valid clocks?* (No — Rule 1 fails.)
2. *Does the validation gate save us from clocks the OS hands us that are
   unsafe?* (Yes, intermittently on AMD virtualized.)

Question 2 doesn't strictly need the *latency-comparison* part of selection —
just the validation gate. A compile-time path could keep cross-thread
validation as a runtime check and still drop the multi-candidate latency
microbenchmarks.

## Recommendation

The matrix supports:

- **Drop multi-candidate latency selection** for `Instant` on every measured
  target. Promote each target's compile-time path to direct.
- **Keep cross-thread validation** as a compile-time check that runs once on
  first call; if direct RDTSC fails, fall back to `clock_gettime` /
  `unix-monotonic`. This preserves the AMD-virtualized safety net at a
  fraction of the current selection complexity.
- **`Cycles`** has not cleared the 30 % bar on any benchmarked production
  cell. The m7i.metal x86_64-linux Cycles win at 19 % is the only material
  data point in the entire May-8 + May-9 corpus, and it does not meet the
  threshold the user set. **Drop the `Cycles` API**, or scope it down to a
  feature-flagged opt-in for x86_64 Linux callers who knowingly want the
  perf-RDPMC path.

## What would still flip the default

- A future x86_64 Linux bare-metal cell with **`/sys/bus/event_source/devices/cpu/rdpmc=2`**
  (unprivileged direct RDPMC enabled by host policy) where direct-RDPMC ≤ 70 %
  of RDTSC. The Intel m7i.metal cell did not have this enabled — the May-8
  Cycles win came from `perf-RDPMC`, not `direct-RDPMC` — so this is the
  unmeasured high-end of the matrix. Direct-RDPMC could conceivably clear the
  30 % bar.
- An aarch64 Linux cell with **`/proc/sys/kernel/perf_user_access=1`** (custom
  kernel config or a host that explicitly enables PMCCNTR for unprivileged
  reads) where PMCCNTR ≤ 70 % of CNTVCT. Stock AL2023 on Graviton
  (c7g.metal + c7g.4xlarge) and GitHub's `ubuntu-24.04-arm` all ship with
  `perf_user_access=0`, so this would require a deliberate operator
  configuration.
- Future hardware where the architectural counter (CNTVCT, RDTSC) is
  appreciably slower than a PMU counter the kernel exposes user-mode. None of
  the seven cells in this run nor the 19 cells in the May-8 set show this.

None of these are produced by stock AWS, GitHub, or AL2023 today.

## Out of scope (intentional)

- The rdcycle SIGILL exposure on RISC-V (`src/arch/riscv64.rs:23-35`) — fix
  belongs to the implementation session that follows the decision.
- Signal-probed direct RDPMC (deferred per `SIGNAL_PROBED_RDPMC.md`).
- GCP / Azure cells — permission-gating, not cloud diversity, was the
  question.

## Reproducibility

Raw stdout from each AWS cell and per-candidate trace lines from the GitHub
runs are saved under
[`benches/assets/selection-falsification-2026-05-09/`](assets/selection-falsification-2026-05-09/).
The runner command was
`TACH_VALIDATION_MEASURE_ITERS=5_000_000 TACH_VALIDATION_SAMPLES=101 taskset -c {0,2} cargo run --release -p tach-selection-validation-runner`,
preceded by a Phase-A pass with `TACH_SELECTOR_TRACE=1` to capture the
per-candidate decision trace.
