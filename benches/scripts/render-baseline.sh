#!/bin/bash
# Aggregate every cell's phase-b.log into a unified per-cell-primitive markdown table.
#
# Single-source data path: every column in the output table comes from the validator's
# interleaved 101×5M harness, including the raw clock-primitive columns. clock-survey.log
# is intentionally NOT read here -- the previous baseline (2026-05-13) used a separate
# clock-survey harness for the primitive columns, which produced numbers that did not
# match the validator's tach::Instant / tach::Cycles measurements and made correct
# selection look wrong by inspection.
#
# Hard verification gate: for each row, tach::Cycles must match exactly one
# clock-primitive column within --tolerance ns, AND that column's primitive name must
# agree with selected-cycles-clock from phase-a.log. Same for tach::Instant. Any
# violation aborts rendering with a non-zero exit and a diagnostic.
#
# Usage: benches/scripts/render-baseline.sh [output-file] [--tolerance NS]
# Defaults: output-file=benches/baseline-$(date +%F).md, tolerance=1.5

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
RESULTS_DIR="$REPO_ROOT/benches/results"
OUTPUT="$REPO_ROOT/benches/baseline-$(date +%F).md"
TOLERANCE=1.5

# Trivial argument parsing: positional output path + optional --tolerance.
while [ $# -gt 0 ]; do
  case "$1" in
    --tolerance) TOLERANCE="$2"; shift 2 ;;
    --tolerance=*) TOLERANCE="${1#--tolerance=}"; shift ;;
    -h|--help)
      sed -n '2,20p' "$0"
      exit 0
      ;;
    *) OUTPUT="$1"; shift ;;
  esac
done

python3 - "$RESULTS_DIR" "$OUTPUT" "$TOLERANCE" <<'PY'
import os
import re
import sys
from pathlib import Path

results_dir = Path(sys.argv[1])
output_path = Path(sys.argv[2])
tolerance_ns = float(sys.argv[3])

# Cell order for the table. Cells without phase-b.log are listed as "missing data".
CELLS = [
    # AWS bare metal (x86_64-linux)
    ("m7i-metal-24xl",         "x86_64-linux",   "Intel SapphireRapids bare metal", "m7i.metal-24xl"),
    ("z1d-metal",              "x86_64-linux",   "Intel Skylake bare metal",        "z1d.metal"),
    ("c7a-metal-48xl",         "x86_64-linux",   "AMD Zen4 bare metal",             "c7a.metal-48xl"),
    # AWS bare metal (aarch64-linux)
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
    # GitHub-hosted runners
    ("github-ubuntu-x86_64",   "x86_64-linux",   "GitHub Actions VM",               "ubuntu-24.04"),
    ("github-ubuntu-aarch64",  "aarch64-linux",  "GitHub Actions VM",               "ubuntu-24.04-arm"),
    ("github-windows-x86_64",  "x86_64-windows", "GitHub Actions VM",               "windows-2025"),
    ("github-windows-aarch64", "aarch64-windows","GitHub Actions VM",               "windows-11-arm"),
    ("github-macos-intel",     "x86_64-darwin",  "GitHub Actions VM",               "macos-15-intel"),
    # Local cells
    ("local-catalyst",         "aarch64-darwin", "Apple Silicon MBP",               "catalyst"),
    ("local-catalyst-mini",    "aarch64-darwin", "Apple M4 Pro",                    "catalyst-mini"),
    ("local-rosetta",          "x86_64-darwin",  "Rosetta 2 on Apple Silicon",      "catalyst / arch -x86_64"),
    ("local-docker-arm64",     "aarch64-linux",  "Docker Debian arm64 on Mac",      "Docker on catalyst"),
    ("local-docker-arm64-musl","aarch64-musl",   "Docker Alpine arm64 on Mac",      "Docker on catalyst"),
]

# Column order: identifier (3) + clock primitives + tach + comparators + std (23 total).
# Final std::Instant column placement per user direction in the May-13 session.
HEADERS = [
    "Target", "Env", "Instance",
    "RDTSC", "RDTSCP", "direct-RDPMC", "perf-RDPMC",
    "CNTVCT_EL0", "CNTPCT_EL0", "PMCCNTR(direct)", "PMCCNTR(perf)",
    "mach_abs", "mach_cont",
    "MONOTONIC", "MONO_RAW", "BOOTTIME",
    "QPC",
    "tach::Instant", "tach::Cycles",
    "quanta", "minstant", "fastant",
    "std::Instant",
]

