#!/bin/bash
# Run the bench inside an Alpine Linux Docker container on an existing AWS host.
# Produces a musl-target cell. The host AMI is AL2023 (the standard Tier 1 host);
# this script SSHes to it, pulls the alpine image, builds with rustup inside the
# container, runs Phase A/B + clock-survey, and pulls logs back.
#
# Usage: AWS_PROFILE=tach benches/scripts/bench-alpine.sh <cell> <host-public-ip> <arch>
#   <arch> is one of: x86_64 | aarch64

set -euo pipefail

CELL="$1"
HOST_IP="$2"
ARCH="$3"

REPO_ROOT="$(git rev-parse --show-toplevel)"
KEY="$REPO_ROOT/benches/scripts/.bench-key.pem"
RESULT_DIR="$REPO_ROOT/benches/results/$CELL"
mkdir -p "$RESULT_DIR"

[ -f "$KEY" ] || { echo "Key not found at $KEY" >&2; exit 2; }

case "$ARCH" in
  x86_64)  DOCKER_PLATFORM=linux/amd64; RUST_TARGET=x86_64-unknown-linux-musl ;;
  aarch64) DOCKER_PLATFORM=linux/arm64; RUST_TARGET=aarch64-unknown-linux-musl ;;
  *) echo "Unknown arch: $ARCH" >&2; exit 2 ;;
esac

TARBALL="$REPO_ROOT/benches/scripts/.tach-bench.tar.gz"
[ -f "$TARBALL" ] || { echo "Source tarball missing; orchestrate-cell.sh builds it." >&2; exit 2; }

echo "[$CELL] Uploading source to $HOST_IP..."
scp -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  "$TARBALL" ec2-user@"$HOST_IP":/home/ec2-user/

REMOTE_SCRIPT='set -e
sudo dnf install -y docker >/dev/null 2>&1 || sudo yum install -y docker >/dev/null 2>&1 || true
sudo systemctl start docker
sudo usermod -aG docker ec2-user || true
mv /home/ec2-user/.tach-bench.tar.gz /home/ec2-user/tach-bench.tar.gz 2>/dev/null || true
rm -rf /home/ec2-user/tach-alpine
mkdir /home/ec2-user/tach-alpine
tar xzf /home/ec2-user/tach-bench.tar.gz -C /home/ec2-user/tach-alpine

# Run inside Alpine container. Install rust via rustup (apk rust is gnu).
sudo docker run --rm --platform=DOCKER_PLATFORM_PLACEHOLDER \
  -v /home/ec2-user/tach-alpine:/work \
  -w /work \
  alpine:3.20 sh -c "
    apk add --no-cache build-base curl bash linux-headers >/dev/null
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable >/dev/null
    . /root/.cargo/env
    echo === METADATA ===
    uname -a
    cat /etc/os-release | head -5
    grep -m1 \"model name\\|CPU implementer\\|CPU part\\|vendor_id\" /proc/cpuinfo || true
    rustc --version

    cd tools/selection-validation-runner
    cargo build --release 2>&1 | tail -10
    BIN=\$(pwd)/target/release/tach-selection-validation-runner

    echo === PHASE A ===
    TACH_SELECTOR_TRACE=1 \"\$BIN\" 2>&1 | tee /work/phase-a.log

    echo === PHASE B ===
    TACH_VALIDATION_MEASURE_ITERS=5000000 TACH_VALIDATION_SAMPLES=101 \\
      taskset -c "\$((\$(nproc) - 1))" \"\$BIN\" 2>&1 | tee /work/phase-b.log

    echo === CLOCK SURVEY ===
    cd /work/tools/clock-survey
    cargo build --release 2>&1 | tail -5
    SURVEY=\$(pwd)/target/release/clock-survey
    taskset -c "\$((\$(nproc) - 1))" \"\$SURVEY\" 2>&1 | tee /work/clock-survey.log
  "
'
REMOTE_SCRIPT="${REMOTE_SCRIPT//DOCKER_PLATFORM_PLACEHOLDER/$DOCKER_PLATFORM}"

echo "[$CELL] Running Alpine container bench on host..."
ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  ec2-user@"$HOST_IP" "$REMOTE_SCRIPT" \
  > "$RESULT_DIR/stdout.txt" 2> "$RESULT_DIR/stderr.txt" || {
    echo "[$CELL] BENCH FAILED (exit $?)"
    tail -40 "$RESULT_DIR/stderr.txt"
    exit 1
  }

echo "[$CELL] Pulling logs..."
scp -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  ec2-user@"$HOST_IP":'/home/ec2-user/tach-alpine/phase-*.log' "$RESULT_DIR/" 2>/dev/null || true
scp -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  ec2-user@"$HOST_IP":/home/ec2-user/tach-alpine/clock-survey.log "$RESULT_DIR/" 2>/dev/null || true

if grep -q "cycles-le-instant.*fail" "$RESULT_DIR"/phase-*.log 2>/dev/null; then
  echo "[$CELL] CONTRACT VIOLATION: cycles-le-instant=fail"
  exit 3
fi

echo "[$CELL] Done. Results in $RESULT_DIR"
ls -la "$RESULT_DIR"
