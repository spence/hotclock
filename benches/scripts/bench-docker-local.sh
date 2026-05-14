#!/bin/bash
# Run the bench in a Docker container locally. Used on Apple Silicon to produce
# Linux cells (native arm64) and Alpine cells (arm64 musl) without spinning up
# AWS instances.
#
# Usage: benches/scripts/bench-docker-local.sh <cell> <variant>
#   <variant> is one of: arm64 | arm64-musl

set -euo pipefail

CELL="$1"
VARIANT="$2"
REPO_ROOT="$(git rev-parse --show-toplevel)"
RESULT_DIR="$REPO_ROOT/benches/results/$CELL"
mkdir -p "$RESULT_DIR"

if ! command -v docker >/dev/null 2>&1; then
  echo "Docker not installed. Install Docker Desktop and retry." >&2
  exit 2
fi
if ! docker info >/dev/null 2>&1; then
  echo "Docker daemon not running. Start Docker Desktop and retry." >&2
  exit 2
fi

case "$VARIANT" in
  arm64)       BASE_IMAGE=debian:bookworm; PLATFORM=linux/arm64 ;;
  arm64-musl)  BASE_IMAGE=alpine:3.20;     PLATFORM=linux/arm64 ;;
  *) echo "Unknown variant: $VARIANT (use arm64 | arm64-musl)" >&2; exit 2 ;;
esac

echo "[$CELL] Running $VARIANT in $BASE_IMAGE..."

# Bind-mount the repo read-only and a writable output dir.
OUT_TMP="$(mktemp -d)"
trap "rm -rf '$OUT_TMP'" EXIT

REMOTE_SCRIPT='set -e
if command -v apt-get >/dev/null 2>&1; then
  apt-get update >/dev/null && apt-get install -y --no-install-recommends curl ca-certificates build-essential util-linux >/dev/null
elif command -v apk >/dev/null 2>&1; then
  apk add --no-cache curl ca-certificates build-base bash linux-headers >/dev/null
fi
curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable >/dev/null
source $HOME/.cargo/env
mkdir -p /build
cp -r /work/src /work/tools /work/examples /work/benches /work/Cargo.toml /work/Cargo.lock /build/
cd /build/tools/selection-validation-runner
echo === METADATA ===
uname -a
cat /etc/os-release | head -5 || true
grep -m1 "model name\|CPU implementer\|CPU part\|vendor_id" /proc/cpuinfo || true
rustc --version
echo === BUILD ===
cargo build --release 2>&1 | tail -5
BIN=$(pwd)/target/release/tach-selection-validation-runner

echo === PHASE A ===
TACH_SELECTOR_TRACE=1 "$BIN" 2>&1 | tee /out/phase-a.log

echo === PHASE B ===
if command -v taskset >/dev/null 2>&1; then
  TACH_VALIDATION_MEASURE_ITERS=5000000 TACH_VALIDATION_SAMPLES=101 \
    taskset -c 0 "$BIN" 2>&1 | tee /out/phase-b.log
else
  TACH_VALIDATION_MEASURE_ITERS=5000000 TACH_VALIDATION_SAMPLES=101 \
    "$BIN" 2>&1 | tee /out/phase-b.log
fi

echo === CLOCK SURVEY ===
cd /build/tools/clock-survey
cargo build --release 2>&1 | tail -5
SURVEY=$(pwd)/target/release/clock-survey
if command -v taskset >/dev/null 2>&1; then
  taskset -c 0 "$SURVEY" 2>&1 | tee /out/clock-survey.log
else
  "$SURVEY" 2>&1 | tee /out/clock-survey.log
fi
'

docker run --rm --platform="$PLATFORM" \
  -v "$REPO_ROOT:/work:ro" \
  -v "$OUT_TMP:/out" \
  "$BASE_IMAGE" sh -c "$REMOTE_SCRIPT" \
  > "$RESULT_DIR/stdout.txt" 2> "$RESULT_DIR/stderr.txt" || {
    echo "[$CELL] DOCKER BENCH FAILED (exit $?)"
    tail -40 "$RESULT_DIR/stderr.txt"
    exit 1
  }

cp "$OUT_TMP"/phase-*.log "$RESULT_DIR/" 2>/dev/null || true
cp "$OUT_TMP"/clock-survey.log "$RESULT_DIR/" 2>/dev/null || true

if grep -q "cycles-le-instant.*fail" "$RESULT_DIR"/phase-*.log 2>/dev/null; then
  echo "[$CELL] CONTRACT VIOLATION: cycles-le-instant=fail"
  exit 3
fi

echo "[$CELL] Done. Results in $RESULT_DIR"
ls -la "$RESULT_DIR"
