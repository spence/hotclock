# Runtime selection validation, 2026-05-08

Rows were produced by `tools/selection-validation-runner` with
`HOTCLOCK_ENFORCE_EXPECTED=1`, so each run exited successfully only when
`Instant::implementation()` and `Cycles::implementation()` matched the expected
fastest valid clocks for that target/environment. Scores are median `ns/op` for
`now()` reads.

┌────────────────┬───────────────────┬────────────────┬───────────────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┐
│ Environment    │ Target            │ Instant clock  │ Cycles clock      │ Instant  │ Cycles   │ quanta   │ minstant │ fastant  │ std      │
├────────────────┼───────────────────┼────────────────┼───────────────────┼──────────┼──────────┼──────────┼──────────┼──────────┼──────────┤
│ AWS t3 KVM     │ x86_64-linux-musl │ x86_64-rdtsc   │ x86_64-rdtsc      │ 8.711    │ 8.066    │ 13.254   │ 9.356    │ 9.356    │ 24.249   │
│ AWS m7i metal  │ x86_64-linux-musl │ x86_64-rdtsc   │ x86_64-perf-rdpmc │ 6.841    │ 5.262    │ 7.130    │ 6.841    │ 6.841    │ 14.734   │
│ AWS m7i metal  │ x86_64-linux-gnu  │ x86_64-rdtsc   │ x86_64-perf-rdpmc │ 7.879    │ 5.758    │ 7.131    │ 6.842    │ 6.842    │ 14.999   │
│ AWS t3 KVM     │ x86-linux-musl    │ x86-rdtsc      │ x86-rdtsc         │ 13.552   │ 13.551   │ 69.312   │ 14.363   │ 14.154   │ 66.458   │
│ AWS m7i metal  │ x86-linux-musl    │ x86-rdtsc      │ x86-rdtsc         │ 6.841    │ 6.841    │ 23.066   │ 6.841    │ 6.841    │ 22.743   │
│ Docker amd64   │ x86_64-linux-gnu  │ x86_64-rdtsc   │ x86_64-rdtsc      │ 15.394   │ 15.222   │ 25.079   │ 39.050   │ 22.066   │ 28.070   │
│ Docker 386     │ x86-linux-gnu     │ x86-rdtsc      │ x86-rdtsc         │ 25.789   │ 25.780   │ 253.702  │ 323.398  │ 324.264  │ 222.623  │
│ Docker arm64   │ aarch64-linux-gnu │ aarch64-cntvct │ aarch64-cntvct    │ 0.330    │ 0.330    │ 4.466    │ 27.203   │ 27.275   │ 20.222   │
│ Docker riscv64 │ riscv64-linux-gnu │ riscv64-rdtime │ riscv64-rdcycle   │ 59.584   │ 58.656   │ 215.296  │ 271.262  │ 271.241  │ 185.151  │
│ local macOS    │ aarch64-macos     │ aarch64-cntvct │ aarch64-cntvct    │ 0.330    │ 0.330    │ 4.622    │ 25.740   │ 25.683   │ 18.422   │
└────────────────┴───────────────────┴────────────────┴───────────────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┘

The x86_64 Linux rows are the runtime-selection proof point: the same target
family selected `RDTSC` for `Cycles` on AWS t3 KVM and `perf-RDPMC` on AWS m7i
bare metal. Direct RDPMC was not exposed through
`/sys/bus/event_source/devices/cpu/rdpmc` on the AL2023 c5/m7i hosts used for
this run, so the strongest current production-selector `Cycles` row is
`x86_64-linux-gnu` on m7i metal.

The `i686-unknown-linux-musl` bare-metal false negative is fixed. A 100-process
cold-start run on AWS m7i metal selected `x86-rdtsc` for `Instant` and `Cycles`
on every run.
