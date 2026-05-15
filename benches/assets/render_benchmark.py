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
ELAPSED_SVG_PATH = ROOT / "benchmark-elapsed.svg"
ELAPSED_PNG_PATH = ROOT / "benchmark-elapsed.png"

BACKGROUND = "#FBF6EC"
FONT = "Avenir Next, Helvetica, Arial, sans-serif"

CRATES = [
  ("tach@0.2.0", "#D72D24"),
  ("quanta@0.12.6", "#5B6472"),
  ("fastant@0.1.11", "#4F6F6A"),
  ("minstant@0.1.7", "#8B5E3C"),
  ("std::Instant", "#9A8A3A"),
]

GROUPS = [
  (("Apple Silicon", "M1 MacBook Pro", "aarch64-apple-darwin"),
   [0.348, 4.585, 27.228, 27.288, 20.277]),
  (("AWS Graviton 3", "c7g.4xlarge", "aarch64-unknown-linux-gnu"),
   [6.675, 7.016, 41.684, 41.682, 32.510]),
  (("AWS Intel Burst", "t3.medium", "x86_64-unknown-linux-gnu"),
   [8.743, 13.321, 11.192, 9.395, 24.278]),
  (("Alpine on Metal", "m7i.metal-24xl", "x86_64-unknown-linux-musl"),
   [6.842, 7.105, 6.842, 6.842, 14.653]),
  (("AWS Lambda", "provided.al2023", "x86_64-unknown-linux-gnu"),
   [13.602, 23.344, 15.540, 56.930, 50.760]),
  (("GitHub Windows", "windows-2025", "x86_64-pc-windows-msvc"),
   [12.339, 12.432, 45.535, 45.518, 41.230]),
]

ELAPSED_CRATES = [
  ("tach: elapsed()", "#D72D24"),
  ("quanta: elapsed()", "#5B6472"),
  ("fastant: elapsed()", "#4F6F6A"),
  ("minstant: elapsed()", "#8B5E3C"),
  ("std: elapsed()", "#9A8A3A"),
]

ELAPSED_GROUPS = [
  (("Apple Silicon", "M1 MacBook Pro", "aarch64-apple-darwin"),
   [1.197, 9.163, 59.662, 59.640, 43.716]),
  (("AWS Graviton 3", "c7g.4xlarge", "aarch64-unknown-linux-gnu"),
   [13.354, 15.304, 87.806, 88.134, 72.580]),
  (("AWS Intel Burst", "t3.medium", "x86_64-unknown-linux-gnu"),
   [18.944, 28.179, 31.027, 31.087, 53.479]),
  (("Alpine on Metal", "m7i.metal-24xl", "x86_64-unknown-linux-musl"),
   [13.684, 17.511, 21.399, 21.412, 32.579]),
  (("AWS Lambda", "provided.al2023", "x86_64-unknown-linux-gnu"),
   [31.929, 50.860, 51.788, 135.750, 106.361]),
  (("GitHub Windows", "windows-2025", "x86_64-pc-windows-msvc"),
   [24.695, 25.477, 104.510, 104.440, 85.678]),
]

BAR_GAP = 4
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
TITLE_Y = 22
TITLE_FONT_SIZE = 16


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


def bar_break(x: float, bar_width: int) -> list[str]:
  y = BAR_BOTTOM - LOWER_BAR_HEIGHT
  path = (
    f"M{x - 1:g} {y - 3:g} "
    f"C{x + 1:g} {y - 8:g} {x + 3:g} {y + 2:g} {x + 5:g} {y - 3:g} "
    f"S{x + 9:g} {y + 2:g} {x + bar_width + 1:g} {y - 3:g}"
  )
  return [
    f'<path d="{path}" stroke="{BACKGROUND}" stroke-width="4" fill="none" stroke-linecap="round"/>',
    '<path '
    f'd="{path}" stroke="#2E231B" stroke-width="0.8" fill="none" '
    'stroke-linecap="round" opacity="0.65"/>',
  ]


