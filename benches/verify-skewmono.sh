#!/usr/bin/env bash
# End-to-end verification per Phase G of the plan.
# Exits 0 if all artifacts are present and the build matrix is clean.

set -euo pipefail
cd "$(dirname "$0")/.."

red()   { printf '\033[31m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }

fail=0
check() {
  local desc="$1"; shift
  if "$@" >/dev/null 2>&1; then
    green "PASS: $desc"
  else
    red "FAIL: $desc"
    fail=1
  fi
}

echo "── Feature gate matrix ───────────────────────────────────────────────────"
check "default no_std (wasm32v1-none)"  cargo check --lib --target wasm32v1-none
check "default"                          cargo check --lib
check "recalibrate-background"           cargo check --lib --features recalibrate-background
check "bench-internal"                   cargo check --lib --features bench-internal
check "bench-internal + recal-bg"        cargo check --lib --features "bench-internal recalibrate-background"

echo "── Unit tests ────────────────────────────────────────────────────────────"
check "tests default"                    cargo test --lib --tests
check "tests with bench-internal"        cargo test --lib --tests --features bench-internal

echo "── Clippy ────────────────────────────────────────────────────────────────"
check "clippy default"                   cargo clippy --lib --all-targets -- -D warnings
check "clippy bench-internal"            cargo clippy --lib --features bench-internal -- -D warnings

echo "── Per-cell artifacts ────────────────────────────────────────────────────"
cells=(apple-silicon-m1 c7g-4xlarge t3-medium m7i-metal-24xl lambda-x86_64 github-windows-x86_64)
for c in "${cells[@]}"; do
  if [[ -f "benches/skewmono-$c.json" ]]; then
    # validate schema
    if python3 -c "
import json,sys
d=json.load(open('benches/skewmono-$c.json'))
assert d['schema']=='tach-skew-bench/v1', f'bad schema {d.get(\"schema\")}'
assert len(d.get('clocks',{})) >= 6, f'expected >=6 clocks, got {len(d.get(\"clocks\",{}))}'
" 2>/dev/null; then
      green "PASS: benches/skewmono-$c.json (well-formed)"
    else
      red "FAIL: benches/skewmono-$c.json (schema/clock count problem)"
      fail=1
    fi
  else
    red "FAIL: benches/skewmono-$c.json (MISSING)"
    fail=1
  fi
  if [[ -f "benches/report-skewmono-$c.svg" ]]; then
    green "PASS: benches/report-skewmono-$c.svg"
  else
    red "FAIL: benches/report-skewmono-$c.svg (MISSING)"
    fail=1
  fi
done

echo "── README drift markers ──────────────────────────────────────────────────"
# After Phase F, every "~" prefix on numbers in the drift table should be gone
if grep -E '\| ~[0-9]' README.md >/dev/null 2>&1; then
  red "FAIL: README.md still contains ~ estimate markers in the drift table"
  fail=1
else
  green "PASS: README.md drift markers replaced"
fi

echo ""
if [[ "$fail" == "0" ]]; then
  green "═══════════════════ ALL VERIFICATIONS PASSED ═══════════════════"
  exit 0
else
  red   "═══════════════════ VERIFICATION FAILED ═══════════════════"
  exit 1
fi
