#!/usr/bin/env python3
"""Render the README benchmark chart with fixed pixel geometry."""

from __future__ import annotations

import html
import shutil
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parent
SVG_PATH = ROOT / "benchmark-instant.svg"
PNG_PATH = ROOT / "benchmark-instant.png"

BACKGROUND = "#FBF6EC"
FONT = "Avenir Next, Helvetica, Arial, sans-serif"

CRATES = [
  ("tach@0.2.0", "#D72D24"),
  ("quanta@0.12.6", "#5B6472"),
  ("fastant@0.1.11", "#4F6F6A"),
  ("minstant@0.1.7", "#8B5E3C"),
  ("std::Instant", "#9A8A3A"),
]

TITLE = "Instant::now()"
TITLE_Y = 22
TITLE_FONT_SIZE = 16

GROUPS = [
  (("Apple Silicon", "M1 MacBook Pro", "aarch64-apple-darwin"),
   [0.331, 7.591, 43.722, 43.259, 31.658]),
  (("AWS Graviton 3", "c7g.4xlarge", "aarch64-unknown-linux-gnu"),
   [6.673, 7.062, 38.939, 39.578, 31.463]),
  (("AWS Intel Burst", "t3.medium", "x86_64-unknown-linux-gnu"),
   [8.762, 13.314, 9.408, 9.408, 24.059]),
  (("Alpine on Metal", "m7i.metal-24xl", "x86_64-unknown-linux-musl"),
   [14.316, 17.074, 14.625, 14.625, 25.865]),
  (("AWS Lambda", "provided.al2023", "x86_64-unknown-linux-gnu"),
   [9.556, 14.102, 10.206, 44.386, 29.919]),
  (("GitHub macOS", "macos-15-intel", "x86_64-apple-darwin"),
   [6.076, 81.234, 39.790, 40.199, 38.587]),
  (("GitHub Windows", "windows-2025", "x86_64-pc-windows-msvc"),
   [11.245, 11.670, 40.925, 40.926, 38.396]),
]

BAR_WIDTH = 8
BAR_GAP = 4
GROUP_WIDTH = 94
GROUP_GAP = 20
LEFT = 6
HEIGHT = 375
LEGEND_GAP = 18
LEGEND_SQUARE = 6
BAR_BOTTOM = 256
LOWER_BAR_HEIGHT = 230
UPPER_BAR_HEIGHT = 0
BREAK_VALUE = 82.0
VALUE_FONT_SIZE = 7
LABEL_FONT_SIZE = 11
LABEL_LINE_GAP = 13
LABEL_TOP = BAR_BOTTOM + 24
LABEL_TO_LEGEND_GAP = 30
LEGEND_TO_NOTE_GAP = 20
TARGET_LABEL_FONT_SIZE = 7
LEGEND_FONT_SIZE = 12


def looks_like_target_triple(label: str) -> bool:
  return label.count("-") >= 2 or any(
    marker in label for marker in ("linux", "darwin", "windows", "unknown")
  )


def value_label(value: float) -> str:
  if value >= 100:
    return f"{value:.0f}"
  if value >= 10:
    return f"{value:.1f}"
  return f"{value:.2f}"


def esc(value: str) -> str:
  return html.escape(value, quote=True)


def text(
  x: float, y: float, value: str, size: int, anchor: str = "middle", italic: bool = False
) -> str:
  style = ' font-style="italic"' if italic else ""
  return (
    f'<text x="{x:g}" y="{y:g}" text-anchor="{anchor}" '
    f'font-family="{FONT}" font-size="{size}"{style} fill="#2E231B">{esc(value)}</text>'
  )


def text_width(value: str, size: int) -> float:
  return len(value) * size * 0.56


def bar_height(value: float, global_max: float) -> int:
  if value <= BREAK_VALUE:
    return max(2, round(value / BREAK_VALUE * LOWER_BAR_HEIGHT))
  upper = (value - BREAK_VALUE) / (global_max - BREAK_VALUE) * UPPER_BAR_HEIGHT
  return round(LOWER_BAR_HEIGHT + upper)


