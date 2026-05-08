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

BACKGROUND = "#FBF6EC"
GROUP_BACKGROUND = "#FFF8E8"
FONT = "Avenir Next, Helvetica, Arial, sans-serif"

CRATES = [
  ("tach@0.2.0", "#075A4A"),
  ("quanta@0.12.6", "#D48A22"),
  ("minstant@0.1.7", "#8F2E2E"),
  ("fastant@0.1.11", "#4B6F78"),
  ("std@1.91", "#B95032"),
]

GROUPS = [
  (("AWS t3 KVM", "x86_64-musl"), [8.711, 13.254, 9.356, 9.356, 24.249]),
  (("AWS m7i metal", "x86_64-musl"), [6.841, 7.130, 6.841, 6.841, 14.734]),
  (("AWS t3 KVM", "x86-musl"), [13.552, 69.312, 14.363, 14.154, 66.458]),
  (("AWS m7i metal", "x86-musl"), [6.841, 23.066, 6.841, 6.841, 22.743]),
  (("Docker", "x86_64-gnu"), [15.394, 25.079, 39.050, 22.066, 28.070]),
  (("Docker", "x86-gnu"), [25.789, 253.702, 323.398, 324.264, 222.623]),
  (("Docker", "aarch64-gnu"), [0.330, 4.466, 27.203, 27.275, 20.222]),
  (("Docker", "riscv64-gnu"), [59.584, 215.296, 271.262, 271.241, 185.151]),
  (("macOS", "aarch64"), [0.330, 4.620, 25.700, 25.700, 18.400]),
]

BAR_WIDTH = 8
BAR_GAP = 4
GROUP_WIDTH = 81
GROUP_GAP = 20
LEFT = 6
HEIGHT = 218
LEGEND_Y = 15
LEGEND_GAP = 18
LEGEND_SQUARE = 6
NOTE_Y = 30
GROUP_TOP = 42
GROUP_HEIGHT = 164
BAR_BOTTOM = 166
MAX_BAR_HEIGHT = 86
VALUE_FONT_SIZE = 7
LABEL_FONT_SIZE = 12
LEGEND_FONT_SIZE = LABEL_FONT_SIZE


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


def render_svg() -> str:
  width = LEFT * 2 + len(GROUPS) * GROUP_WIDTH + (len(GROUPS) - 1) * GROUP_GAP
  group_xs = [LEFT + i * (GROUP_WIDTH + GROUP_GAP) for i in range(len(GROUPS))]
  bars_width = len(CRATES) * BAR_WIDTH + (len(CRATES) - 1) * BAR_GAP

  parts = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    (
      f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{HEIGHT}" '
      f'viewBox="0 0 {width} {HEIGHT}">'
    ),
    f'<rect width="{width}" height="{HEIGHT}" fill="{BACKGROUND}"/>',
    '<g shape-rendering="crispEdges">',
  ]

  legend_items = []
  legend_width = sum(LEGEND_SQUARE + 4 + text_width(name, LEGEND_FONT_SIZE) for name, _ in CRATES)
  legend_width += LEGEND_GAP * (len(CRATES) - 1)
  legend_x = (width - legend_width) / 2
  for name, color in CRATES:
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
  parts.append(text(width / 2, NOTE_Y, "All measurements are nanoseconds.", 9))

  for group_x, (labels, values) in zip(group_xs, GROUPS):
    parts.append(
      f'<rect x="{group_x}" y="{GROUP_TOP}" width="{GROUP_WIDTH}" '
      f'height="{GROUP_HEIGHT}" fill="{GROUP_BACKGROUND}"/>'
    )

    bar_x = group_x + (GROUP_WIDTH - bars_width) / 2
    group_max = max(values)
    placed_labels = []
    for i, value in enumerate(values):
      height = max(2, round(value / group_max * MAX_BAR_HEIGHT))
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
      parts.append(text(label_x, label_y, label, VALUE_FONT_SIZE))

    center = group_x + GROUP_WIDTH / 2
    parts.append(text(center, 185, labels[0], LABEL_FONT_SIZE))
    parts.append(text(center, 199, labels[1], LABEL_FONT_SIZE))

  parts.append("</g>")
  parts.append("</svg>")
  return "\n".join(parts) + "\n"


def main() -> None:
  SVG_PATH.write_text(render_svg())
  rsvg_convert = shutil.which("rsvg-convert")
  if rsvg_convert is None:
    raise SystemExit("rsvg-convert is required to render benchmark.png")
  subprocess.run([rsvg_convert, "-o", str(PNG_PATH), str(SVG_PATH)], check=True)


if __name__ == "__main__":
  main()
