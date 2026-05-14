#!/bin/bash
# Run a single local cell. Dispatches by cell name.
#
# Usage: benches/scripts/run-local-cell.sh <cell>
#   Known cells:
#     local-catalyst              — current Mac (aarch64-apple-darwin)
#     local-catalyst-mini         — Mac mini via 'ssh macmini'
#     local-rosetta               — current Mac under `arch -x86_64`
#     local-docker-arm64          — Docker debian arm64 (calls bench-docker-local.sh)
#     local-docker-arm64-musl     — Docker alpine arm64 (calls bench-docker-local.sh)

set -euo pipefail

CELL="$1"
REPO_ROOT="$(git rev-parse --show-toplevel)"
RESULT_DIR="$REPO_ROOT/benches/results/$CELL"
mkdir -p "$RESULT_DIR"
SCRIPT_DIR="$REPO_ROOT/benches/scripts"

run_native_local() {
  local cell="$1"
  local arch_prefix="$2"   # e.g. "" or "arch -x86_64"
  cd "$REPO_ROOT/tools/selection-validation-runner"
  echo "[$cell] === METADATA ==="
  uname -a
  sysctl -n machdep.cpu.brand_string 2>/dev/null || true
  rustc --version

  echo "[$cell] === BUILD ==="
  if [ -n "$arch_prefix" ]; then
    $arch_prefix cargo build --release --target x86_64-apple-darwin 2>&1 | tail -5
    local BIN="$REPO_ROOT/tools/selection-validation-runner/target/x86_64-apple-darwin/release/tach-selection-validation-runner"
  else
    cargo build --release 2>&1 | tail -5
    local BIN="$REPO_ROOT/tools/selection-validation-runner/target/release/tach-selection-validation-runner"
  fi

  echo "[$cell] === PHASE A ==="
  TACH_SELECTOR_TRACE=1 $arch_prefix "$BIN" 2>&1 | tee "$RESULT_DIR/phase-a.log"

  echo "[$cell] === PHASE B (unpinned — macOS lacks taskset) ==="
  TACH_VALIDATION_MEASURE_ITERS=5000000 TACH_VALIDATION_SAMPLES=101 \
    $arch_prefix "$BIN" 2>&1 | tee "$RESULT_DIR/phase-b.log"

  echo "[$cell] === CLOCK SURVEY ==="
  cd "$REPO_ROOT/tools/clock-survey"
  if [ -n "$arch_prefix" ]; then
    $arch_prefix cargo build --release --target x86_64-apple-darwin 2>&1 | tail -5
    $arch_prefix "$REPO_ROOT/tools/clock-survey/target/x86_64-apple-darwin/release/clock-survey" \
      2>&1 | tee "$RESULT_DIR/clock-survey.log"
  else
    cargo build --release 2>&1 | tail -5
    "$REPO_ROOT/tools/clock-survey/target/release/clock-survey" 2>&1 \
      | tee "$RESULT_DIR/clock-survey.log"
  fi
}

case "$CELL" in
  local-catalyst)
    run_native_local "$CELL" ""
    ;;

  local-rosetta)
    if ! arch -x86_64 uname -m 2>/dev/null | grep -q x86_64; then
      echo "Rosetta 2 not active on this Mac. Install via 'softwareupdate --install-rosetta'." >&2
      exit 2
    fi
    run_native_local "$CELL" "arch -x86_64"
    ;;

  local-catalyst-mini)
    if ! ssh -o ConnectTimeout=5 -o BatchMode=yes macmini 'true' 2>/dev/null; then
      echo "Cannot reach Mac mini via 'ssh macmini'. Check Tailscale + ssh config." >&2
      exit 2
    fi
    # Tarball the repo and run remotely.
    TARBALL="$REPO_ROOT/benches/scripts/.tach-bench.tar.gz"
    if [ ! -f "$TARBALL" ]; then
      (cd "$REPO_ROOT" && tar czf "$TARBALL" \
        --exclude target --exclude .git --exclude 'benches/assets' \
        --exclude 'benches/results' --exclude 'benches/scripts/.*' \
        --exclude '*.md' \
        src tools examples benches Cargo.toml Cargo.lock)
    fi
    scp -q "$TARBALL" macmini:~/tach-bench.tar.gz
    ssh macmini "
      set -e
      rm -rf ~/tach-bench && mkdir ~/tach-bench
      tar xzf ~/tach-bench.tar.gz -C ~/tach-bench
      source ~/.cargo/env 2>/dev/null || true
      cd ~/tach-bench/tools/selection-validation-runner
      echo === METADATA ===
      uname -a
      sysctl -n machdep.cpu.brand_string 2>/dev/null || true
      rustc --version
      echo === BUILD ===
      cargo build --release 2>&1 | tail -5
      BIN=\$(pwd)/target/release/tach-selection-validation-runner
      echo === PHASE A ===
      TACH_SELECTOR_TRACE=1 \"\$BIN\" 2>&1 | tee ~/tach-bench/phase-a.log
      echo === PHASE B ===
      TACH_VALIDATION_MEASURE_ITERS=5000000 TACH_VALIDATION_SAMPLES=101 \\
        \"\$BIN\" 2>&1 | tee ~/tach-bench/phase-b.log
      echo === CLOCK SURVEY ===
      cd ~/tach-bench/tools/clock-survey
      cargo build --release 2>&1 | tail -5
      ./target/release/clock-survey 2>&1 | tee ~/tach-bench/clock-survey.log
    " > "$RESULT_DIR/stdout.txt" 2> "$RESULT_DIR/stderr.txt"
    scp -q "macmini:~/tach-bench/phase-a.log"  "$RESULT_DIR/" || true
    scp -q "macmini:~/tach-bench/phase-b.log"  "$RESULT_DIR/" || true
    scp -q "macmini:~/tach-bench/clock-survey.log" "$RESULT_DIR/" || true
    ;;

  local-docker-arm64)
    "$SCRIPT_DIR/bench-docker-local.sh" "$CELL" arm64
    ;;

  local-docker-arm64-musl)
    "$SCRIPT_DIR/bench-docker-local.sh" "$CELL" arm64-musl
    ;;

  *)
    echo "Unknown local cell: $CELL" >&2
    echo "Known: local-catalyst, local-catalyst-mini, local-rosetta, local-docker-arm64, local-docker-arm64-musl" >&2
    exit 2
    ;;
esac

if grep -q "cycles-le-instant.*fail" "$RESULT_DIR"/phase-*.log 2>/dev/null; then
  echo "[$CELL] CONTRACT VIOLATION: cycles-le-instant=fail"
  grep "cycles-le-instant" "$RESULT_DIR"/phase-*.log
  exit 3
fi

echo "[$CELL] Done. Results in $RESULT_DIR"
ls -la "$RESULT_DIR"
