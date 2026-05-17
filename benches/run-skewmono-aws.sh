#!/usr/bin/env bash
# Launch an AWS EC2 instance, run the skew+monotonicity bench on it, copy
# the JSON back, and terminate.
#
# Usage: benches/run-skewmono-aws.sh <cell-name> <instance-type> [--use-docker-alpine]

set -euo pipefail

cell="${1:?cell name required}"
itype="${2:?instance type required}"
use_docker=0
for a in "${@:3}"; do
  case "$a" in
    --use-docker-alpine) use_docker=1 ;;
  esac
done

profile=tach
region=us-east-2
key_name=tach-bench-temp
key_file=~/.ssh/tach-bench-key.pem
sg_id=sg-05e99abafa54936d3
tag_value="tach-bench-skew-${cell}-$(date +%s)"

case "$itype" in
  c7g*|c8g*|t4g*|c6g*|m7g*) arch=arm64 ;;
  *) arch=x86_64 ;;
esac

echo "[$(date +%T)] resolving AMI ($arch) ..."
# SSM access not granted; use ec2:DescribeImages.
ami=$(aws ec2 describe-images --profile "$profile" --region "$region" \
  --owners amazon \
  --filters "Name=name,Values=al2023-ami-2023*-kernel-6.12-${arch}" "Name=state,Values=available" \
  --query "reverse(sort_by(Images, &CreationDate))[0].ImageId" --output text)
echo "  AMI: $ami"

echo "[$(date +%T)] launching $itype tagged $tag_value ..."
iid=$(aws ec2 run-instances --profile "$profile" --region "$region" \
  --image-id "$ami" \
  --instance-type "$itype" \
  --key-name "$key_name" \
  --security-group-ids "$sg_id" \
  --instance-initiated-shutdown-behavior terminate \
  --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$tag_value}]" \
  --query 'Instances[0].InstanceId' --output text)
echo "  instance: $iid"

cleanup() {
  echo "[$(date +%T)] terminating $iid ..."
  aws ec2 terminate-instances --profile "$profile" --region "$region" \
    --instance-ids "$iid" --query 'TerminatingInstances[0].CurrentState.Name' --output text || true
}
trap cleanup EXIT

echo "[$(date +%T)] waiting for instance running ..."
aws ec2 wait instance-running --profile "$profile" --region "$region" --instance-ids "$iid"

pub_ip=$(aws ec2 describe-instances --profile "$profile" --region "$region" \
  --instance-ids "$iid" --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)
echo "  ip: $pub_ip"

echo "[$(date +%T)] waiting for SSH ..."
for i in $(seq 1 60); do
  if ssh -i "$key_file" -o StrictHostKeyChecking=no -o ConnectTimeout=5 \
       -o BatchMode=yes ec2-user@"$pub_ip" true 2>/dev/null; then
    echo "  SSH up after ${i}0s"
    break
  fi
  sleep 10
done

ssh_args=(-i "$key_file" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
          -o LogLevel=ERROR)
remote() { ssh "${ssh_args[@]}" ec2-user@"$pub_ip" "$@"; }
scp_from() { scp "${ssh_args[@]}" "ec2-user@$pub_ip:$1" "$2"; }

echo "[$(date +%T)] preparing tarball ..."
local_tar=/tmp/tach-bench-${cell}.tar.gz
# Whole source tree; exclude target/, .git/, criterion outputs, and rendered figures.
tar --exclude='./target' --exclude='./.git' --exclude='./.github' \
    --exclude='*.svg' --exclude='*.png' --exclude='./benches/skewmono-*.json' \
    -czf "$local_tar" -C . \
    Cargo.toml Cargo.lock src benches examples
echo "  tarball: $(du -sh "$local_tar" | cut -f1)"

if [[ "$use_docker" == "1" ]]; then
  echo "[$(date +%T)] installing docker on host ..."
  remote 'sudo dnf install -y docker 2>&1 | tail -3; sudo systemctl start docker'
  scp "${ssh_args[@]}" "$local_tar" "ec2-user@$pub_ip:/tmp/tach-src.tgz"
  echo "[$(date +%T)] running bench inside Alpine container ..."
  remote bash -s <<EOF
set -e
mkdir -p /tmp/tach && tar -xzf /tmp/tach-src.tgz -C /tmp/tach
cat > /tmp/in-alpine.sh <<'INNER'
#!/bin/sh
set -e
apk add --no-cache build-base curl bash python3 2>&1 | tail -2
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > /tmp/rustup.sh
sh /tmp/rustup.sh -y --profile minimal --default-toolchain stable 2>&1 | tail -2
. \$HOME/.cargo/env
cd /tach
chmod +x benches/run-skewmono-local.sh
benches/run-skewmono-local.sh $cell
cp benches/skewmono-${cell}.json /out/
INNER
chmod +x /tmp/in-alpine.sh
mkdir -p /tmp/out
sudo docker run --rm \
  -v /tmp/tach:/tach \
  -v /tmp/in-alpine.sh:/in.sh \
  -v /tmp/out:/out \
  alpine:latest /in.sh
ls -la /tmp/out/
EOF
  echo "[$(date +%T)] copying JSON back ..."
  scp_from "/tmp/out/skewmono-${cell}.json" "benches/skewmono-${cell}.json"
else
  echo "[$(date +%T)] preparing remote (gcc, rust, source) ..."
  remote 'sudo dnf install -y gcc tar python3 2>&1 | tail -3'
  remote 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable 2>&1 | tail -3'
  scp "${ssh_args[@]}" "$local_tar" "ec2-user@$pub_ip:/home/ec2-user/tach-src.tgz"
  remote 'mkdir -p ~/tach && tar -xzf ~/tach-src.tgz -C ~/tach'
  echo "[$(date +%T)] running bench ..."
  remote 'source ~/.cargo/env && cd ~/tach && chmod +x benches/run-skewmono-local.sh && benches/run-skewmono-local.sh '"$cell"' 2>&1 | tail -60'
  echo "[$(date +%T)] copying JSON back ..."
  scp_from "tach/benches/skewmono-${cell}.json" "benches/skewmono-${cell}.json"
fi

rm -f "$local_tar"
echo "[$(date +%T)] DONE: benches/skewmono-${cell}.json"
