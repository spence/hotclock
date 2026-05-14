#!/bin/bash
# Per-instance bench execution. Runs on the EC2 instance after source has been
# uploaded to /home/ec2-user/tach-bench.tar.gz. Captures metadata, builds the
# validation runner, runs Phase A (TACH_SELECTOR_TRACE=1) and Phase B (pinned).

set -e
source /home/ec2-user/.cargo/env

cd /home/ec2-user
rm -rf tach && mkdir tach
tar xzf tach-bench.tar.gz -C tach
cd tach/tools/selection-validation-runner

echo "=== METADATA ==="
uname -a
cat /etc/os-release | head -5
echo "--- CPU ---"
grep -m1 "model name\|CPU implementer\|CPU part\|cpu MHz\|vendor_id" /proc/cpuinfo || true
echo "--- PMU policy ---"
echo "rdpmc:"; cat /sys/bus/event_source/devices/cpu/rdpmc 2>/dev/null || cat /sys/devices/cpu/rdpmc 2>/dev/null || echo "not present"
echo "perf_user_access:"; cat /proc/sys/kernel/perf_user_access 2>/dev/null || echo "not present"
echo "perf_event_paranoid:"; cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null || echo "not present"
rustc --version

echo "=== BUILD ==="
cargo build --release 2>&1 | tail -10

BIN="$(pwd)/target/release/tach-selection-validation-runner"
ls -la "$BIN"

echo
echo "=== PHASE A (trace) ==="
TACH_SELECTOR_TRACE=1 "$BIN" 2>&1 | tee /home/ec2-user/tach/phase-a.log

echo
echo "=== PHASE B (high-confidence) ==="
TACH_VALIDATION_MEASURE_ITERS=5000000 TACH_VALIDATION_SAMPLES=101 \
  taskset -c "$(( $(nproc) - 1 ))" "$BIN" 2>&1 | tee /home/ec2-user/tach/phase-b.log

echo
echo "=== CLOCK SURVEY ==="
cd /home/ec2-user/tach/tools/clock-survey
cargo build --release 2>&1 | tail -5
SURVEY_BIN="$(pwd)/target/release/clock-survey"
if command -v taskset >/dev/null 2>&1; then
  taskset -c "$(( $(nproc) - 1 ))" "$SURVEY_BIN" 2>&1 | tee /home/ec2-user/tach/clock-survey.log
else
  "$SURVEY_BIN" 2>&1 | tee /home/ec2-user/tach/clock-survey.log
fi

echo
echo "=== DONE ==="
