# Reproducing the tach benchmark matrix

This directory contains the infrastructure to run a representative
(target × environment) sample and verify the contract — `Cycles ≤ Instant`
on every cell, with the selected counter being the fastest available on
that host.

## Prerequisites

- `aws` CLI with a profile that has EC2 launch / SSH key / SG-modify permission. The
  scripts default to `AWS_PROFILE=tach`; override via the environment.
- `gh` CLI authenticated to `spence/tach` (or a fork) for the GitHub-hosted cells.
- `ssh`, `scp`, `tar`, `cargo` available locally.

## One-time setup

```bash
AWS_PROFILE=tach benches/scripts/aws-prereqs.sh
```

Creates an `ed25519` EC2 key pair, authorizes SSH ingress for your public IP
on the default security group in `us-east-2`, and caches the latest AL2023
x86_64 / arm64 AMI IDs. The key, SG ID, and AMI IDs are written to dotfiles
under `benches/scripts/`. Re-running is idempotent.

## Running the matrix

The full baseline run takes ~1.5–3 hours wall clock and ~$5–15 in AWS spend:

```bash
AWS_PROFILE=tach benches/scripts/run-matrix.sh
```

This sequentially provisions each AWS cell, runs the validation runner, pulls
back the phase logs, and terminates the instance. It then triggers the
`selection-experiment` GitHub workflow for the hosted-runner cells and waits
for it to finish. Local macOS results are captured at the end.

Results land under `benches/results/<cell-name>/`. The summary at the end
prints the `cycles-le-instant` line from every Phase B log — every cell must
report `pass`.

## Running a single cell

```bash
AWS_PROFILE=tach benches/scripts/orchestrate-cell.sh m7i-metal-24xl m7i.metal-24xl ami-XXXXXXXXXX
```

The cell name is a free-form identifier; the instance type and AMI must
match. Use `cat benches/scripts/.ami-x86_64.txt` to retrieve the cached AMI
IDs.

## The contract verification gate

Each cell's Phase B output contains a line `cycles-le-instant=pass` or
`cycles-le-instant=fail`. The check is `tach-cycles-bench ≤ tach-instant-bench + 0.5 ns`.

A `fail` indicates a selection or patching bug — Cycles selection always
includes the Instant counter as a candidate, so the chosen Cycles read
should never exceed Instant by more than measurement noise. Investigate any
fail; do not widen the tolerance.

## Cells in the baseline matrix

| Cell | Target | Validates |
|---|---|---|
| `m7i-metal-24xl` | x86_64-linux Intel bare metal | Cycles selects perf-RDPMC (~19 % faster than RDTSC) |
| `m7i-4xlarge` | x86_64-linux Intel virtualized | Cycles falls back to RDTSC when PMU sysfs not exposed |
| `c7a-metal-48xl` | x86_64-linux AMD Zen4 bare metal | Cycles fallback on AMD; cross-thread anomaly documented |
| `c7g-metal` | aarch64-linux Graviton bare metal | aarch64 Cycles compile-time = Instant (no PMU patching in 0.2.0) |
| GitHub `ubuntu-24.04` | x86_64-linux AMD virtualized | Hosted CI: Cycles fallback when PMU denied |
| GitHub `ubuntu-24.04-arm` | aarch64-linux | aarch64 CI: Cycles = Instant |
| GitHub `macos-15` | aarch64-darwin (Apple Silicon) | macOS native: Cycles = Instant (CNTVCT) |
| Local macOS | aarch64-darwin or x86_64-darwin | Developer machine baseline |

Add cells by extending `AWS_CELLS` in `run-matrix.sh` (for AWS) or the
matrix in `.github/workflows/selection-experiment.yml` (for hosted runners).

## Cleanup

Run instances are terminated automatically by `orchestrate-cell.sh` via its
EXIT trap. To audit any orphan instances:

```bash
AWS_PROFILE=tach aws ec2 describe-instances --region us-east-2 \
  --filters "Name=tag:Name,Values=tach-bench-*" "Name=instance-state-name,Values=running,pending" \
  --query 'Reservations[].Instances[].[InstanceId,InstanceType,State.Name]' --output table
```

The key pair `tach-bench-temp` and the SSH-ingress rule on the default SG
persist between runs. Delete them manually if you don't plan to re-run:

```bash
AWS_PROFILE=tach aws ec2 delete-key-pair --region us-east-2 --key-name tach-bench-temp
# (security group rule cleanup requires the rule ID; check 'describe-security-groups')
```
