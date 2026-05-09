#!/usr/bin/env python3
"""Render the README benchmark chart with fixed pixel geometry."""

from __future__ import annotations

import html
import shutil
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parent
SVG_PATH = ROOT / "benchmark.svg"
PNG_PATH = ROOT / "benchmark.png"
SIMPLE_SVG_PATH = ROOT / "benchmark-simple.svg"
SIMPLE_PNG_PATH = ROOT / "benchmark-simple.png"

BACKGROUND = "#FBF6EC"
GROUP_BACKGROUND = "#FFF8E8"
FONT = "Avenir Next, Helvetica, Arial, sans-serif"

CRATES = [
  ("clock", "#1B1A17"),
  ("tach@0.2.0", "#D72D24"),
  ("quanta@0.12.6", "#5B6472"),
  ("minstant@0.1.7", "#8B5E3C"),
  ("fastant@0.1.11", "#4F6F6A"),
  ("std@1.91-1.95", "#9A8A3A"),
]

GROUPS = [
  (("macOS aarch64", "aarch64-apple-darwin", "(cntvct)"), [0.330, 0.330, 4.620, 25.700, 25.700, 18.400]),
  (("Docker x86_64", "x86_64-unknown-linux-gnu", "(rdtsc)"), [15.222, 15.394, 25.079, 39.050, 22.066, 28.070]),
  (("Docker x86", "i686-unknown-linux-gnu", "(rdtsc)"), [25.780, 25.789, 253.702, 323.398, 324.264, 222.623]),
  (("Docker aarch64", "aarch64-unknown-linux-gnu", "(cntvct)"), [0.330, 0.330, 4.466, 27.203, 27.275, 20.222]),
  (("Docker riscv64", "riscv64gc-unknown-linux-gnu", "(rdtime)"), [59.584, 59.584, 215.296, 271.262, 271.241, 185.151]),
  (("AWS Nitro x86_64", "t3.micro", "x86_64-unknown-linux-musl", "(rdtsc)"), [9.399, 9.722, 13.954, 10.222, 10.052, 25.300]),
  (("AWS Lambda x86_64", "provided.al2023", "x86_64-unknown-linux-musl", "(rdtsc)"), [17.516, 17.825, 22.172, 75.637, 18.160, 53.764]),
  (("AWS Metal x86_64", "m7i.metal", "x86_64-unknown-linux-musl", "(rdtsc)"), [6.841, 6.841, 7.130, 6.841, 6.841, 14.734]),
  (("AWS Nitro x86", "t3.micro", "i686-unknown-linux-musl", "(rdtsc)"), [10.050, 10.054, 42.391, 10.706, 10.697, 44.581]),
  (("AWS Metal x86", "m7i.metal", "i686-unknown-linux-musl", "(rdtsc)"), [6.841, 6.841, 23.066, 6.841, 6.841, 22.743]),
  (("AWS Lambda aarch64", "provided.al2023", "aarch64-unknown-linux-gnu", "(cntvct)"), [17.328, 17.325, 21.970, 72.252, 74.114, 54.165]),
  (("AWS Nitro x86_64", "Windows c5.large", "x86_64-pc-windows-msvc", "(rdtsc)"), [6.957, 6.957, 11.719, 33.650, 33.656, 39.224]),
]
SIMPLE_GROUPS = [
  (labels[:2] if labels[0].startswith("AWS") else labels[:1], values[1:])
  for labels, values in GROUPS
  if labels[0] == "macOS aarch64" or any("x86_64" in label for label in labels)
]
SIMPLE_CRATES = CRATES[1:]

