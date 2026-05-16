# tach benchmarks

`tach::Instant::now()` and `Instant::elapsed()` cost compared with `quanta`,
`fastant`, `minstant`, and `std::time::Instant` across six target /
environment cells. All numbers are nanoseconds per call (lower is better).

## Results

### `Instant::now()` cost

| Target | Environment | Instance | tach | quanta | fastant | minstant | std |
|---|---|---|---:|---:|---:|---:|---:|
| `aarch64-apple-darwin` | Apple Silicon MBP | M1 MacBook Pro | **0.35** | 4.59 | 27.23 | 27.29 | 20.28 |
| `aarch64-unknown-linux-gnu` | Graviton 3 Nitro VM | c7g.4xlarge | **6.68** | 7.02 | 41.68 | 41.68 | 32.51 |
| `x86_64-unknown-linux-gnu` | Intel burst VM | t3.medium | **8.74** | 13.32 | 11.19 | 9.40 | 24.28 |
| `x86_64-unknown-linux-musl` | Alpine Docker on Intel host | m7i.metal-24xl | **6.84** | 7.11 | **6.84** | **6.84** | 14.65 |
| `x86_64-unknown-linux-gnu` | AWS Lambda (Firecracker) | provided.al2023 | **13.60** | 23.34 | 15.54 | 56.93 | 50.76 |
| `x86_64-pc-windows-msvc` | GitHub Actions | windows-2025 | **12.34** | 12.43 | 45.54 | 45.52 | 41.23 |

### `Instant::now() + elapsed()` cost (full roundtrip)

| Target | Environment | Instance | tach | quanta | fastant | minstant | std |
|---|---|---|---:|---:|---:|---:|---:|
| `aarch64-apple-darwin` | Apple Silicon MBP | M1 MacBook Pro | **1.20** | 9.16 | 59.66 | 59.64 | 43.72 |
| `aarch64-unknown-linux-gnu` | Graviton 3 Nitro VM | c7g.4xlarge | **13.35** | 15.30 | 87.81 | 88.13 | 72.58 |
| `x86_64-unknown-linux-gnu` | Intel burst VM | t3.medium | **18.94** | 28.18 | 31.03 | 31.09 | 53.48 |
| `x86_64-unknown-linux-musl` | Alpine Docker on Intel host | m7i.metal-24xl | **13.68** | 17.51 | 21.40 | 21.41 | 32.58 |
| `x86_64-unknown-linux-gnu` | AWS Lambda (Firecracker) | provided.al2023 | **31.93** | 50.86 | 51.79 | 135.75 | 106.36 |
| `x86_64-pc-windows-msvc` | GitHub Actions | windows-2025 | **24.70** | 25.48 | 104.51 | 104.44 | 85.68 |

Chart: [`benches/assets/benchmark.png`](benches/assets/benchmark.png) — one cell per target environment. Each crate row shows `Instant::now()` (dark portion of bar) and the full `now() + elapsed()` roundtrip (lighter extension), with numeric times as `now / elapsed` on the right.

**Not included**: `x86_64-apple-darwin` (GitHub Actions `macos-13`) — could not land an Intel macOS runner allocation across multiple `workflow_dispatch` attempts. The GitHub-hosted Intel macOS runner pool has very low capacity.

## Methodology

- **Harness**: Criterion 0.8 (`harness = false`, custom `criterion_main!`).
- **Measured functions**: `Instant::now()` standalone, and `let start = Instant::now(); black_box(start.elapsed())` (full roundtrip).
- **Compiler**: stable Rust at the time of run (2026-05).
- **Sample size**: Criterion default — 100 samples × ~3s measurement time per bench. GitHub Actions runs use `--warm-up-time 1 --measurement-time 3` to fit the 6 min runner budget.
- **CPU governor**: `performance` where the runtime exposes it (Linux). macOS and Windows use the OS default; bare metal runs at base clock.
- **Process**: single-threaded, no other workload contending for the CPU.

## Reproducing

### Local

```bash
git clone https://github.com/spence/tach.git
cd tach
cargo bench --bench instant
# results land in target/criterion/<name>/new/estimates.json
# point estimate is at .median.point_estimate (in nanoseconds)
```

### AWS EC2 (Linux gnu)

For `aarch64-unknown-linux-gnu` (Graviton) and `x86_64-unknown-linux-gnu` (Intel/AMD):

```bash
# Launch the smallest instance that meets the technical requirement.
# Examples: c7g.4xlarge for Graviton, t3.medium for Intel burst.
aws ec2 run-instances \
  --image-id $(aws ssm get-parameters --names \
      "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-${ARCH}" \
      --query 'Parameters[0].Value' --output text) \
  --instance-type c7g.4xlarge \
  --key-name "$KEY_NAME" \
  --security-group-ids "$SG_WITH_SSH" \
  --instance-initiated-shutdown-behavior terminate \
  --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=tach-bench-XYZ}]" \
  --region us-east-2

# Once running, SSH in and run:
ssh -i ~/.ssh/your-key.pem ec2-user@<public-ip>
sudo dnf install -y gcc git                                    # <-- MUST install gcc; AL2023 is bare
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
source $HOME/.cargo/env
git clone --depth 1 https://github.com/spence/tach.git
cd tach
cargo bench --bench instant 2>&1 | tee /tmp/bench.out
# When done: aws ec2 terminate-instances --instance-ids <id>
```

