# AWS m7i GNU high-confidence benchmark, 2026-05-08

This rerun checks the noisy `AWS m7i metal / x86_64-linux-gnu` row with pinned,
interleaved sampling.

Environment:

- Instance: `m7i.metal-24xl`
- CPU: `Intel(R) Xeon(R) Platinum 8488C`
- Kernel: `Linux 6.1.168-203.330.amzn2023.x86_64`
- Target: `x86_64-unknown-linux-gnu`
- Pinning: `taskset -c 2`
- Runner: `tools/selection-validation-runner`
- Samples: `101`
- Iterations per sample: `5,000,000`
- Method: interleaved crate order, raw `_rdtsc()` baseline included

Result:

┌───────────────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┐
│ Benchmark         │ min      │ p25      │ median   │ p75      │ p95      │ max      │
├───────────────────┼──────────┼──────────┼──────────┼──────────┼──────────┼──────────┤
│ hotclock-instant  │  6.841ns │  6.842ns │  6.842ns │  6.842ns │  6.844ns │  7.377ns │
│ hotclock-cycles   │  5.263ns │  5.500ns │  5.526ns │  5.617ns │  5.789ns │  5.963ns │
│ raw-counter       │  6.841ns │  6.842ns │  6.842ns │  6.842ns │  6.842ns │  6.857ns │
│ quanta            │  7.368ns │  7.387ns │  7.393ns │  7.395ns │  7.412ns │  7.430ns │
│ minstant          │  6.841ns │  6.842ns │  6.842ns │  6.842ns │  6.843ns │  6.844ns │
│ fastant           │  6.841ns │  6.842ns │  6.842ns │  6.842ns │  6.842ns │  6.845ns │
│ std-instant       │ 14.569ns │ 14.786ns │ 14.810ns │ 14.831ns │ 14.878ns │ 14.917ns │
└───────────────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┘

Confirmatory run:

┌───────────────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┐
│ Benchmark         │ min      │ p25      │ median   │ p75      │ p95      │ max      │
├───────────────────┼──────────┼──────────┼──────────┼──────────┼──────────┼──────────┤
│ hotclock-instant  │  6.842ns │  6.842ns │  6.842ns │  6.842ns │  6.843ns │  7.304ns │
│ hotclock-cycles   │  5.270ns │  5.422ns │  5.526ns │  5.564ns │  5.668ns │  5.719ns │
│ raw-counter       │  6.841ns │  6.842ns │  6.842ns │  6.842ns │  6.843ns │  6.843ns │
│ quanta            │  7.376ns │  7.389ns │  7.395ns │  7.399ns │  7.408ns │  7.413ns │
│ minstant          │  6.841ns │  6.842ns │  6.842ns │  6.842ns │  6.843ns │  6.844ns │
│ fastant           │  6.842ns │  6.842ns │  6.842ns │  6.842ns │  6.843ns │  6.867ns │
│ std-instant       │ 14.544ns │ 14.791ns │ 14.812ns │ 14.824ns │ 14.838ns │ 14.850ns │
└───────────────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┘

Conclusion:

The old `7.879ns` `hotclock` value was a benchmark artifact. Under pinned,
interleaved sampling, `hotclock::Instant::now()` ties raw `_rdtsc()` and the
TSC-only `minstant` / `fastant` path at `6.842ns`. `quanta::Instant::now()` is
slower on this run, and `std::time::Instant::now()` is much slower.
