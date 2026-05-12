#!/bin/bash
# Cloud-init bootstrap. Runs once on EC2 instance launch.
# Installs Rust toolchain and signals readiness via /home/ec2-user/.ready.
# Idempotent — safe to re-invoke.

exec > /var/log/user-data.log 2>&1
set -ex

dnf install -y gcc tar gzip util-linux

sudo -u ec2-user bash <<'EOSU'
set -ex
cd /home/ec2-user
if [ ! -d "$HOME/.cargo" ]; then
  curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    --default-toolchain stable --profile minimal --no-modify-path
fi
echo "BOOTSTRAP_READY" > /home/ec2-user/.ready
EOSU
