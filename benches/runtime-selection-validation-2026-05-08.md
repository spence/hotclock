# Runtime selection validation, 2026-05-08

Rows were produced by `tools/selection-validation-runner`. Scores are median
`ns/op` for `now()` reads.

┌────────────────┬─────────────────────┬────────────────┬───────────────────┬─────────┬────────┬─────────┬──────────┬─────────┬─────────┐
│ Environment    │ Target              │ Instant clock  │ Cycles clock      │ Instant │ Cycles │ quanta  │ minstant │ fastant │ std     │
├────────────────┼─────────────────────┼────────────────┼───────────────────┼─────────┼────────┼─────────┼──────────┼─────────┼─────────┤
│ AWS t3 Nitro   │ x86_64-linux-musl   │ x86_64-rdtsc   │ x86_64-rdtsc      │ 9.722   │ 9.722  │ 13.954  │ 10.222   │ 10.052  │ 25.300  │
│ AWS Lambda     │ x86_64-linux-musl   │ x86_64-rdtsc   │ x86_64-rdtsc      │ 17.825  │ 17.801 │ 22.172  │ 75.637   │ 18.160  │ 53.764  │
│ AWS m7i metal  │ x86_64-linux-musl   │ x86_64-rdtsc   │ x86_64-perf-rdpmc │ 6.841   │ 5.262  │ 7.130   │ 6.841    │ 6.841   │ 14.734  │
│ AWS m7i metal  │ x86_64-linux-gnu    │ x86_64-rdtsc   │ x86_64-perf-rdpmc │ 6.842   │ 5.526  │ 7.395   │ 6.842    │ 6.842   │ 14.812  │
│ AWS Windows    │ x86_64-windows-msvc │ x86_64-rdtsc   │ x86_64-rdtsc      │ 6.957   │ 6.967  │ 11.719  │ 33.650   │ 33.656  │ 39.224  │
│ AWS t3 Nitro   │ x86-linux-musl      │ x86-rdtsc      │ x86-rdtsc         │ 10.054  │ 10.051 │ 42.391  │ 10.706   │ 10.697  │ 44.581  │
│ AWS m7i metal  │ x86-linux-musl      │ x86-rdtsc      │ x86-rdtsc         │ 6.841   │ 6.841  │ 23.066  │ 6.841    │ 6.841   │ 22.743  │
│ AWS Lambda     │ aarch64-linux-gnu   │ aarch64-cntvct │ aarch64-cntvct    │ 17.325  │ 17.306 │ 21.970  │ 72.252   │ 74.114  │ 54.165  │
│ Docker amd64   │ x86_64-linux-gnu    │ x86_64-rdtsc   │ x86_64-rdtsc      │ 15.394  │ 15.222 │ 25.079  │ 39.050   │ 22.066  │ 28.070  │
│ Docker 386     │ x86-linux-gnu       │ x86-rdtsc      │ x86-rdtsc         │ 25.789  │ 25.780 │ 253.702 │ 323.398  │ 324.264 │ 222.623 │
│ Docker arm64   │ aarch64-linux-gnu   │ aarch64-cntvct │ aarch64-cntvct    │ 0.330   │ 0.330  │ 4.466   │ 27.203   │ 27.275  │ 20.222  │
│ Docker riscv64 │ riscv64-linux-gnu   │ riscv64-rdtime │ riscv64-rdcycle   │ 59.584  │ 58.656 │ 215.296 │ 271.262  │ 271.241 │ 185.151 │
│ local macOS    │ aarch64-macos       │ aarch64-cntvct │ aarch64-cntvct    │ 0.330   │ 0.330  │ 4.622   │ 25.740   │ 25.683  │ 18.422  │
└────────────────┴─────────────────────┴────────────────┴───────────────────┴─────────┴────────┴─────────┴──────────┴─────────┴─────────┘

The x86_64 Linux rows are the runtime-selection proof point: the same target
family selected `RDTSC` for `Cycles` on AWS t3 Nitro and `perf-RDPMC` on AWS m7i
bare metal. Direct RDPMC was not exposed through
`/sys/bus/event_source/devices/cpu/rdpmc` on the AL2023 c5/m7i hosts used for
this run, so the strongest current production-selector `Cycles` row is
`x86_64-linux-gnu` on m7i metal.

The AWS Lambda rows are second warm invokes on temporary 1024 MB
`provided.al2023` functions. Lambda showed stable bimodal distributions around
the raw hardware counter path; the table reports the runner median.

The `i686-unknown-linux-musl` bare-metal false negative is fixed. A 100-process
cold-start run on AWS m7i metal selected `x86-rdtsc` for `Instant` and `Cycles`
on every run.
