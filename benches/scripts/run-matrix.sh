#!/bin/bash
# Run the full Tier 1 benchmark matrix. 24 cells across AWS (10), GitHub (5),
# AWS musl (2), AWS Lambda (2), local (5).
#
# AWS cells run sequentially. GitHub-hosted cells run in parallel via the
# selection-experiment workflow. Local cells run on this Mac. Alpine cells
# piggyback on an existing AL2023 AWS host. Lambda packages and invokes.
#
# Set RUN_SUBSET to a space-separated list of cell groups to run only some:
#   RUN_SUBSET="aws-metal aws-vm" benches/scripts/run-matrix.sh
# Groups: aws-metal aws-vm aws-alpine aws-lambda github local
#
# Usage: AWS_PROFILE=tach benches/scripts/run-matrix.sh

set -uo pipefail   # NOT -e: cell failures should not abort the matrix.

REPO_ROOT="$(git rev-parse --show-toplevel)"
SCRIPTS="$REPO_ROOT/benches/scripts"
PROFILE="${AWS_PROFILE:-tach}"
RUN_SUBSET="${RUN_SUBSET:-aws-metal aws-vm aws-alpine aws-lambda github local}"

X86_AMI="$(cat "$SCRIPTS/.ami-x86_64.txt" 2>/dev/null || echo "")"
ARM_AMI="$(cat "$SCRIPTS/.ami-arm64.txt"  2>/dev/null || echo "")"

# Ensure source tarball is built once for every AWS-side cell.
TARBALL="$SCRIPTS/.tach-bench.tar.gz"
if [ ! -f "$TARBALL" ]; then
  echo "Building source tarball..."
  (cd "$REPO_ROOT" && tar czf "$TARBALL" \
    --exclude target --exclude .git --exclude 'benches/assets' \
    --exclude 'benches/results' --exclude 'benches/scripts/.*' \
    --exclude '*.md' \
    src tools examples benches Cargo.toml Cargo.lock)
fi

run_aws_cell() {
  local cell="$1" type="$2" ami="$3"
  echo
  echo "================ AWS $cell ($type) ================"
  AWS_PROFILE=$PROFILE "$SCRIPTS/orchestrate-cell.sh" "$cell" "$type" "$ami" \
    || echo "[$cell] FAILED — continuing matrix"
}

if [[ " $RUN_SUBSET " == *" aws-metal "* ]]; then
  [ -n "$X86_AMI" ] && [ -n "$ARM_AMI" ] || { echo "AWS AMIs missing; run aws-prereqs.sh"; exit 2; }
  echo "=== AWS bare-metal cells ==="
  run_aws_cell "m7i-metal-24xl"  "m7i.metal-24xl"  "$X86_AMI"
  run_aws_cell "z1d-metal"       "z1d.metal"       "$X86_AMI"
  run_aws_cell "c7a-metal-48xl"  "c7a.metal-48xl"  "$X86_AMI"
  run_aws_cell "c7g-metal"       "c7g.metal"       "$ARM_AMI"
  run_aws_cell "c8g-metal-24xl"  "c8g.metal-24xl"  "$ARM_AMI"
fi

if [[ " $RUN_SUBSET " == *" aws-vm "* ]]; then
  echo "=== AWS Nitro VM cells ==="
  run_aws_cell "m7i-4xlarge"     "m7i.4xlarge"     "$X86_AMI"
  run_aws_cell "c7a-4xlarge"     "c7a.4xlarge"     "$X86_AMI"
  run_aws_cell "t3-medium"       "t3.medium"       "$X86_AMI"
  run_aws_cell "c7g-4xlarge"     "c7g.4xlarge"     "$ARM_AMI"
  run_aws_cell "c8g-4xlarge"     "c8g.4xlarge"     "$ARM_AMI"
fi

