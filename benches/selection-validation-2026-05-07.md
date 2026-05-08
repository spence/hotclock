# Selection validation

All rows were produced by `tools/selection-validation-runner` in-process. No candidate clock was
benchmarked in a crash-isolated child process.

┌───────────────────────────┬──────────────────────────────┬────────────────────┬────────────────────┬──────────────┬──────────────┬────────────┬──────────────┬─────────────┬───────┐
│ Env                       │ Target                       │ Instant clock      │ Cycles clock       │ tach now │ tach cyc │ quanta now │ minstant now │ fastant now │ match │
├───────────────────────────┼──────────────────────────────┼────────────────────┼────────────────────┼──────────────┼──────────────┼────────────┼──────────────┼─────────────┼───────┤
│ local-macos               │ aarch64-macos-unknown-64bit  │ aarch64-cntvct     │ aarch64-cntvct     │ 0.330 ns     │ 0.330 ns     │ 4.498 ns   │ 25.056 ns    │ 25.083 ns   │ yes   │
│ local-macos-rosetta       │ x86_64-macos-unknown-64bit   │ x86_64-rdtsc       │ x86_64-rdtsc       │ 8.745 ns     │ 8.755 ns     │ 13.545 ns  │ 42.016 ns    │ 41.749 ns   │ yes   │
│ docker-arm64              │ aarch64-linux-musl-64bit     │ aarch64-cntvct     │ aarch64-cntvct     │ 0.733 ns     │ 0.750 ns     │ 4.480 ns   │ 27.964 ns    │ 27.919 ns   │ yes   │
│ docker-amd64-on-arm       │ x86_64-linux-musl-64bit      │ x86_64-rdtsc       │ x86_64-rdtsc       │ 15.688 ns    │ 19.761 ns    │ 23.234 ns  │ 40.847 ns    │ 16.122 ns   │ yes   │
│ docker-386-on-arm         │ x86-linux-musl-32bit         │ x86-rdtsc          │ x86-rdtsc          │ 28.126 ns    │ 39.052 ns    │ 243.065 ns │ 348.428 ns   │ 344.666 ns  │ yes   │
│ docker-riscv64            │ riscv64-linux-gnu-64bit      │ riscv64-rdtime     │ riscv64-rdcycle    │ 61.102 ns    │ 59.576 ns    │ 260.516 ns │ 267.539 ns   │ 267.039 ns  │ yes   │
│ docker-loong64            │ loongarch64-linux-gnu-64bit  │ loongarch64-rdtime │ loongarch64-rdtime │ 25.237 ns    │ 24.971 ns    │ 237.491 ns │ 237.626 ns   │ 239.213 ns  │ yes   │
│ docker-loong64-alpine     │ loongarch64-linux-musl-64bit │ loongarch64-rdtime │ loongarch64-rdtime │ 25.485 ns    │ 25.356 ns    │ 216.865 ns │ 225.916 ns   │ 225.186 ns  │ yes   │
│ docker-ppc64le            │ powerpc64-linux-gnu-64bit    │ powerpc64-mftb     │ powerpc64-mftb     │ 35.367 ns    │ 35.159 ns    │ 149.408 ns │ 202.744 ns   │ 200.864 ns  │ yes   │
│ docker-s390x              │ s390x-linux-gnu-64bit        │ unix-monotonic     │ unix-monotonic     │ 134.920 ns   │ 134.415 ns   │ 164.097 ns │ 287.983 ns   │ 289.709 ns  │ yes   │
│ aws-t3.micro              │ x86_64-linux-musl-64bit      │ x86_64-rdtsc       │ x86_64-rdtsc       │ 8.713 ns     │ 10.687 ns    │ 13.263 ns  │ 9.357 ns     │ 9.359 ns    │ yes   │
│ aws-t3.micro              │ x86-linux-musl-32bit         │ x86-rdtsc          │ x86-rdtsc          │ 9.034 ns     │ 9.679 ns     │ 40.869 ns  │ 9.682 ns     │ 9.359 ns    │ yes   │
│ aws-t4g.nano              │ aarch64-linux-musl-64bit     │ aarch64-cntvct     │ aarch64-cntvct     │ 7.278 ns     │ 7.278 ns     │ 10.366 ns  │ 41.716 ns    │ 42.761 ns   │ yes   │
│ aws-c5.metal              │ x86_64-linux-musl-64bit      │ x86_64-rdtsc       │ x86_64-rdtsc       │ 7.049 ns     │ 6.945 ns     │ 7.180 ns   │ 6.453 ns     │ 6.453 ns    │ yes   │
│ aws-c5.metal              │ x86-linux-musl-32bit         │ x86-rdtsc          │ x86-rdtsc          │ 6.945 ns     │ 6.945 ns     │ 31.359 ns  │ 6.411 ns     │ 6.411 ns    │ yes   │
│ aws-windows-t3.micro      │ x86_64-windows-msvc-64bit    │ x86_64-rdtsc       │ x86_64-rdtsc       │ 9.126 ns     │ 9.034 ns     │ 13.552 ns  │ 41.835 ns    │ 39.476 ns   │ yes   │
│ aws-windows-c5.large      │ x86_64-windows-msvc-64bit    │ x86_64-rdtsc       │ x86_64-rdtsc       │ 8.236 ns     │ 8.237 ns     │ 12.355 ns  │ 36.757 ns    │ 35.516 ns   │ yes   │
│ aws-windows-i686-t3.micro │ x86-windows-msvc-32bit       │ x86-rdtsc          │ x86-rdtsc          │ 9.061 ns     │ 9.035 ns     │ 43.891 ns  │ 74.028 ns    │ 74.154 ns   │ yes   │
│ aws-windows-i686-c5.large │ x86-windows-msvc-32bit       │ x86-rdtsc          │ x86-rdtsc          │ 8.237 ns     │ 8.237 ns     │ 39.966 ns  │ 66.603 ns    │ 66.618 ns   │ yes   │
│ github-windows-11-arm     │ aarch64-windows-msvc-64bit   │ aarch64-cntvct     │ aarch64-cntvct     │ 13.360 ns    │ 13.361 ns    │ 13.395 ns  │ 36.780 ns    │ 36.838 ns   │ yes   │
│ aws-lambda-x86_64         │ x86_64-linux-musl-64bit      │ x86_64-rdtsc       │ x86_64-rdtsc       │ 8.711 ns     │ 10.650 ns    │ 13.232 ns  │ 42.171 ns    │ 9.472 ns    │ yes   │
│ aws-lambda-arm64          │ aarch64-linux-musl-64bit     │ aarch64-cntvct     │ aarch64-cntvct     │ 7.279 ns     │ 7.278 ns     │ 10.362 ns  │ 42.421 ns    │ 45.339 ns   │ yes   │
└───────────────────────────┴──────────────────────────────┴────────────────────┴────────────────────┴──────────────┴──────────────┴────────────┴──────────────┴─────────────┴───────┘

AWS resources used for this report were terminated after the run, and the temporary security
groups were deleted.

## Notes

- AWS returned zero Amazon-owned Windows ARM64 AMIs in `us-east-1`; Windows ARM64 runtime
  validation used GitHub's `windows-11-arm` hosted runner.
- Current Linux bare-metal validation selected `rdtsc`; no current environment produced a
  production `rdpmc` selection.
