#!/bin/bash
# Package tach-selection-validation-runner as an AWS Lambda
# custom-runtime function (provided.al2023), invoke it, and pull the output
# from CloudWatch Logs into the per-cell phase logs.
#
# Usage: AWS_PROFILE=tach benches/scripts/bench-lambda.sh <cell> <arch>
#   <arch> is one of: x86_64 | aarch64
#
# Build host must have a Linux gnu cross-compile setup OR use a Docker build.
# This script uses Docker (the public.ecr.aws/lambda/provided:al2023 base image)
# so it works from macOS too.

set -euo pipefail

CELL="$1"
ARCH="$2"
REPO_ROOT="$(git rev-parse --show-toplevel)"
REGION=us-east-2
PROFILE="${AWS_PROFILE:-tach}"
RESULT_DIR="$REPO_ROOT/benches/results/$CELL"
mkdir -p "$RESULT_DIR"

case "$ARCH" in
  x86_64)
    DOCKER_PLATFORM=linux/amd64
    RUST_TARGET=x86_64-unknown-linux-gnu
    LAMBDA_ARCH=x86_64
    ;;
  aarch64)
    DOCKER_PLATFORM=linux/arm64
    RUST_TARGET=aarch64-unknown-linux-gnu
    LAMBDA_ARCH=arm64
    ;;
  *) echo "Unknown arch: $ARCH" >&2; exit 2 ;;
esac

FN_NAME="tach-bench-$CELL"
ROLE_NAME="tach-bench-lambda-role"
WORK="$RESULT_DIR/build"
rm -rf "$WORK" && mkdir -p "$WORK"

# Create the IAM role if missing.
ROLE_ARN=$(AWS_PROFILE=$PROFILE aws iam get-role --role-name "$ROLE_NAME" \
  --query 'Role.Arn' --output text 2>/dev/null || echo "")
if [ -z "$ROLE_ARN" ] || [ "$ROLE_ARN" = "None" ]; then
  echo "[$CELL] Creating IAM role $ROLE_NAME..."
  AWS_PROFILE=$PROFILE aws iam create-role --role-name "$ROLE_NAME" \
    --assume-role-policy-document '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"lambda.amazonaws.com"},"Action":"sts:AssumeRole"}]}' >/dev/null
  AWS_PROFILE=$PROFILE aws iam attach-role-policy --role-name "$ROLE_NAME" \
    --policy-arn arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole >/dev/null
  echo "[$CELL] Waiting 10s for role to propagate..."
  sleep 10
  ROLE_ARN=$(AWS_PROFILE=$PROFILE aws iam get-role --role-name "$ROLE_NAME" --query 'Role.Arn' --output text)
fi

# Build the validator binary inside Amazon Linux 2023 (matches the Lambda
# provided.al2023 runtime's glibc). The lambda/provided image wraps an
# RIE entrypoint that hijacks `bash -c`, so we use the plain AL2023 image.
echo "[$CELL] Building tach binaries in amazonlinux:2023 ($DOCKER_PLATFORM)..."
docker run --rm --platform=$DOCKER_PLATFORM \
  -v "$REPO_ROOT:/work:ro" -v "$WORK:/out" \
  -w /work public.ecr.aws/amazonlinux/amazonlinux:2023 bash -c "
    dnf install -y gcc tar gzip >/dev/null
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable >/dev/null
    source /root/.cargo/env
    cp -r /work /build
    cd /build/tools/selection-validation-runner
    cargo build --release 2>&1 | tail -5
    cp target/release/tach-selection-validation-runner /out/
  "

# Bootstrap: the Lambda custom-runtime entrypoint. Runs phase-b (validator),
# whose stdout becomes the Lambda function response and writes to phase-b.log.
cat > "$WORK/bootstrap" <<'BOOTSTRAP'
#!/bin/bash
set -e
while true; do
  HEADERS=$(mktemp)
  EVENT=$(curl -sS -D "$HEADERS" "http://${AWS_LAMBDA_RUNTIME_API}/2018-06-01/runtime/invocation/next")
  REQ_ID=$(grep -i lambda-runtime-aws-request-id "$HEADERS" | awk '{print $2}' | tr -d '\r\n')

  # Run the validator with AWS_LAMBDA_RUNTIME_API stripped. The
  # tach-selection-validation-runner binary contains its own Lambda runtime
  # loop that would otherwise race our bash bootstrap to POST /response and
  # win, hijacking the response payload.
  env -u AWS_LAMBDA_RUNTIME_API \
    TACH_VALIDATION_MEASURE_ITERS=5000000 TACH_VALIDATION_SAMPLES=101 \
    /var/task/tach-selection-validation-runner > /tmp/validator.out 2>&1

  curl -sS -X POST "http://${AWS_LAMBDA_RUNTIME_API}/2018-06-01/runtime/invocation/$REQ_ID/response" \
    --data-binary "@/tmp/validator.out" >/dev/null
done
BOOTSTRAP
chmod +x "$WORK/bootstrap"
chmod +x "$WORK/tach-selection-validation-runner"

ZIP="$WORK/function.zip"
(cd "$WORK" && zip -q "$ZIP" bootstrap tach-selection-validation-runner)

# Create or update function.
echo "[$CELL] Deploying Lambda $FN_NAME ($LAMBDA_ARCH)..."
if AWS_PROFILE=$PROFILE aws lambda get-function --region "$REGION" --function-name "$FN_NAME" >/dev/null 2>&1; then
  AWS_PROFILE=$PROFILE aws lambda update-function-code --region "$REGION" \
    --function-name "$FN_NAME" --zip-file "fileb://$ZIP" >/dev/null
  echo "[$CELL] Waiting for update to settle..."
  AWS_PROFILE=$PROFILE aws lambda wait function-updated --region "$REGION" --function-name "$FN_NAME"
else
  AWS_PROFILE=$PROFILE aws lambda create-function --region "$REGION" \
    --function-name "$FN_NAME" \
    --runtime provided.al2023 \
    --role "$ROLE_ARN" \
    --architectures "$LAMBDA_ARCH" \
    --handler bootstrap \
    --timeout 600 \
    --memory-size 1769 \
    --zip-file "fileb://$ZIP" >/dev/null
  echo "[$CELL] Waiting for function active..."
  AWS_PROFILE=$PROFILE aws lambda wait function-active --region "$REGION" --function-name "$FN_NAME"
fi

echo "[$CELL] Invoking validator (phase-b, ~30s)..."
AWS_PROFILE=$PROFILE aws lambda invoke --region "$REGION" \
  --cli-read-timeout 600 --cli-connect-timeout 30 \
  --function-name "$FN_NAME" \
  "$RESULT_DIR/phase-b.log" >/dev/null

if grep -q "cycles-le-instant.*fail" "$RESULT_DIR"/phase-b.log 2>/dev/null; then
  echo "[$CELL] CONTRACT VIOLATION: cycles-le-instant=fail"
  exit 3
fi

echo "[$CELL] Done. Results in $RESULT_DIR"
ls -la "$RESULT_DIR"