if [[ " $RUN_SUBSET " == *" aws-alpine "* ]]; then
  echo "=== AWS Alpine musl cells (piggyback on existing metal hosts) ==="
  # These run inside a Docker container on a freshly-launched AL2023 host. We
  # launch a single host per arch, run the alpine bench, then terminate.
  for arch in x86_64 aarch64; do
    case "$arch" in
      x86_64)  HOST_TYPE="m7i.4xlarge"; AMI="$X86_AMI"; CELL_HOST="alpine-host-x86_64" ;;
      aarch64) HOST_TYPE="c7g.4xlarge"; AMI="$ARM_AMI"; CELL_HOST="alpine-host-aarch64" ;;
    esac
    CELL="alpine-${arch}-musl"
    echo "--- Launching $CELL_HOST as Alpine bench host ---"
    AWS_PROFILE=$PROFILE aws ec2 run-instances --region us-east-2 \
      --image-id "$AMI" --instance-type "$HOST_TYPE" \
      --key-name tach-bench-temp \
      --security-group-ids "$(cat "$SCRIPTS/.bench-sg.txt")" \
      --instance-initiated-shutdown-behavior terminate \
      --user-data "file://$SCRIPTS/user-data.sh" \
      --block-device-mappings 'DeviceName=/dev/xvda,Ebs={VolumeSize=20,VolumeType=gp3,DeleteOnTermination=true}' \
      --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=tach-bench-$CELL_HOST}]" \
      --query 'Instances[0].InstanceId' --output text > "/tmp/$CELL_HOST.id"
    INST_ID="$(cat "/tmp/$CELL_HOST.id")"
    echo "Waiting for $INST_ID..."
    AWS_PROFILE=$PROFILE aws ec2 wait instance-running --region us-east-2 --instance-ids "$INST_ID"
    HOST_IP="$(AWS_PROFILE=$PROFILE aws ec2 describe-instances --region us-east-2 \
      --instance-ids "$INST_ID" --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)"
    echo "Host IP: $HOST_IP. Waiting for cloud-init..."
    for _ in $(seq 1 90); do
      if ssh -i "$SCRIPTS/.bench-key.pem" \
        -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        -o ConnectTimeout=5 -o BatchMode=yes \
        ec2-user@"$HOST_IP" 'test -f /home/ec2-user/.ready' 2>/dev/null; then
        break
      fi
      sleep 5
    done

    "$SCRIPTS/bench-alpine.sh" "$CELL" "$HOST_IP" "$arch" \
      || echo "[$CELL] FAILED — continuing matrix"

    echo "Terminating $INST_ID..."
    AWS_PROFILE=$PROFILE aws ec2 terminate-instances --region us-east-2 \
      --instance-ids "$INST_ID" --output text >/dev/null 2>&1 || true
  done
fi

if [[ " $RUN_SUBSET " == *" aws-lambda "* ]]; then
  echo "=== AWS Lambda cells ==="
  AWS_PROFILE=$PROFILE "$SCRIPTS/bench-lambda.sh" "lambda-x86_64"  "x86_64"  \
    || echo "[lambda-x86_64] FAILED — continuing matrix"
  AWS_PROFILE=$PROFILE "$SCRIPTS/bench-lambda.sh" "lambda-aarch64" "aarch64" \
    || echo "[lambda-aarch64] FAILED — continuing matrix"
fi

if [[ " $RUN_SUBSET " == *" github "* ]]; then
  echo "=== GitHub Actions cells ==="
  echo "Triggering selection-experiment workflow..."
  gh workflow run selection-experiment.yml -R spence/tach
  sleep 5
  RUN_ID="$(gh run list -R spence/tach --workflow selection-experiment.yml \
    --limit 1 --json databaseId --jq '.[0].databaseId')"
  echo "Run ID: $RUN_ID"
  echo "Waiting for completion (5 matrix legs)..."
  gh run watch -R spence/tach "$RUN_ID" --exit-status \
    || echo "Some GH cells may have failed — check 'gh run view $RUN_ID'"

  STAGING="$(mktemp -d)"
  (cd "$STAGING" && gh run download "$RUN_ID" -R spence/tach)
  for d in "$STAGING"/bench-*; do
    [ -d "$d" ] || continue
    cell="$(basename "$d" | sed 's/^bench-//')"
    dst="$REPO_ROOT/benches/results/$cell"
    mkdir -p "$dst"
    cp -f "$d"/*.log "$dst/" 2>/dev/null || true
    echo "Pulled GH artifact -> $dst"
  done
  rm -rf "$STAGING"
fi

if [[ " $RUN_SUBSET " == *" local "* ]]; then
  echo "=== Local cells ==="
  for cell in local-catalyst local-catalyst-mini local-rosetta local-docker-arm64 local-docker-arm64-musl; do
    echo
    echo "------ $cell ------"
    "$SCRIPTS/run-local-cell.sh" "$cell" \
      || echo "[$cell] FAILED — continuing matrix"
  done
fi

echo
echo "==================== Summary ==================="
echo "Per-cell results: $REPO_ROOT/benches/results/"
echo
echo "cycles-le-instant pass/fail rollup:"
grep -h "cycles-le-instant" "$REPO_ROOT/benches/results"/*/phase-b.log 2>/dev/null \
  | sort | uniq -c | sort -rn
echo
echo "Run: $SCRIPTS/render-baseline.sh   to produce the unified markdown table."