**Gotcha**: AL2023's base image doesn't include a C linker, and `rustup --profile minimal` also doesn't include one. You'll see `linker 'cc' not found` from native-build-script crates (serde, libc, etc.) unless you `dnf install -y gcc` first.

### AWS EC2 (Linux musl, Alpine on metal)

For `x86_64-unknown-linux-musl`, run inside an Alpine Docker container on a metal host:

```bash
# Launch m7i.metal-24xl (or smaller; this one was kept from the historical baseline)
sudo dnf install -y docker
sudo systemctl start docker
sudo docker run --rm alpine:latest sh -c '
  apk add --no-cache git curl build-base linux-headers
  curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  source $HOME/.cargo/env
  git clone --depth 1 https://github.com/spence/tach.git
  cd tach
  cargo bench --bench instant
'
```

**Note**: Alpine's `build-base` package includes `gcc`, so no separate install needed.

### GitHub Actions runners

For `x86_64-pc-windows-msvc` (windows-2025) and `x86_64-apple-darwin` (macos-13):

The workflow at [`.github/workflows/bench.yml`](.github/workflows/bench.yml) runs on manual dispatch. Trigger via:

```bash
gh workflow run bench --ref main
gh run watch                                                # follow live
gh run view <run-id> --log --job=<job-id> | grep "time:"   # extract numbers
```

**Gotcha**: GitHub runner labels are confusing — `macos-15`/`macos-14` are Apple Silicon (ARM). `macos-13` is the only Intel macOS runner available. `windows-2025` and `ubuntu-24.04` are x86_64. Intel macOS runner capacity is limited on the GH-hosted fleet; expect long queues.

### AWS Lambda

For `provided.al2023` x86_64. A standalone Lambda handler (not the criterion bench — Lambda's runtime doesn't accommodate criterion's filesystem assumptions) runs the bench in-process and returns the per-call timings as JSON. Source at `/tmp/tach-lambda-bench/` (separate Cargo project, depends on `tach` via path).

```bash
# Build (uses zig under the hood for cross-compile)
cd /tmp/tach-lambda-bench
cargo lambda build --release --output-format=zip

# Deploy (requires a pre-created execution role; one-time setup with admin creds)
cargo lambda deploy --profile $YOUR_PROFILE --region us-east-2 \
  --iam-role arn:aws:iam::$ACCT:role/tach-bench-lambda-role \
  --memory 1024 --timeout 300 tach-lambda-bench

# Invoke and capture the JSON response
aws lambda invoke --function-name tach-lambda-bench \
  --profile $YOUR_PROFILE --region us-east-2 \
  --cli-binary-format raw-in-base64-out --payload '{}' /tmp/result.json
cat /tmp/result.json | python3 -m json.tool

# Cleanup
aws lambda delete-function --function-name tach-lambda-bench \
  --profile $YOUR_PROFILE --region us-east-2
```

**Note**: Lambda numbers are noisier than EC2 (Firecracker VM with shared CPU). They're useful as a relative comparison but don't compare directly to bare-metal numbers.

## Updating the chart

After collecting new measurements, edit `NOW_GROUPS` and `ELAPSED_GROUPS` in `benches/assets/render.py`, then:

```bash
python3 benches/assets/render.py
```

`rsvg-convert` is required (`brew install librsvg` on macOS, `apt install librsvg2-bin` on Debian).

## Per-cell criterion reports

Each cell has a standalone SVG report at `benches/violins/<cell>.svg`. It combines criterion's auto-generated violin plots for both `Instant::now()` and `Instant::now() + elapsed()` plus a per-crate medians + 95% CI table into one file.

Generated with `benches/assets/build-cell-report.py`. After running `cargo bench --bench instant` on a target machine, run the tool with the cell name and optional header text:

```bash
# On the machine that just ran the bench:
python3 benches/assets/build-cell-report.py <cell-name> \
  --title "<Pretty Cell Title>" \
  --subtitle "<target-triple>"

# Or pointing at criterion data from elsewhere on disk:
python3 benches/assets/build-cell-report.py <cell-name> \
  --criterion-dir path/to/criterion \
  --title "..." --subtitle "..."
```

The tool reads `<criterion-dir>/Instant__now()/report/violin.svg`, the matching `+ elapsed()` group, and the per-crate `estimates.json` files, then writes `benches/violins/<cell-name>.svg`. Handles both gnuplot- and plotters-generated violins (gnuplot is used when installed locally; plotters is criterion's default on most EC2 AMIs).

Current cells with reports:

- `benches/violins/local-catalyst.svg` — Apple Silicon M1 MBP
- `benches/violins/c7g-4xlarge.svg` — AWS Graviton 3
- `benches/violins/t3-medium.svg` — AWS Intel Burst
- `benches/violins/m7i-metal-24xl.svg` — Docker Alpine on AWS Metal
- `benches/violins/github-windows-x86_64.svg` — GitHub Actions Windows
- `benches/violins/lambda-x86_64/` — AWS Lambda has no criterion violin (filesystem restrictions); raw harness output in `runs/run{1,2,3}.json`
