# AWS Lambda criterion data

Lambda's runtime can't host criterion's filesystem assumptions (read-only `/var/task`, no `cargo bench` machinery, no plotters). The `runs/run{1,2,3}.json` files in this directory are output from the standalone `tach-lambda-bench` handler at `/tmp/tach-lambda-bench/` (separate Cargo project, depends on `tach` via path). Each run loops 5,000,000 iterations of each call, 5 batches per crate, reports the per-batch median in nanoseconds.

Configuration:
- Runtime: `provided.al2023` x86_64
- Memory: 1024 MB (matches the BENCHMARKS.md baseline; Lambda CPU scales with memory)
- Architecture: `x86_64-unknown-linux-gnu`
- Hypervisor: Firecracker microVM

No `now.svg` / `elapsed.svg` violin plots in this directory — Lambda doesn't emit criterion data. The 3 runs together approximate the variance you'd see in a violin.
