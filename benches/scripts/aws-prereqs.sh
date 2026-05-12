#!/bin/bash
# Idempotent AWS setup for the bench matrix:
# - Creates EC2 key pair `tach-bench-temp` if not already present.
# - Adds SSH ingress for this machine's public IP to the default SG.
# - Prints the latest AL2023 AMI IDs for x86_64 and arm64 in us-east-2.
#
# Re-run safely anytime — won't duplicate state.
#
# Usage: AWS_PROFILE=tach benches/scripts/aws-prereqs.sh

set -euo pipefail

REGION=us-east-2
PROFILE="${AWS_PROFILE:-tach}"
REPO_ROOT="$(git rev-parse --show-toplevel)"
SCRIPTS="$REPO_ROOT/benches/scripts"

echo "=== Verifying AWS access ==="
AWS_PROFILE=$PROFILE aws sts get-caller-identity --query 'Arn' --output text

echo "=== Key pair ==="
KEY="$SCRIPTS/.bench-key.pem"
if [ -f "$KEY" ]; then
  echo "Key already exists at $KEY"
else
  echo "Creating ed25519 key pair tach-bench-temp..."
  AWS_PROFILE=$PROFILE aws ec2 create-key-pair --region "$REGION" \
    --key-name tach-bench-temp --key-type ed25519 \
    --query 'KeyMaterial' --output text > "$KEY"
  chmod 600 "$KEY"
  echo "Saved at $KEY"
fi

echo "=== Security group ==="
SG_FILE="$SCRIPTS/.bench-sg.txt"
if [ -f "$SG_FILE" ]; then
  SG="$(cat "$SG_FILE")"
  echo "Using cached SG: $SG"
else
  SG=$(AWS_PROFILE=$PROFILE aws ec2 describe-security-groups --region "$REGION" \
    --filters Name=group-name,Values=default \
    --query 'SecurityGroups[0].GroupId' --output text)
  echo "Default SG: $SG"
  echo "$SG" > "$SG_FILE"
fi

MY_IP=$(curl -s ifconfig.me)
echo "This machine's IP: $MY_IP"

echo "Authorizing SSH from $MY_IP/32 (idempotent)..."
AWS_PROFILE=$PROFILE aws ec2 authorize-security-group-ingress --region "$REGION" \
  --group-id "$SG" --protocol tcp --port 22 --cidr "$MY_IP/32" --output text 2>/dev/null || \
  echo "(SSH ingress likely already authorized)"

echo "=== AMIs ==="
X86_AMI=$(AWS_PROFILE=$PROFILE aws ec2 describe-images --region "$REGION" --owners amazon \
  --filters "Name=name,Values=al2023-ami-2023.*-x86_64" "Name=state,Values=available" \
  --query 'sort_by(Images, &CreationDate)[-1].ImageId' --output text)
ARM_AMI=$(AWS_PROFILE=$PROFILE aws ec2 describe-images --region "$REGION" --owners amazon \
  --filters "Name=name,Values=al2023-ami-2023.*-arm64" "Name=state,Values=available" \
  --query 'sort_by(Images, &CreationDate)[-1].ImageId' --output text)

echo "Latest AL2023 x86_64 AMI: $X86_AMI"
echo "Latest AL2023 arm64  AMI: $ARM_AMI"
echo "$X86_AMI" > "$SCRIPTS/.ami-x86_64.txt"
echo "$ARM_AMI" > "$SCRIPTS/.ami-arm64.txt"

echo
echo "=== Ready ==="
echo "Run individual cells with: benches/scripts/orchestrate-cell.sh <cell> <type> <ami>"
echo "Run the full matrix with:  benches/scripts/run-matrix.sh"
