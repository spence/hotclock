# tach benchmarks

`tach::Instant::now()` read cost compared with `quanta`, `fastant`, `minstant`,
and `std::time::Instant` across seven target / environment cells. All numbers
are nanoseconds per call (lower is better).

## Results

| Target | Environment | Instance | tach::Instant | quanta | fastant | minstant | std::Instant |
|---|---|---|---:|---:|---:|---:|---:|
| `aarch64-apple-darwin` | Apple Silicon MBP | M1 MacBook Pro | **0.33** | 7.59 | 43.72 | 43.26 | 31.66 |
| `aarch64-unknown-linux-gnu` | Graviton 3 Nitro VM | c7g.4xlarge | **6.67** | 7.06 | 38.94 | 39.58 | 31.46 |
| `x86_64-unknown-linux-gnu` | Intel burst VM | t3.medium | **8.76** | 13.31 | 9.41 | 9.41 | 24.06 |
| `x86_64-unknown-linux-musl` | Alpine Docker on Intel host | m7i.metal-24xl | **14.32** | 17.07 | 14.63 | 14.63 | 25.87 |
| `x86_64-unknown-linux-gnu` | AWS Lambda (Firecracker) | provided.al2023 | **9.56** | 14.10 | 10.21 | 44.39 | 29.92 |
| `x86_64-apple-darwin` | GitHub Actions VM | macos-15-intel | **6.08** | 81.23 | 39.79 | 40.20 | 38.59 |
| `x86_64-pc-windows-msvc` | GitHub Actions VM | windows-2025 | **11.25** | 11.67 | 40.93 | 40.93 | 38.40 |

Visualized at [`benches/assets/benchmark-instant.png`](benches/assets/benchmark-instant.png).

## Methodology

- **Harness**: Criterion 0.8 (`harness = false`, custom `criterion_main!`).
- **Measured function**: `let start = Instant::now(); black_box(start.elapsed())`.
  Each row is the median of Criterion's resampled distribution.
- **Compiler**: stable Rust at the time of run (2026-05).
- **Sample size**: Criterion default (100 samples × ~3s measurement time per sample).
- **CPU governor**: `performance` where the runtime exposes it (Linux). macOS and
  Windows use the OS default; bare metal runs at base clock.
- **Process**: single-threaded, no other workload contending for the CPU.

## Reproducing

```bash
# 1. clone and check out the version you want to measure
git clone https://github.com/spence/tach.git
cd tach

# 2. install the toolchain for your target if cross-compiling
rustup target add <target-triple>      # e.g. aarch64-unknown-linux-gnu

# 3. run the bench
cargo bench --bench instant

# 4. results land in target/criterion/<name>/new/estimates.json
#    point estimate is at .median.point_estimate (in nanoseconds)
```

To regenerate the chart after a new run, edit `INSTANT_GROUPS` in
`benches/assets/render_benchmark.py` with the new numbers, then:

```bash
python3 benches/assets/render_benchmark.py
```

`rsvg-convert` is required (`brew install librsvg` on macOS,
`apt install librsvg2-bin` on Debian/Ubuntu).
