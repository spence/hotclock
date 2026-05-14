#!/bin/bash
# Launch one EC2 cell, run the benchmark, collect logs, terminate.
#
# Usage: AWS_PROFILE=tach benches/scripts/orchestrate-cell.sh <cell> <type> <ami>
#
# Prerequisites: aws-prereqs.sh has been run (key pair + SG ingress set up).
# Results: benches/results/<cell>/phase-{a,b}.log + stdout/stderr.

set -euo pipefail

CELL="$1"
INSTANCE_TYPE="$2"
AMI="$3"

REPO_ROOT="$(git rev-parse --show-toplevel)"
KEY="$REPO_ROOT/benches/scripts/.bench-key.pem"
SG_FILE="$REPO_ROOT/benches/scripts/.bench-sg.txt"
REGION=us-east-2
PROFILE="${AWS_PROFILE:-tach}"
RESULT_DIR="$REPO_ROOT/benches/results/$CELL"
mkdir -p "$RESULT_DIR"

[ -f "$KEY" ] || { echo "Key not found at $KEY; run aws-prereqs.sh first." >&2; exit 2; }
[ -f "$SG_FILE" ] || { echo "Security group ID not recorded; run aws-prereqs.sh." >&2; exit 2; }
SG="$(cat "$SG_FILE")"

TARBALL="$REPO_ROOT/benches/scripts/.tach-bench.tar.gz"
if [ ! -f "$TARBALL" ]; then
  echo "Source tarball not found; building..."
  (cd "$REPO_ROOT" && tar czf "$TARBALL" \
    --exclude target --exclude .git --exclude 'benches/assets' \
    --exclude 'benches/results' --exclude 'benches/scripts/.*' \
    --exclude '*.md' \
    src tools examples benches Cargo.toml Cargo.lock)
fi

echo "[$CELL] Launching $INSTANCE_TYPE on $AMI..."
INSTANCE_ID=$(AWS_PROFILE=$PROFILE aws ec2 run-instances \
  --region "$REGION" \
  --image-id "$AMI" \
  --instance-type "$INSTANCE_TYPE" \
  --key-name tach-bench-temp \
  --security-group-ids "$SG" \
  --instance-initiated-shutdown-behavior terminate \
  --user-data "file://$REPO_ROOT/benches/scripts/user-data.sh" \
  --block-device-mappings 'DeviceName=/dev/xvda,Ebs={VolumeSize=20,VolumeType=gp3,DeleteOnTermination=true}' \
  --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=tach-bench-$CELL}]" \
  --query 'Instances[0].InstanceId' \
  --output text)

echo "$INSTANCE_ID" > "$RESULT_DIR/instance-id.txt"
echo "[$CELL] Launched $INSTANCE_ID"

cleanup() {
  echo "[$CELL] Terminating $INSTANCE_ID..."
  AWS_PROFILE=$PROFILE aws ec2 terminate-instances --region "$REGION" \
    --instance-ids "$INSTANCE_ID" --output text > /dev/null 2>&1 || true
  # Wait for full termination — shutting-down instances still consume vCPU quota,
  # so the next sequential cell would hit VcpuLimitExceeded on metal-class cells.
  echo "[$CELL] Waiting for instance-terminated (vCPU release)..."
  AWS_PROFILE=$PROFILE aws ec2 wait instance-terminated --region "$REGION" \
    --instance-ids "$INSTANCE_ID" 2>/dev/null || true
}
trap cleanup EXIT

echo "[$CELL] Waiting for instance running..."
AWS_PROFILE=$PROFILE aws ec2 wait instance-running --region "$REGION" --instance-ids "$INSTANCE_ID"

PUBLIC_IP=$(AWS_PROFILE=$PROFILE aws ec2 describe-instances \
  --region "$REGION" --instance-ids "$INSTANCE_ID" \
  --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)
echo "$PUBLIC_IP" > "$RESULT_DIR/public-ip.txt"
echo "[$CELL] Public IP: $PUBLIC_IP"

echo "[$CELL] Waiting for cloud-init bootstrap..."
for attempt in $(seq 1 90); do
  if ssh -i "$KEY" \
    -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    -o ConnectTimeout=5 -o BatchMode=yes \
    ec2-user@"$PUBLIC_IP" 'test -f /home/ec2-user/.ready' 2>/dev/null; then
    echo "[$CELL] Bootstrap complete after ${attempt} tries"
    break
  fi
  sleep 5
done

if ! ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
   -o BatchMode=yes ec2-user@"$PUBLIC_IP" 'test -f /home/ec2-user/.ready' 2>/dev/null; then
  echo "[$CELL] BOOTSTRAP FAILED"
  AWS_PROFILE=$PROFILE aws ec2 get-console-output --region "$REGION" \
    --instance-id "$INSTANCE_ID" --query Output --output text > "$RESULT_DIR/console.txt" || true
  exit 1
fi

echo "[$CELL] Uploading source..."
scp -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  "$TARBALL" \
  "$REPO_ROOT/benches/scripts/run-on-instance.sh" \
  ec2-user@"$PUBLIC_IP":/home/ec2-user/

# Rename the tarball without the leading dot.
ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  ec2-user@"$PUBLIC_IP" "mv /home/ec2-user/.tach-bench.tar.gz /home/ec2-user/tach-bench.tar.gz 2>/dev/null || true"

echo "[$CELL] Running bench (this takes 5-15 min)..."
ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  ec2-user@"$PUBLIC_IP" 'bash /home/ec2-user/run-on-instance.sh' \
  > "$RESULT_DIR/stdout.txt" 2> "$RESULT_DIR/stderr.txt" || {
    echo "[$CELL] BENCH FAILED (exit $?)"
    tail -40 "$RESULT_DIR/stderr.txt"
    exit 1
  }

echo "[$CELL] Pulling phase logs..."
scp -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  ec2-user@"$PUBLIC_IP":'/home/ec2-user/tach/phase-*.log' "$RESULT_DIR/" 2>/dev/null || true

# Verify the cycles-le-instant contract.
if grep -q "cycles-le-instant.*fail" "$RESULT_DIR"/phase-*.log 2>/dev/null; then
  echo "[$CELL] CONTRACT VIOLATION: cycles-le-instant=fail"
  grep "cycles-le-instant" "$RESULT_DIR"/phase-*.log
  exit 3
fi

echo "[$CELL] Done. Results in $RESULT_DIR"
ls -la "$RESULT_DIR"