BAR_WIDTH = 8
BAR_GAP = 4
GROUP_WIDTH = 94
GROUP_GAP = 20
LEFT = 6
HEIGHT = 462
SIMPLE_HEIGHT = 348
LEGEND_Y = 15
LEGEND_GAP = 18
LEGEND_SQUARE = 6
NOTE_Y = 30
GROUP_TOP = 42
SIMPLE_GROUP_TOP = 42
GROUP_HEIGHT = 382
SIMPLE_GROUP_HEIGHT = 270
BAR_BOTTOM = 384
SIMPLE_BAR_BOTTOM = 286
BREAK_VALUE = 80.0
LOWER_BAR_HEIGHT = 160
SIMPLE_LOWER_BAR_HEIGHT = 230
UPPER_BAR_HEIGHT = 110
SIMPLE_UPPER_BAR_HEIGHT = 0
MAX_BAR_HEIGHT = LOWER_BAR_HEIGHT + UPPER_BAR_HEIGHT
VALUE_FONT_SIZE = 7
LABEL_FONT_SIZE = 11
LABEL_LINE_GAP = 13
LABEL_TOP = BAR_BOTTOM + 21
SIMPLE_LABEL_TOP = SIMPLE_BAR_BOTTOM + 24
TARGET_LABEL_FONT_SIZE = 7
LEGEND_FONT_SIZE = 12


def value_label(value: float) -> str:
  if value >= 100:
    return f"{value:.0f}"
  if value >= 10:
    return f"{value:.1f}"
  return f"{value:.2f}"


def esc(value: str) -> str:
  return html.escape(value, quote=True)


def text(x: float, y: float, value: str, size: int, anchor: str = "middle") -> str:
  return (
    f'<text x="{x:g}" y="{y:g}" text-anchor="{anchor}" '
    f'font-family="{FONT}" font-size="{size}" fill="#2E231B">{esc(value)}</text>'
  )


def text_width(value: str, size: int) -> float:
  return len(value) * size * 0.56


def bar_height(value: float, global_max: float, lower_bar_height: int, upper_bar_height: int) -> int:
  if value <= BREAK_VALUE:
    return max(2, round(value / BREAK_VALUE * lower_bar_height))
  upper = (value - BREAK_VALUE) / (global_max - BREAK_VALUE) * upper_bar_height
  return round(lower_bar_height + upper)


def bar_break(
  x: float,
  bar_bottom: int,
  lower_bar_height: int,
  group_background: str,
) -> list[str]:
  y = bar_bottom - lower_bar_height
  path = (
    f"M{x - 1:g} {y - 3:g} "
    f"C{x + 1:g} {y - 8:g} {x + 3:g} {y + 2:g} {x + 5:g} {y - 3:g} "
    f"S{x + 9:g} {y + 2:g} {x + BAR_WIDTH + 1:g} {y - 3:g}"
  )
  return [
    f'<path d="{path}" stroke="{group_background}" stroke-width="4" fill="none" stroke-linecap="round"/>',
    '<path '
    f'd="{path}" stroke="#2E231B" stroke-width="0.8" fill="none" '
    'stroke-linecap="round" opacity="0.65"/>',
  ]