def bar_break(x: float) -> list[str]:
  y = BAR_BOTTOM - LOWER_BAR_HEIGHT
  path = (
    f"M{x - 1:g} {y - 3:g} "
    f"C{x + 1:g} {y - 8:g} {x + 3:g} {y + 2:g} {x + 5:g} {y - 3:g} "
    f"S{x + 9:g} {y + 2:g} {x + BAR_WIDTH + 1:g} {y - 3:g}"
  )
  return [
    f'<path d="{path}" stroke="{BACKGROUND}" stroke-width="4" fill="none" stroke-linecap="round"/>',
    '<path '
    f'd="{path}" stroke="#2E231B" stroke-width="0.8" fill="none" '
    'stroke-linecap="round" opacity="0.65"/>',
  ]


def render_svg() -> str:
  group_area_width = len(GROUPS) * GROUP_WIDTH + (len(GROUPS) - 1) * GROUP_GAP
  legend_width = sum(LEGEND_SQUARE + 4 + text_width(name, LEGEND_FONT_SIZE) for name, _ in CRATES)
  legend_width += LEGEND_GAP * (len(CRATES) - 1)
  width = max(LEFT * 2 + group_area_width, LEFT * 2 + legend_width)
  group_left = (width - group_area_width) / 2
  group_xs = [group_left + i * (GROUP_WIDTH + GROUP_GAP) for i in range(len(GROUPS))]
  bars_width = len(CRATES) * BAR_WIDTH + (len(CRATES) - 1) * BAR_GAP
  global_max = max(value for _, values in GROUPS for value in values if value is not None)

  parts = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    (
      f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{HEIGHT}" '
      f'viewBox="0 0 {width} {HEIGHT}">'
    ),
    f'<rect width="{width}" height="{HEIGHT}" fill="{BACKGROUND}"/>',
    '<g shape-rendering="crispEdges">',
    text(width / 2, TITLE_Y, TITLE, TITLE_FONT_SIZE),
  ]

  max_label_lines = max(len(labels) for labels, _ in GROUPS)
  labels_bottom = LABEL_TOP + (max_label_lines - 1) * LABEL_LINE_GAP
  legend_y = labels_bottom + LABEL_TO_LEGEND_GAP
  note_y = legend_y + LEGEND_TO_NOTE_GAP
  legend_items = []
  legend_x = (width - legend_width) / 2
  for name, color in CRATES:
    legend_items.append((legend_x, name, color))
    legend_x += LEGEND_SQUARE + 4 + text_width(name, LEGEND_FONT_SIZE) + LEGEND_GAP
  for x, name, color in legend_items:
    parts.append(
      f'<rect x="{x:g}" y="{legend_y - LEGEND_SQUARE}" '
      f'width="{LEGEND_SQUARE}" height="{LEGEND_SQUARE}" fill="{color}"/>'
    )
    parts.append(
      f'<text x="{x + LEGEND_SQUARE + 4:g}" y="{legend_y:g}" text-anchor="start" '
      f'font-family="{FONT}" font-size="{LEGEND_FONT_SIZE}" fill="#2E231B">{esc(name)}</text>'
    )
  note = "All measurements are nanoseconds."
  if global_max > BREAK_VALUE:
    note = "All measurements are nanoseconds; squiggle marks compressed upper range."
  parts.append(text(width / 2, note_y, note, 8, italic=True))

  for group_x, (labels, values) in zip(group_xs, GROUPS):
    bar_x = group_x + (GROUP_WIDTH - bars_width) / 2
    placed_labels = []
    for i, value in enumerate(values):
      if value is None:
        continue
      height = bar_height(value, global_max)
      x = bar_x + i * (BAR_WIDTH + BAR_GAP)
      y = BAR_BOTTOM - height
      color = CRATES[i][1]
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
        parts.extend(bar_break(x))
      parts.append(text(label_x, label_y, label, VALUE_FONT_SIZE))

    center = group_x + GROUP_WIDTH / 2
    for line, label in enumerate(labels):
      size = TARGET_LABEL_FONT_SIZE if looks_like_target_triple(label) else LABEL_FONT_SIZE
      parts.append(text(center, LABEL_TOP + line * LABEL_LINE_GAP, label, size))

  parts.append("</g>")
  parts.append("</svg>")
  return "\n".join(parts) + "\n"


def main() -> None:
  SVG_PATH.write_text(render_svg())
  rsvg_convert = shutil.which("rsvg-convert")
  if rsvg_convert is None:
    raise SystemExit("rsvg-convert is required to render benchmark-instant.png")
  subprocess.run([rsvg_convert, "--zoom", "2", "-o", str(PNG_PATH), str(SVG_PATH)], check=True)


if __name__ == "__main__":
  main()
