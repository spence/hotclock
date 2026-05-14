#!/bin/bash
# Aggregate every cell's phase-b.log + clock-survey.log into a unified
# 20-column-per-cell markdown table.
#
# Usage: benches/scripts/render-baseline.sh [output-file]
# Default output: benches/baseline-$(date +%F).md

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
RESULTS_DIR="$REPO_ROOT/benches/results"
OUTPUT="${1:-$REPO_ROOT/benches/baseline-$(date +%F).md}"

python3 - "$RESULTS_DIR" "$OUTPUT" <<'PY'
import os
import re
import sys
from pathlib import Path

results_dir = Path(sys.argv[1])
output_path = Path(sys.argv[2])

# Cell order for the table. Cells not present in results/ are skipped with a note.
CELLS = [
    # AWS bare metal
    ("m7i-metal-24xl",         "x86_64-linux",   "Intel SapphireRapids bare metal", "m7i.metal-24xl"),
    ("z1d-metal",              "x86_64-linux",   "Intel Skylake bare metal",        "z1d.metal"),
    ("c7a-metal-48xl",         "x86_64-linux",   "AMD Zen4 bare metal",             "c7a.metal-48xl"),
    ("c7g-metal",              "aarch64-linux",  "Graviton 3 bare metal",           "c7g.metal"),
    ("c8g-metal-24xl",         "aarch64-linux",  "Graviton 4 bare metal",           "c8g.metal-24xl"),
    # AWS Nitro VM
    ("m7i-4xlarge",            "x86_64-linux",   "Intel Nitro VM",                  "m7i.4xlarge"),
    ("c7a-4xlarge",            "x86_64-linux",   "AMD Zen4 Nitro VM",               "c7a.4xlarge"),
    ("t3-medium",              "x86_64-linux",   "Intel burst VM",                  "t3.medium"),
    ("c7g-4xlarge",            "aarch64-linux",  "Graviton 3 Nitro VM",             "c7g.4xlarge"),
    ("c8g-4xlarge",            "aarch64-linux",  "Graviton 4 Nitro VM",             "c8g.4xlarge"),
    # AWS musl / Alpine
    ("alpine-x86_64-musl",     "x86_64-musl",    "Alpine Docker on Intel host",     "alpine on m7i.metal-24xl"),
    ("alpine-aarch64-musl",    "aarch64-musl",   "Alpine Docker on Graviton host",  "alpine on c7g.metal"),
    # AWS Lambda
    ("lambda-x86_64",          "x86_64-linux",   "AWS Lambda (Firecracker)",        "Lambda provided.al2023 x86_64"),
    ("lambda-aarch64",         "aarch64-linux",  "AWS Lambda (Firecracker)",        "Lambda provided.al2023 arm64"),
    # GitHub-hosted
    ("github-ubuntu-x86_64",   "x86_64-linux",   "GitHub Actions VM",               "ubuntu-24.04"),
    ("github-ubuntu-aarch64",  "aarch64-linux",  "GitHub Actions VM",               "ubuntu-24.04-arm"),
    ("github-windows-x86_64",  "x86_64-windows", "GitHub Actions VM",               "windows-2025"),
    ("github-windows-aarch64", "aarch64-windows","GitHub Actions VM",               "windows-11-arm"),
    ("github-macos-intel",     "x86_64-darwin",  "GitHub Actions VM",               "macos-15-intel"),
    # Local
    ("local-catalyst",         "aarch64-darwin", "Apple Silicon MBP",               "catalyst"),
    ("local-catalyst-mini",    "aarch64-darwin", "Apple M4 Pro",                    "catalyst-mini"),
    ("local-rosetta",          "x86_64-darwin",  "Rosetta 2 on Apple Silicon",      "catalyst / arch -x86_64"),
    ("local-docker-arm64",     "aarch64-linux",  "Docker Debian arm64 on Mac",      "Docker on catalyst"),
    ("local-docker-arm64-musl","aarch64-musl",   "Docker Alpine arm64 on Mac",      "Docker on catalyst"),
]

# Columns: identifier (3) + measurement (20) = 23
HEADERS = [
    "Target", "Env", "Instance",
    "RDTSC", "RDTSCP", "direct-RDPMC", "perf-RDPMC",
    "CNTVCT_EL0", "CNTPCT_EL0", "PMCCNTR(direct)", "PMCCNTR(perf)",
    "mach_abs", "mach_cont",
    "MONOTONIC", "MONO_RAW", "BOOTTIME",
    "QPC",
    "std::Instant", "tach::Instant", "tach::Cycles",
    "quanta", "minstant", "fastant",
]

