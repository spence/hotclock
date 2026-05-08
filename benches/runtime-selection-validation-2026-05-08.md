# Runtime selection validation, 2026-05-08

Rows were produced by `tools/selection-validation-runner` with
`HOTCLOCK_ENFORCE_EXPECTED=1`, so each run exited successfully only when
`Instant::implementation()` and `Cycles::implementation()` matched the expected
fastest valid clocks for that target/environment. Scores are median `ns/op` for
`now()` reads.

┌────────────────┬───────────────────┬────────────────┬───────────────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬────┐
│ Environment    │ Target            │ Instant clock  │ Cycles clock      │ Instant  │ Cycles   │ quanta   │ minstant │ fastant  │ std      │ ok │
├────────────────┼───────────────────┼────────────────┼───────────────────┼──────────┼──────────┼──────────┼──────────┼──────────┼──────────┼────┤
│ AWS t3 KVM     │ x86_64-linux-musl │ x86_64-rdtsc   │ x86_64-rdtsc      │ 12.296   │ 12.354   │ 27.088   │ 14.126   │ 14.577   │ 24.098   │ yes│
│ AWS m7i metal  │ x86_64-linux-musl │ x86_64-rdtsc   │ x86_64-perf-rdpmc │ 6.841    │ 4.999    │ 7.353    │ 6.841    │ 6.841    │ 14.735   │ yes│
│ AWS t3 KVM     │ x86-linux-musl    │ x86-rdtsc      │ x86-rdtsc         │ 13.002   │ 13.187   │ 75.085   │ 13.931   │ 15.827   │ 75.004   │ yes│
│ Docker amd64   │ x86_64-linux-gnu  │ x86_64-rdtsc   │ x86_64-rdtsc      │ 15.394   │ 15.222   │ 25.079   │ 39.050   │ 22.066   │ 28.070   │ yes│
│ Docker 386     │ x86-linux-gnu     │ x86-rdtsc      │ x86-rdtsc         │ 25.789   │ 25.780   │ 253.702  │ 323.398  │ 324.264  │ 222.623  │ yes│
│ Docker arm64   │ aarch64-linux-gnu │ aarch64-cntvct │ aarch64-cntvct    │ 0.330    │ 0.330    │ 4.466    │ 27.203   │ 27.275   │ 20.222   │ yes│
│ Docker riscv64 │ riscv64-linux-gnu │ riscv64-rdtime │ riscv64-rdcycle   │ 59.584   │ 58.656   │ 215.296  │ 271.262  │ 271.241  │ 185.151  │ yes│
│ local macOS    │ aarch64-macos     │ aarch64-cntvct │ aarch64-cntvct    │ 0.330    │ 0.330    │ 4.622    │ 25.740   │ 25.683   │ 18.422   │ yes│
└────────────────┴───────────────────┴────────────────┴───────────────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴────┘

The `x86_64-linux-musl` rows are the runtime-selection proof point: the same
target selected `RDTSC` for `Cycles` on AWS t3 KVM and `perf-RDPMC` on AWS m7i
bare metal.

One diagnostic run on AWS m7i metal with the `i686-unknown-linux-musl` binary
selected `unix-monotonic` for `Instant`; 38 of 40 follow-up cold starts selected
`x86-rdtsc`. That validation false negative is tracked in `PROJECT.md` and is
not used as a README proof row.
