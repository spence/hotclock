#!/bin/bash
# Run the full baseline benchmark matrix.
#
# AWS cells run sequentially (so vCPU-bucket limits don't stall on metal cells).
# GitHub-hosted cells run in parallel via the selection-experiment workflow.
# Local Docker cells (qemu) run optionally; uncomment if you want them.
#
# Usage: AWS_PROFILE=tach benches/scripts/run-matrix.sh

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
SCRIPTS="$REPO_ROOT/benches/scripts"
PROFILE="${AWS_PROFILE:-tach}"

X86_AMI=$(cat "$SCRIPTS/.ami-x86_64.txt")
ARM_AMI=$(cat "$SCRIPTS/.ami-arm64.txt")

echo "=== AWS cells (sequential) ==="
echo "x86_64 AMI: $X86_AMI"
echo "arm64  AMI: $ARM_AMI"

# Representative production cells. Add more rows as needed.
# Format: cell_name instance_type ami
AWS_CELLS=(
  "m7i-metal-24xl|m7i.metal-24xl|$X86_AMI"
  "m7i-4xlarge|m7i.4xlarge|$X86_AMI"
  "c7g-metal|c7g.metal|$ARM_AMI"
  "c7a-metal-48xl|c7a.metal-48xl|$X86_AMI"
)

for row in "${AWS_CELLS[@]}"; do
  IFS='|' read -r CELL TYPE AMI <<< "$row"
  echo
  echo "==================== AWS $CELL ===================="
  AWS_PROFILE=$PROFILE "$SCRIPTS/orchestrate-cell.sh" "$CELL" "$TYPE" "$AMI" || {
    echo "[$CELL] FAILED — continuing matrix"
    continue
  }
done

echo
echo "==================== GitHub Actions ===================="
echo "Triggering selection-experiment workflow..."
gh workflow run selection-experiment.yml -R spence/tach
sleep 5
RUN_ID=$(gh run list -R spence/tach --workflow selection-experiment.yml --limit 1 --json databaseId --jq '.[0].databaseId')
echo "Run ID: $RUN_ID"
echo "Waiting for completion..."
gh run watch -R spence/tach "$RUN_ID" --exit-status || echo "Some GH cells may have failed"

mkdir -p "$REPO_ROOT/benches/results/github"
(cd "$REPO_ROOT/benches/results/github" && gh run download "$RUN_ID" -R spence/tach)

echo
echo "==================== Local macOS ===================="
echo "Running local Phase A + Phase B (host: $(uname -ms))..."
mkdir -p "$REPO_ROOT/benches/results/local-macos"
(
  cd "$REPO_ROOT/tools/selection-validation-runner"
  cargo build --release 2>&1 | tail -3
  BIN="$(pwd)/target/release/tach-selection-validation-runner"
  TACH_SELECTOR_TRACE=1 "$BIN" 2>&1 | tee "$REPO_ROOT/benches/results/local-macos/phase-a.log"
  TACH_VALIDATION_MEASURE_ITERS=5000000 TACH_VALIDATION_SAMPLES=101 \
    "$BIN" 2>&1 | tee "$REPO_ROOT/benches/results/local-macos/phase-b.log"
)

echo
echo "==================== Summary ==================="
echo "Per-cell results in $REPO_ROOT/benches/results/"
echo "Verify all cells passed cycles-le-instant:"
grep -h "cycles-le-instant" "$REPO_ROOT/benches/results"/*/phase-b.log 2>/dev/null | sort -u