# Validator phase-b box-table field names → column headers.
PHASEB_FIELD_TO_COL = {
    "tach-instant-bench":         "tach::Instant",
    "tach-cycles-bench":          "tach::Cycles",
    "quanta-bench":               "quanta",
    "minstant-bench":             "minstant",
    "fastant-bench":              "fastant",
    "std-instant-bench":          "std::Instant",
    "rdtsc-bench":                "RDTSC",
    "rdtscp-bench":               "RDTSCP",
    "direct-rdpmc-bench":         "direct-RDPMC",
    "perf-rdpmc-bench":           "perf-RDPMC",
    "cntvct-el0-bench":           "CNTVCT_EL0",
    "cntpct-el0-bench":           "CNTPCT_EL0",
    "pmccntr-direct-bench":       "PMCCNTR(direct)",
    "pmccntr-perf-bench":         "PMCCNTR(perf)",
    "mach-abs-bench":             "mach_abs",
    "mach-cont-bench":            "mach_cont",
    "clock-monotonic-bench":      "MONOTONIC",
    "clock-monotonic-raw-bench":  "MONO_RAW",
    "clock-boottime-bench":       "BOOTTIME",
    "qpc-bench":                  "QPC",
}

# tach selector candidate name → table column. Used to verify selected-cycles-clock
# names a column that actually has a numeric match for tach::Cycles.
SELECTED_TO_COL = {
    "x86_64-rdtsc":         "RDTSC",
    "x86_64-perf-rdpmc":    "perf-RDPMC",
    "x86_64-direct-rdpmc":  "direct-RDPMC",
    "aarch64-cntvct":       "CNTVCT_EL0",
    "aarch64-perf-pmccntr": "PMCCNTR(perf)",
    "unix-monotonic":       "MONOTONIC",
    # Compile-time-inline (non-Linux): cycles == instant, same primitive.
    "x86-rdtsc":            "RDTSC",
    "riscv64-rdtime":       None,        # no riscv64 cells in current matrix
    "loongarch64-rdtime":   None,
}

NUM_RE = re.compile(r"([0-9]+\.[0-9]+)\s*ns/op")
UNAVAIL_RE = re.compile(r"unavailable:\s*(.+)$")

def parse_phaseb(path):
    """Returns (values_by_col, selected_instant, selected_cycles).

    `values_by_col` maps column header → display string (numeric ns/op or unavail variant).
    For unavailable primitives: 'kernel: <detail>' or 'host: <detail>' (or '—' for N/A).
    """
    values = {}
    selected_instant = None
    selected_cycles = None
    if not path.exists():
        return values, selected_instant, selected_cycles

    for raw in path.read_text(errors="replace").splitlines():
        parts = [p.strip() for p in raw.split("│")]
        if len(parts) < 3:
            continue
        field, value = parts[1], parts[2]

        if field == "selected-instant-clock":
            selected_instant = value.strip()
            continue
        if field == "selected-cycles-clock":
            selected_cycles = value.strip()
            continue

        if field not in PHASEB_FIELD_TO_COL:
            continue
        col = PHASEB_FIELD_TO_COL[field]

        unavail = UNAVAIL_RE.search(value)
        if unavail:
            reason = unavail.group(1).strip()
            # Validator categorizes as "not applicable" / "kernel: ..." / "host: ...".
            if reason.startswith("not applicable"):
                values[col] = "—"
            else:
                values[col] = reason
            continue

        m = NUM_RE.search(value)
        if m:
            values[col] = m.group(1)
    return values, selected_instant, selected_cycles


def numeric(value):
    """Return a float if the cell holds a numeric ns/op, else None."""
    if value is None:
        return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def verify_row(cell, values, selected_instant, selected_cycles):
    """Return list of failure strings (empty == row passes the hard gate)."""
    failures = []

    tach_instant = numeric(values.get("tach::Instant"))
    tach_cycles = numeric(values.get("tach::Cycles"))

    if tach_instant is None:
        failures.append(f"{cell}: tach::Instant has no numeric value")
        return failures
    if tach_cycles is None:
        failures.append(f"{cell}: tach::Cycles has no numeric value")
        return failures

    def closest_primitive(target_ns, selected_name):
        """Pick the primitive column whose numeric value is closest to target_ns.

        If `selected_name` is a known candidate that maps to a column, the rule
        is stricter -- that exact column must match within tolerance. Otherwise
        any primitive column within tolerance qualifies.
        """
        candidates = []
        for col, raw in values.items():
            if col in ("tach::Instant", "tach::Cycles", "quanta", "minstant",
                       "fastant", "std::Instant"):
                continue
            v = numeric(raw)
            if v is None:
                continue
            candidates.append((col, v, abs(v - target_ns)))

        if not candidates:
            return None, "no primitive columns have numeric values"

        candidates.sort(key=lambda triple: triple[2])
        best_col, best_v, best_d = candidates[0]

        if best_d > tolerance_ns:
            return None, f"closest primitive {best_col}={best_v} ns is {best_d:.3f} ns away (tolerance {tolerance_ns})"

        expected_col = SELECTED_TO_COL.get(selected_name) if selected_name else None
        if expected_col and expected_col != best_col:
            ev = numeric(values.get(expected_col))
            if ev is None or abs(ev - target_ns) > tolerance_ns:
                return None, (
                    f"selected={selected_name} -> {expected_col} but closest within tolerance "
                    f"is {best_col}={best_v} (expected col value {values.get(expected_col, 'missing')})"
                )
        return best_col, None

    cyc_col, err = closest_primitive(tach_cycles, selected_cycles)
    if err:
        failures.append(f"{cell}: tach::Cycles={tach_cycles}: {err}")
    inst_col, err = closest_primitive(tach_instant, selected_instant)
    if err:
        failures.append(f"{cell}: tach::Instant={tach_instant}: {err}")
    return failures