# Map clock-survey output name → column header.
SURVEY_NAME_TO_COL = {
    "RDTSC": "RDTSC",
    "RDTSCP": "RDTSCP",
    "direct-RDPMC": "direct-RDPMC",
    "perf-RDPMC": "perf-RDPMC",
    "CNTVCT_EL0": "CNTVCT_EL0",
    "CNTPCT_EL0": "CNTPCT_EL0",
    "PMCCNTR_EL0 (direct)": "PMCCNTR(direct)",
    "PMCCNTR_EL0 (perf)":   "PMCCNTR(perf)",
    "PMCCNTR_EL0":          "PMCCNTR(direct)",  # non-Linux: single line
    "mach_absolute_time":   "mach_abs",
    "mach_continuous_time": "mach_cont",
    "clock_gettime(MONOTONIC)":     "MONOTONIC",
    "clock_gettime(MONOTONIC_RAW)": "MONO_RAW",
    "clock_gettime(BOOTTIME)":      "BOOTTIME",
    "QueryPerformanceCounter":      "QPC",
}

# Map phase-b box field → column header.
PHASEB_FIELD_TO_COL = {
    "tach-instant-bench": "tach::Instant",
    "tach-cycles-bench":  "tach::Cycles",
    "quanta-bench":       "quanta",
    "minstant-bench":     "minstant",
    "fastant-bench":      "fastant",
    "std-instant-bench":  "std::Instant",
}

NUM_RE = re.compile(r"([0-9]+\.[0-9]+)\s*ns/op")

def parse_clock_survey(path):
    """Returns {column_header: cell_value} parsed from clock-survey.log."""
    out = {}
    if not path.exists():
        return out
    for line in path.read_text(errors="replace").splitlines():
        line = line.rstrip()
        if not line:
            continue
        # Match the survey output format: "NAME   X.XXX ns/op" or "NAME   unavailable: REASON"
        # Identify column by longest-prefix match.
        for name, col in sorted(SURVEY_NAME_TO_COL.items(), key=lambda kv: -len(kv[0])):
            if line.startswith(name) and (len(line) == len(name) or line[len(name)] in " \t"):
                rest = line[len(name):].strip()
                if "unavailable" in rest:
                    out[col] = "unavail"
                else:
                    m = NUM_RE.search(rest)
                    if m:
                        out[col] = m.group(1)
                break
    return out

def parse_phaseb(path):
    """Returns {column_header: ns_str} for the API + comparator columns."""
    out = {}
    if not path.exists():
        return out
    for line in path.read_text(errors="replace").splitlines():
        # box rows look like:  │ tach-instant-bench             │ 9.489 ns/op  │
        parts = [p.strip() for p in line.split("│")]
        if len(parts) >= 3:
            field, value = parts[1], parts[2]
            if field in PHASEB_FIELD_TO_COL:
                m = NUM_RE.search(value)
                if m:
                    out[PHASEB_FIELD_TO_COL[field]] = m.group(1)
    return out

def render_row(cell, target, env, instance):
    cell_dir = results_dir / cell
    survey = parse_clock_survey(cell_dir / "clock-survey.log")
    phaseb = parse_phaseb(cell_dir / "phase-b.log")
    values = {**survey, **phaseb}

    row = [target, env, instance]
    for col in HEADERS[3:]:
        row.append(values.get(col, "—"))
    return row

# Filter to cells that actually have results
present = [(cell, target, env, instance) for cell, target, env, instance in CELLS
           if (results_dir / cell / "phase-b.log").exists()]
missing = [cell for cell, _, _, _ in CELLS if not (results_dir / cell / "phase-b.log").exists()]

lines = []
lines.append("# tach benchmark baseline — " + os.popen("date +%F").read().strip())
lines.append("")
lines.append("Unified 20-column-per-cell read-cost matrix. Numbers are median ns/op (Phase B: 5M iters × 101 samples, pinned core where the OS supports `taskset`).")
lines.append("")
lines.append("Identifier columns:")
lines.append("- **Target** — Rust target triple family (arch + libc/OS short form)")
lines.append("- **Env** — environment description")
lines.append("- **Instance** — concrete instance type or host name")
lines.append("")
lines.append("Measurement columns (20 total):")
lines.append("- **Clock primitives:** RDTSC / RDTSCP / direct-RDPMC / perf-RDPMC (x86_64), CNTVCT_EL0 / CNTPCT_EL0 / PMCCNTR(direct) / PMCCNTR(perf) (aarch64), mach_abs / mach_cont (Darwin), MONOTONIC / MONO_RAW / BOOTTIME (Linux clock_gettime), QPC (Windows QueryPerformanceCounter)")
lines.append("- **APIs:** std::Instant, tach::Instant, tach::Cycles")
lines.append("- **Comparator crates:** quanta, minstant, fastant")
lines.append("")
lines.append("Cells missing data (not yet run, or `cycles-le-instant=fail`):")
if missing:
    for c in missing:
        lines.append(f"- `{c}`")
else:
    lines.append("- (none)")
lines.append("")

# Table.
hdr = "| " + " | ".join(HEADERS) + " |"
sep = "|" + "|".join(["---"] * len(HEADERS)) + "|"
lines.append(hdr)
lines.append(sep)
for cell, target, env, instance in present:
    row = render_row(cell, target, env, instance)
    lines.append("| " + " | ".join(row) + " |")

output_path.write_text("\n".join(lines) + "\n")
print(f"Wrote {len(present)} cells to {output_path} ({len(missing)} missing).")
PY
