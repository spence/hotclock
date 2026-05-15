# tach benchmarks

`tach::Instant::now()` and `Instant::elapsed()` cost compared with `quanta`,
`fastant`, `minstant`, and `std::time::Instant` across seven target /
environment cells. All numbers are nanoseconds per call (lower is better).

## Results

### `Instant::now()` cost

| Target | Environment | Instance | tach | quanta | fastant | minstant | std |
|---|---|---|---:|---:|---:|---:|---:|
| `aarch64-apple-darwin` | Apple Silicon MBP | M1 MacBook Pro | **0.35** | 4.57 | 27.64 | 27.22 | 20.15 |
| `aarch64-unknown-linux-gnu` | Graviton 3 Nitro VM | c7g.4xlarge | **6.68** | 7.09 | 41.43 | 41.34 | 32.53 |
| `x86_64-unknown-linux-gnu` | Intel burst VM | t3.medium | **8.76** | 13.28 | 9.41 | 9.40 | 24.35 |
| `x86_64-unknown-linux-musl` | Alpine Docker on Intel host | m7i.metal-24xl | **6.84** | 7.11 | **6.84** | **6.84** | 14.65 |
| `x86_64-unknown-linux-gnu` | AWS Lambda (Firecracker) | provided.al2023 | **13.67** | 23.34 | 15.57 | 56.56 | 49.18 |
| `x86_64-apple-darwin` | GitHub Actions | macos-13 | TBD | TBD | TBD | TBD | TBD |
| `x86_64-pc-windows-msvc` | GitHub Actions | windows-2025 | **11.56** | 11.85 | 41.25 | 41.07 | 38.48 |

### `Instant::now() + elapsed()` cost (full roundtrip)

| Target | Environment | Instance | tach.elapsed | tach.elapsed_fast | quanta | fastant | minstant | std |
|---|---|---|---:|---:|---:|---:|---:|---:|
| `aarch64-apple-darwin` | Apple Silicon MBP | M1 MacBook Pro | 5.29 | **3.39** | 9.09 | 59.38 | 60.64 | 43.70 |
| `aarch64-unknown-linux-gnu` | Graviton 3 Nitro VM | c7g.4xlarge | 15.19 | **13.51** | 15.34 | 87.17 | 87.24 | 70.44 |
| `x86_64-unknown-linux-gnu` | Intel burst VM | t3.medium | 42.11 | 29.28 | **27.84** | 31.02 | 31.18 | 53.74 |
| `x86_64-unknown-linux-musl` | Alpine Docker on Intel host | m7i.metal-24xl | 20.53 | **15.66** | 17.50 | 21.42 | 21.42 | 32.75 |
| `x86_64-unknown-linux-gnu` | AWS Lambda (Firecracker) | provided.al2023 | 71.74 | **48.55** | 54.21 | 51.83 | 138.04 | 107.95 |
| `x86_64-apple-darwin` | GitHub Actions | macos-13 | TBD | TBD | TBD | TBD | TBD | TBD |
| `x86_64-pc-windows-msvc` | GitHub Actions | windows-2025 | 23.69 | **22.82** | 24.68 | 94.98 | 94.94 | 79.87 |

Charts: [`benches/assets/benchmark-instant.png`](benches/assets/benchmark-instant.png) (now() across targets), [`benches/assets/benchmark-elapsed.png`](benches/assets/benchmark-elapsed.png) (single-cell elapsed detail).

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

For `x86_64-apple-darwin` (macos-13) and `x86_64-pc-windows-msvc` (windows-2025):

The workflow at [`.github/workflows/bench.yml`](.github/workflows/bench.yml) runs on manual dispatch. Trigger via:

```bash
gh workflow run bench --ref main
gh run watch                                                # follow live
gh run view <run-id> --log --job=<job-id> | grep "time:"   # extract numbers
```

**Gotcha**: GitHub runner labels are confusing — `macos-15`/`macos-14` are Apple Silicon (ARM). `macos-13` is the only Intel macOS runner available. `windows-2025` and `ubuntu-24.04` are x86_64.

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

After collecting new measurements, edit `INSTANT_GROUPS` (now-only chart) and `ELAPSED_GROUPS` (elapsed detail chart) in `benches/assets/render_benchmark.py`, then:

```bash
python3 benches/assets/render_benchmark.py
```

`rsvg-convert` is required (`brew install librsvg` on macOS, `apt install librsvg2-bin` on Debian).