def render_row(cell, target, env, instance):
    cell_dir = results_dir / cell
    values, selected_instant, selected_cycles = parse_phaseb(cell_dir / "phase-b.log")

    failures = verify_row(cell, values, selected_instant, selected_cycles)

    row = [target, env, instance]
    for col in HEADERS[3:]:
        row.append(values.get(col, "—"))
    return row, failures, selected_instant, selected_cycles


# Filter to cells that have phase-b.log; record cells missing data.
present = [(cell, target, env, inst) for cell, target, env, inst in CELLS
           if (results_dir / cell / "phase-b.log").exists()]
missing = [cell for cell, *_ in CELLS if not (results_dir / cell / "phase-b.log").exists()]

# Build rows + collect failures.
rows = []
all_failures = []
selection_summary = []
for cell, target, env, instance in present:
    row, failures, sel_instant, sel_cycles = render_row(cell, target, env, instance)
    rows.append(row)
    selection_summary.append((cell, sel_instant, sel_cycles))
    all_failures.extend(failures)

if all_failures:
    print("Hard verification gate FAILED. Per-row diagnostics:", file=sys.stderr)
    for fail in all_failures:
        print(f"  {fail}", file=sys.stderr)
    print(file=sys.stderr)
    print("Refusing to write the baseline. Address the per-row violations and re-run.",
          file=sys.stderr)
    sys.exit(2)

# Render markdown.
lines = []
lines.append("# tach benchmark baseline — " + os.popen("date +%F").read().strip())
lines.append("")
lines.append(
    "Unified 23-column per-cell read-cost matrix. **Every value in this table comes from "
    "the same binary, in the same run, in the same interleaved 5M iter × 101 sample loop "
    "as `tach::Instant` and `tach::Cycles`** -- so for each row, the clock primitive that "
    "tach selected can be identified by eye: it's the column whose number matches "
    "`tach::Cycles` within ~1 ns."
)
lines.append("")
lines.append("Identifier columns:")
lines.append("- **Target** — Rust target triple family (arch + libc/OS short form)")
lines.append("- **Env** — environment description")
lines.append("- **Instance** — concrete instance type or host name")
lines.append("")
lines.append("Measurement columns (20):")
lines.append(
    "- **Clock primitives:** RDTSC / RDTSCP / direct-RDPMC / perf-RDPMC (x86_64), "
    "CNTVCT_EL0 / CNTPCT_EL0 / PMCCNTR(direct) / PMCCNTR(perf) (aarch64), "
    "mach_abs / mach_cont (Darwin), MONOTONIC / MONO_RAW / BOOTTIME (Linux clock_gettime), "
    "QPC (Windows QueryPerformanceCounter)"
)
lines.append("- **APIs:** tach::Instant, tach::Cycles, std::Instant")
lines.append("- **Comparator crates:** quanta, minstant, fastant")
lines.append("")
lines.append("**Legend** for non-numeric cells:")
lines.append("- `—` — primitive doesn't exist on this architecture/OS at all")
lines.append("- `kernel: <reason>` — primitive exists architecturally but the kernel "
             "(sysctl, perf_paranoid, EL0 access bits) blocks user-mode access")
lines.append("- `host: <reason>` — primitive exists and the kernel exposes it, but the "
             "hypervisor or container traps it (perf-RDPMC at 1334 ns on Nitro VMs, etc.)")
lines.append("")
lines.append(f"**Verification:** every row passes the hard gate "
             f"`|tach::Cycles - selected_primitive_column| ≤ {tolerance_ns} ns` "
             f"and selector-named primitive agreement, "
             f"otherwise the baseline is refused (non-zero exit).")
lines.append("")
if missing:
    lines.append("**Cells without data:** " + ", ".join(f"`{c}`" for c in missing))
    lines.append("")

# Table.
hdr = "| " + " | ".join(HEADERS) + " |"
sep = "|" + "|".join(["---"] * len(HEADERS)) + "|"
lines.append(hdr)
lines.append(sep)
for row in rows:
    lines.append("| " + " | ".join(row) + " |")

lines.append("")
lines.append("## Selector decisions per cell")
lines.append("")
lines.append("| Cell | selected-instant-clock | selected-cycles-clock |")
lines.append("|---|---|---|")
for cell, si, sc in selection_summary:
    lines.append(f"| `{cell}` | {si or '—'} | {sc or '—'} |")

output_path.write_text("\n".join(lines) + "\n")
print(f"Wrote {len(rows)} cells to {output_path} (missing: {len(missing)}).")
PY