def render_svg(
  groups: list[tuple[tuple[str, ...], list[float]]],
  crates: list[tuple[str, str]],
  *,
  height: int = HEIGHT,
  group_top: int = GROUP_TOP,
  group_height: int = GROUP_HEIGHT,
  bar_bottom: int = BAR_BOTTOM,
  lower_bar_height: int = LOWER_BAR_HEIGHT,
  upper_bar_height: int = UPPER_BAR_HEIGHT,
  label_top: int = LABEL_TOP,
  group_background: str = GROUP_BACKGROUND,
) -> str:
  group_area_width = len(groups) * GROUP_WIDTH + (len(groups) - 1) * GROUP_GAP
  legend_width = sum(LEGEND_SQUARE + 4 + text_width(name, LEGEND_FONT_SIZE) for name, _ in crates)
  legend_width += LEGEND_GAP * (len(crates) - 1)
  width = max(LEFT * 2 + group_area_width, LEFT * 2 + legend_width)
  group_left = (width - group_area_width) / 2
  group_xs = [group_left + i * (GROUP_WIDTH + GROUP_GAP) for i in range(len(groups))]
  bars_width = len(crates) * BAR_WIDTH + (len(crates) - 1) * BAR_GAP
  global_max = max(value for _, values in groups for value in values if value is not None)

  parts = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    (
      f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
      f'viewBox="0 0 {width} {height}">'
    ),
    f'<rect width="{width}" height="{height}" fill="{BACKGROUND}"/>',
    '<g shape-rendering="crispEdges">',
  ]

  legend_items = []
  legend_x = (width - legend_width) / 2
  for name, color in crates:
    legend_items.append((legend_x, name, color))
    legend_x += LEGEND_SQUARE + 4 + text_width(name, LEGEND_FONT_SIZE) + LEGEND_GAP
  for x, name, color in legend_items:
    parts.append(
      f'<rect x="{x:g}" y="{LEGEND_Y - LEGEND_SQUARE}" '
      f'width="{LEGEND_SQUARE}" height="{LEGEND_SQUARE}" fill="{color}"/>'
    )
    parts.append(
      f'<text x="{x + LEGEND_SQUARE + 4:g}" y="{LEGEND_Y:g}" text-anchor="start" '
      f'font-family="{FONT}" font-size="{LEGEND_FONT_SIZE}" fill="#2E231B">{esc(name)}</text>'
    )
  note = "All measurements are nanoseconds."
  if global_max > BREAK_VALUE:
    note = "All measurements are nanoseconds; squiggle marks compressed upper range."
  parts.append(text(width / 2, NOTE_Y, note, 9))

  for group_x, (labels, values) in zip(group_xs, groups):
    parts.append(
      f'<rect x="{group_x:g}" y="{group_top}" width="{GROUP_WIDTH}" '
      f'height="{group_height}" fill="{group_background}"/>'
    )

    bar_x = group_x + (GROUP_WIDTH - bars_width) / 2
    placed_labels = []
    for i, value in enumerate(values):
      if value is None:
        continue
      height = bar_height(value, global_max, lower_bar_height, upper_bar_height)
      x = bar_x + i * (BAR_WIDTH + BAR_GAP)
      y = bar_bottom - height
      color = crates[i][1]
      label = value_label(value)
      label_x = x + BAR_WIDTH / 2
      label_y = y - 4
      width_estimate = text_width(label, VALUE_FONT_SIZE)
      while any(
        abs(label_x - other_x) < (width_estimate + other_width) / 2 + 1
        and abs(label_y - other_y) < VALUE_FONT_SIZE + 3
        for other_x, other_y, other_width in placed_labels
      ):
        label_y -= VALUE_FONT_SIZE + 3
      placed_labels.append((label_x, label_y, width_estimate))
      parts.append(f'<rect x="{x:g}" y="{y:g}" width="{BAR_WIDTH}" height="{height}" fill="{color}"/>')
      if value > BREAK_VALUE:
        parts.extend(bar_break(x, bar_bottom, lower_bar_height, group_background))
      parts.append(text(label_x, label_y, label, VALUE_FONT_SIZE))

    center = group_x + GROUP_WIDTH / 2
    label_start = label_top
    for line, label in enumerate(labels):
      size = TARGET_LABEL_FONT_SIZE if "-" in label else LABEL_FONT_SIZE
      parts.append(text(center, label_start + line * LABEL_LINE_GAP, label, size))

  parts.append("</g>")
  parts.append("</svg>")
  return "\n".join(parts) + "\n"


def main() -> None:
  SVG_PATH.write_text(render_svg(GROUPS, CRATES))
  SIMPLE_SVG_PATH.write_text(
    render_svg(
      SIMPLE_GROUPS,
      SIMPLE_CRATES,
      height=SIMPLE_HEIGHT,
      group_top=SIMPLE_GROUP_TOP,
      group_height=SIMPLE_GROUP_HEIGHT,
      bar_bottom=SIMPLE_BAR_BOTTOM,
      lower_bar_height=SIMPLE_LOWER_BAR_HEIGHT,
      upper_bar_height=SIMPLE_UPPER_BAR_HEIGHT,
      label_top=SIMPLE_LABEL_TOP,
      group_background=BACKGROUND,
    )
  )
  rsvg_convert = shutil.which("rsvg-convert")
  if rsvg_convert is None:
    raise SystemExit("rsvg-convert is required to render benchmark.png")
  subprocess.run([rsvg_convert, "-o", str(PNG_PATH), str(SVG_PATH)], check=True)
  subprocess.run([rsvg_convert, "-o", str(SIMPLE_PNG_PATH), str(SIMPLE_SVG_PATH)], check=True)


if __name__ == "__main__":
  main()
