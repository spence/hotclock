# AWS full clock and crate benchmarks

All values are `ns/op`. Raw JSON reports in this directory contain the full stats for each row.

Benchmark config: 30,000 warmup iterations, 300,000 measured iterations, 41 samples, 3 stability attempts.

`panic` means the guarded path rejected the clock before timing. `SIGSEGV` means the blind direct counter faulted in the isolated child process.

```text
┌───────────────────┬─────────────────────────┬───────────────────┬───────────────┬─────────────────┬────────────────┬───────────┬─────────────────┬───────────┬───────────┬─────────────┬────────────────┬──────────────────┬────────────┬──────────┬─────────┐
│ Target            │ Env                     │ x64 rdpmc checked │ x64 rdpmc raw │ x64 rdpmc blind │ x64 perf rdpmc │ x64 rdtsc │ x86 rdpmc blind │ x86 rdtsc │ unix mono │ std Instant │ quanta Instant │ quanta Clock now │ quanta raw │ minstant │ fastant │
├───────────────────┼─────────────────────────┼───────────────────┼───────────────┼─────────────────┼────────────────┼───────────┼─────────────────┼───────────┼───────────┼─────────────┼────────────────┼──────────────────┼────────────┼──────────┼─────────┤
│ x86_64-linux-musl │ x86_64 musl / Lambda    │ panic             │ panic         │ SIGSEGV         │ panic          │ 9.472     │ n/a             │ n/a       │ 54.104    │ 55.595      │ 13.370         │ 11.468           │ 9.770      │ 68.684   │ 9.448   │
│ x86_64-linux-musl │ x86_64 musl / t3.micro  │ panic             │ panic         │ SIGSEGV         │ panic          │ 9.495     │ n/a             │ n/a       │ 23.557    │ 24.475      │ 13.357         │ 11.422           │ 9.823      │ 9.488    │ 9.473   │
│ x86_64-linux-musl │ x86_64 musl / m7i.metal │ panic             │ panic         │ 3.017           │ 5.152          │ 6.847     │ n/a             │ n/a       │ 14.519    │ 15.016      │ 7.120          │ 6.979            │ 6.847      │ 6.847    │ 6.847   │
│ x86_64-linux-gnu  │ x86_64 gnu / Lambda     │ panic             │ panic         │ SIGSEGV         │ panic          │ 9.477     │ n/a             │ n/a       │ 54.133    │ 54.747      │ 13.387         │ 10.756           │ 9.756      │ 68.022   │ 9.448   │
│ x86_64-linux-gnu  │ x86_64 gnu / t3.micro   │ panic             │ panic         │ SIGSEGV         │ panic          │ 9.506     │ n/a             │ n/a       │ 23.526    │ 24.213      │ 13.408         │ 10.751           │ 9.757      │ 9.469    │ 9.441   │
│ x86_64-linux-gnu  │ x86_64 gnu / m7i.metal  │ panic             │ panic         │ 2.902           │ 5.003          │ 6.847     │ n/a             │ n/a       │ 14.224    │ 14.595      │ 7.124          │ 6.979            │ 6.847      │ 6.848    │ 6.847   │
│ x86-linux-musl    │ i686 musl / t3.micro    │ n/a               │ n/a           │ n/a             │ n/a            │ n/a       │ SIGSEGV         │ 9.808     │ 38.413    │ 42.585      │ 40.142         │ 38.080           │ 37.917     │ 9.816    │ 9.175   │
│ x86-linux-musl    │ i686 musl / m7i.metal   │ n/a               │ n/a           │ n/a             │ n/a            │ n/a       │ 3.158           │ 6.848     │ 19.629    │ 21.496      │ 21.574         │ 20.105           │ 20.095     │ 6.848    │ 6.848   │
│ x86-linux-gnu     │ i686 gnu / t3.micro     │ n/a               │ n/a           │ n/a             │ n/a            │ n/a       │ SIGSEGV         │ 9.483     │ 38.626    │ 40.111      │ 42.452         │ 39.993           │ 39.968     │ 9.161    │ 9.109   │
│ x86-linux-gnu     │ i686 gnu / m7i.metal    │ n/a               │ n/a           │ n/a             │ n/a            │ n/a       │ 3.076           │ 6.848     │ 20.874    │ 24.543      │ 23.122         │ 21.437           │ 21.453     │ 6.848    │ 6.848   │
└───────────────────┴─────────────────────────┴───────────────────┴───────────────┴─────────────────┴────────────────┴───────────┴─────────────────┴───────────┴───────────┴─────────────┴────────────────┴──────────────────┴────────────┴──────────┴─────────┘
```