def render_svg(groups, crates, title, bar_width, group_width) -> str:
  group_area_width = len(groups) * group_width + (len(groups) - 1) * GROUP_GAP
  legend_width = sum(LEGEND_SQUARE + 4 + text_width(name, LEGEND_FONT_SIZE) for name, _ in crates)
  legend_width += LEGEND_GAP * (len(crates) - 1)
  width = max(LEFT * 2 + group_area_width, LEFT * 2 + legend_width)
  group_left = (width - group_area_width) / 2
  group_xs = [group_left + i * (group_width + GROUP_GAP) for i in range(len(groups))]
  bars_width = len(crates) * bar_width + (len(crates) - 1) * BAR_GAP
  global_max = max(value for _, values in groups for value in values if value is not None)

  parts = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    (
      f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{HEIGHT}" '
      f'viewBox="0 0 {width} {HEIGHT}">'
    ),
    f'<rect width="{width}" height="{HEIGHT}" fill="{BACKGROUND}"/>',
    '<g shape-rendering="crispEdges">',
    text(width / 2, TITLE_Y, title, TITLE_FONT_SIZE),
  ]

  max_label_lines = max(len(labels) for labels, _ in groups)
  labels_bottom = LABEL_TOP + (max_label_lines - 1) * LABEL_LINE_GAP
  legend_y = labels_bottom + LABEL_TO_LEGEND_GAP
  note_y = legend_y + LEGEND_TO_NOTE_GAP
  legend_items = []
  legend_x = (width - legend_width) / 2
  for name, color in crates:
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

  for group_x, (labels, values) in zip(group_xs, groups):
    bar_x = group_x + (group_width - bars_width) / 2
    placed_labels = []
    for i, value in enumerate(values):
      if value is None:
        continue
      height = bar_height(value, global_max)
      x = bar_x + i * (bar_width + BAR_GAP)
      y = BAR_BOTTOM - height
      color = crates[i][1]
      label = value_label(value)
      label_x = x + bar_width / 2
      label_y = y - 4
      width_estimate = text_width(label, VALUE_FONT_SIZE)
      while any(
        abs(label_x - other_x) < (width_estimate + other_width) / 2 + 1
        and abs(label_y - other_y) < VALUE_FONT_SIZE + 3
        for other_x, other_y, other_width in placed_labels
      ):
        label_y -= VALUE_FONT_SIZE + 3
      placed_labels.append((label_x, label_y, width_estimate))
      parts.append(f'<rect x="{x:g}" y="{y:g}" width="{bar_width}" height="{height}" fill="{color}"/>')
      if value > BREAK_VALUE:
        parts.extend(bar_break(x, bar_width))
      parts.append(text(label_x, label_y, label, VALUE_FONT_SIZE))

    center = group_x + group_width / 2
    for line, label in enumerate(labels):
      size = TARGET_LABEL_FONT_SIZE if looks_like_target_triple(label) else LABEL_FONT_SIZE
      parts.append(text(center, LABEL_TOP + line * LABEL_LINE_GAP, label, size))

  parts.append("</g>")
  parts.append("</svg>")
  return "\n".join(parts) + "\n"


def main() -> None:
  SVG_PATH.write_text(render_svg(GROUPS, CRATES, "Instant::now()", 8, 94))
  ELAPSED_SVG_PATH.write_text(
    render_svg(ELAPSED_GROUPS, ELAPSED_CRATES, "Instant: now() + elapsed()", 16, 180)
  )
  rsvg_convert = shutil.which("rsvg-convert")
  if rsvg_convert is None:
    raise SystemExit("rsvg-convert is required to render the benchmark PNGs")
  subprocess.run([rsvg_convert, "--zoom", "2", "-o", str(PNG_PATH), str(SVG_PATH)], check=True)
  subprocess.run(
    [rsvg_convert, "--zoom", "2", "-o", str(ELAPSED_PNG_PATH), str(ELAPSED_SVG_PATH)],
    check=True,
  )


if __name__ == "__main__":
  main()
