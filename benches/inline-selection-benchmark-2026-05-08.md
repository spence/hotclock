# Inline selected-clock benchmark, 2026-05-08

Rows were produced by `tools/selection-validation-runner` in release mode with
the default `10_000` warmup iterations, `100_000` measurement iterations, and
`31` samples. Scores are median `ns/op` for `now()` reads.

┌─────────────────────┬───────────────────────────────┬───────────────────┬───────────────────┐
│ Environment         │ Target                        │ Instant clock     │ Cycles clock      │
├─────────────────────┼───────────────────────────────┼───────────────────┼───────────────────┤
│ local-macos         │ aarch64-macos-unknown-64bit   │ aarch64-cntvct    │ aarch64-cntvct    │
│ docker-arm64        │ aarch64-linux-gnu-64bit       │ aarch64-cntvct    │ aarch64-cntvct    │
│ docker-amd64-on-arm │ x86_64-linux-gnu-64bit        │ x86_64-rdtsc      │ x86_64-rdtsc      │
│ docker-386-on-arm   │ x86-linux-gnu-32bit           │ x86-rdtsc         │ x86-rdtsc         │
│ docker-riscv64      │ riscv64-linux-gnu-64bit       │ riscv64-rdtime    │ riscv64-rdcycle   │
│ docker-s390x        │ s390x-linux-gnu-64bit         │ unix-monotonic    │ unix-monotonic    │
│ docker-ppc64le      │ powerpc64-linux-gnu-64bit     │ unix-monotonic    │ unix-monotonic    │
└─────────────────────┴───────────────────────────────┴───────────────────┴───────────────────┘

┌─────────────────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┬─────────┐
│ Environment         │ hotclock │ Cycles   │ quanta   │ minstant │ fastant  │ std      │ fastest │
├─────────────────────┼──────────┼──────────┼──────────┼──────────┼──────────┼──────────┼─────────┤
│ local-macos         │ 0.330    │ 0.316    │ 4.491    │ 24.929   │ 24.892   │ 17.830   │ yes     │
│ docker-arm64        │ 0.335    │ 0.337    │ 4.531    │ 31.776   │ 27.653   │ 20.444   │ yes     │
│ docker-amd64-on-arm │ 15.249   │ 16.005   │ 24.525   │ 48.824   │ 16.564   │ 38.088   │ yes     │
│ docker-386-on-arm   │ 26.535   │ 25.947   │ 296.446  │ 348.612  │ 337.739  │ 218.628  │ yes     │
│ docker-riscv64      │ 48.572   │ 54.423   │ 267.498  │ 362.020  │ 382.673  │ 242.219  │ yes     │
│ docker-s390x        │ 117.856  │ 117.926  │ 142.782  │ 270.795  │ 270.653  │ 144.031  │ yes     │
│ docker-ppc64le      │ 128.971  │ 127.835  │ 156.873  │ 212.555  │ 210.540  │ 152.432  │ yes     │
└─────────────────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┴─────────┘

`loongarch64-unknown-linux-gnu` is compile-validated. It is not in this runtime
benchmark table because Docker's official `rust:1.91` image has no
`linux/loong64` manifest.
