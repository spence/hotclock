#!/usr/bin/env bash
set -u -o pipefail

examples=(
  comparison_hotclock
  comparison_quanta
  comparison_minstant
  comparison_fastant
  comparison_coarsetime
  comparison_time
  comparison_clock
  comparison_chrono
  comparison_clocksource
  comparison_tick_counter_start
  comparison_tick_counter_elapsed
  comparison_std
)

: "${HOTCLOCK_COMPARISON_BIN_DIR:?}"
: "${HOTCLOCK_COMPARISON_REPORT_DIR:?}"

timeout_seconds="${HOTCLOCK_COMPARISON_TIMEOUT_SECONDS:-120}"
mkdir -p "$HOTCLOCK_COMPARISON_REPORT_DIR"

for example in "${examples[@]}"; do
  bin="$HOTCLOCK_COMPARISON_BIN_DIR/$example"
  out="$HOTCLOCK_COMPARISON_REPORT_DIR/$example"
  timer="${example#comparison_}"
  mkdir -p "$out"

  echo "running $example"
  if HOTCLOCK_COMPARISON_DIR="$out" timeout "$timeout_seconds" "$bin" >"$out/stdout.txt" 2>"$out/stderr.txt"; then
    echo "$example completed"
  else
    status=$?
    echo "$example exited with $status"
    cat >"$out/comparison-report.json" <<JSON
{"schema_version":1,"timer":"$timer","status":"timeout","exit_code":$status,"comparisons":[]}
JSON
    cat >"$out/comparison-report.md" <<MARKDOWN
# hotclock comparison benchmark

Timer: \`$timer\`

Status: \`timeout\`
Exit code: \`$status\`
MARKDOWN
  fi
done
