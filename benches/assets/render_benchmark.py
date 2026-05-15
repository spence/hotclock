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
GRID_SVG_PATH = ROOT / "benchmark-grid.svg"
GRID_PNG_PATH = ROOT / "benchmark-grid.png"

BACKGROUND = "#FBF6EC"
FONT = "Avenir Next, Helvetica, Arial, sans-serif"
MONO = "SFMono-Regular, Menlo, Consolas, monospace"
TEXT_FG = "#2E231B"
MUTED_FG = "#7A6E60"

CRATES = [
  ("tach@0.2.0", "#D72D24"),
  ("quanta@0.12.6", "#5B6472"),
  ("fastant@0.1.11", "#4F6F6A"),
  ("minstant@0.1.7", "#8B5E3C"),
  ("std", "#9A8A3A"),
]

NOW_GROUPS = [
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
LEGEND_GAP = 18
LEGEND_SQUARE = 6
BAR_AREA_HEIGHT = 230
VALUE_FONT_SIZE = 7
LABEL_FONT_SIZE = 11
LABEL_LINE_GAP = 13
LABEL_LINES = 3
LABEL_GAP = 24
TARGET_LABEL_FONT_SIZE = 7
LEGEND_FONT_SIZE = 12
TITLE_Y_OFFSET = 22
TITLE_FONT_SIZE = 16
TITLE_TO_BARS_GAP = 8
PANEL_GAP = 30
LABEL_TO_LEGEND_GAP = 30
LEGEND_TO_NOTE_GAP = 20
NOTE_TO_BOTTOM_GAP = 12

BAR_WIDTH = 16
GROUP_WIDTH = 180


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
  return max(2, round(value / global_max * BAR_AREA_HEIGHT))


def render_panel(groups, crates, title, bar_width, group_width, width, panel_top) -> list[str]:
  parts = []
  group_area_width = len(groups) * group_width + (len(groups) - 1) * GROUP_GAP
  group_left = (width - group_area_width) / 2
  group_xs = [group_left + i * (group_width + GROUP_GAP) for i in range(len(groups))]
  bars_width = len(crates) * bar_width + (len(crates) - 1) * BAR_GAP
  global_max = max(value for _, values in groups for value in values if value is not None)

  title_y = panel_top + TITLE_Y_OFFSET
  bar_bottom = title_y + TITLE_TO_BARS_GAP + BAR_AREA_HEIGHT
  label_top = bar_bottom + LABEL_GAP

  parts.append(text(width / 2, title_y, title, TITLE_FONT_SIZE))

  for group_x, (labels, values) in zip(group_xs, groups):
    bar_x = group_x + (group_width - bars_width) / 2
    placed_labels = []
    for i, value in enumerate(values):
      if value is None:
        continue
      h = bar_height(value, global_max)
      x = bar_x + i * (bar_width + BAR_GAP)
      y = bar_bottom - h
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
      parts.append(f'<rect x="{x:g}" y="{y:g}" width="{bar_width}" height="{h}" fill="{color}"/>')
      parts.append(text(label_x, label_y, label, VALUE_FONT_SIZE))

    center = group_x + group_width / 2
    for line, label in enumerate(labels):
      size = TARGET_LABEL_FONT_SIZE if looks_like_target_triple(label) else LABEL_FONT_SIZE
      parts.append(text(center, label_top + line * LABEL_LINE_GAP, label, size))

  return parts


def render_combined_svg(now_groups, elapsed_groups, crates, bar_width, group_width) -> str:
  group_area_width = len(now_groups) * group_width + (len(now_groups) - 1) * GROUP_GAP
  legend_width = sum(LEGEND_SQUARE + 4 + text_width(name, LEGEND_FONT_SIZE) for name, _ in crates)
  legend_width += LEGEND_GAP * (len(crates) - 1)
  width = max(LEFT * 2 + group_area_width, LEFT * 2 + legend_width)

  labels_block = LABEL_GAP + (LABEL_LINES - 1) * LABEL_LINE_GAP
  panel_content = TITLE_Y_OFFSET + TITLE_TO_BARS_GAP + BAR_AREA_HEIGHT + labels_block

  top_panel_top = 0
  bottom_panel_top = top_panel_top + panel_content + PANEL_GAP
  bottom_panel_bottom = bottom_panel_top + panel_content
  legend_y = bottom_panel_bottom + LABEL_TO_LEGEND_GAP
  note_y = legend_y + LEGEND_TO_NOTE_GAP
  height = note_y + NOTE_TO_BOTTOM_GAP

  parts = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    (
      f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
      f'viewBox="0 0 {width} {height}">'
    ),
    f'<rect width="{width}" height="{height}" fill="{BACKGROUND}"/>',
    '<g shape-rendering="crispEdges">',
  ]

  parts.extend(
    render_panel(now_groups, crates, "now()", bar_width, group_width, width, top_panel_top)
  )
  parts.extend(
    render_panel(
      elapsed_groups,
      crates,
      "now() + elapsed()",
      bar_width,
      group_width,
      width,
      bottom_panel_top,
    )
  )

  legend_x = (width - legend_width) / 2
  for name, color in crates:
    parts.append(
      f'<rect x="{legend_x:g}" y="{legend_y - LEGEND_SQUARE}" '
      f'width="{LEGEND_SQUARE}" height="{LEGEND_SQUARE}" fill="{color}"/>'
    )
    parts.append(
      f'<text x="{legend_x + LEGEND_SQUARE + 4:g}" y="{legend_y:g}" text-anchor="start" '
      f'font-family="{FONT}" font-size="{LEGEND_FONT_SIZE}" fill="#2E231B">{esc(name)}</text>'
    )
    legend_x += LEGEND_SQUARE + 4 + text_width(name, LEGEND_FONT_SIZE) + LEGEND_GAP

  parts.append(text(width / 2, note_y, "All measurements are nanoseconds.", 8, italic=True))

  parts.append("</g>")
  parts.append("</svg>")
  return "\n".join(parts) + "\n"


GRID_COLS = 2
GRID_CELL_W = 820
GRID_CELL_H = 440
GRID_COL_GAP = 36
GRID_ROW_GAP = 48
GRID_MARGIN = 40
GRID_CELL_PAD = 30
GRID_TITLE_FONT_SIZE = 38
GRID_SUBTITLE_FONT_SIZE = 24
GRID_LABEL_FONT_SIZE = 24
GRID_VALUE_FONT_SIZE = 24
GRID_ROW_HEIGHT = 54
GRID_BAR_HEIGHT = 30
GRID_CRATE_LABEL_WIDTH = 170
GRID_VALUE_RESERVE = 180
GRID_LIGHTEN = 0.62


def lighten(hex_color: str, amount: float) -> str:
  h = hex_color.lstrip("#")
  bh = BACKGROUND.lstrip("#")
  fr, fg, fb = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
  br, bg, bb = int(bh[0:2], 16), int(bh[2:4], 16), int(bh[4:6], 16)
  return (
    f"#{int(fr + (br - fr) * amount):02x}"
    f"{int(fg + (bg - fg) * amount):02x}"
    f"{int(fb + (bb - fb) * amount):02x}"
  )


def styled_text(
  x: float,
  y: float,
  value: str,
  size: int,
  family: str = FONT,
  color: str = TEXT_FG,
  anchor: str = "middle",
  weight: str | None = None,
) -> str:
  weight_attr = f' font-weight="{weight}"' if weight else ""
  return (
    f'<text x="{x:g}" y="{y:g}" text-anchor="{anchor}" '
    f'font-family="{family}" font-size="{size}"{weight_attr} '
    f'fill="{color}">{esc(value)}</text>'
  )


def crate_short(name: str) -> str:
  return name.split("@")[0]


def render_grid_cell(now_group, elapsed_group, crates, x0: float, y0: float) -> list[str]:
  (title, instance, triple), now_vals = now_group
  _, elapsed_vals = elapsed_group

  parts = []
  title_x = x0 + GRID_CELL_PAD
  title_y = y0 + GRID_CELL_PAD + GRID_TITLE_FONT_SIZE - 2
  parts.append(
    styled_text(title_x, title_y, title, GRID_TITLE_FONT_SIZE, anchor="start", weight="600")
  )

  subtitle = f"{instance} · {triple}"
  subtitle_y = title_y + GRID_SUBTITLE_FONT_SIZE + 8
  parts.append(
    styled_text(
      title_x, subtitle_y, subtitle, GRID_SUBTITLE_FONT_SIZE,
      family=MONO, color=MUTED_FG, anchor="start",
    )
  )

  bar_area_left = title_x + GRID_CRATE_LABEL_WIDTH + 10
  bar_area_right = x0 + GRID_CELL_W - GRID_CELL_PAD - GRID_VALUE_RESERVE
  bar_area_width = bar_area_right - bar_area_left
  cell_max = max(elapsed_vals)

  rows_top = subtitle_y + 18
  for i, ((crate_full, color), now_v, elapsed_v) in enumerate(zip(crates, now_vals, elapsed_vals)):
    row_top = rows_top + i * GRID_ROW_HEIGHT
    bar_y = row_top + (GRID_ROW_HEIGHT - GRID_BAR_HEIGHT) / 2
    text_baseline = row_top + GRID_ROW_HEIGHT / 2 + 4

    parts.append(
      styled_text(
        title_x, text_baseline, crate_short(crate_full),
        GRID_LABEL_FONT_SIZE, family=MONO, anchor="start",
      )
    )

    light_color = lighten(color, GRID_LIGHTEN)
    elapsed_w = max(1.0, elapsed_v / cell_max * bar_area_width)
    now_w = max(1.0, now_v / cell_max * bar_area_width)
    parts.append(
      f'<rect x="{bar_area_left:g}" y="{bar_y:g}" '
      f'width="{elapsed_w:g}" height="{GRID_BAR_HEIGHT}" fill="{light_color}"/>'
    )
    parts.append(
      f'<rect x="{bar_area_left:g}" y="{bar_y:g}" '
      f'width="{now_w:g}" height="{GRID_BAR_HEIGHT}" fill="{color}"/>'
    )

    value_text = f"{value_label(now_v)} / {value_label(elapsed_v)}"
    parts.append(
      styled_text(
        bar_area_left + elapsed_w + 6, text_baseline, value_text,
        GRID_VALUE_FONT_SIZE, family=MONO, anchor="start",
      )
    )

  return parts


def render_grid_svg(now_groups, elapsed_groups, crates) -> str:
  rows = (len(now_groups) + GRID_COLS - 1) // GRID_COLS
  width = GRID_COLS * GRID_CELL_W + (GRID_COLS - 1) * GRID_COL_GAP + 2 * GRID_MARGIN
  height = rows * GRID_CELL_H + (rows - 1) * GRID_ROW_GAP + 2 * GRID_MARGIN

  parts = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    (
      f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
      f'viewBox="0 0 {width} {height}">'
    ),
    f'<rect width="{width}" height="{height}" fill="{BACKGROUND}"/>',
    '<g shape-rendering="crispEdges">',
  ]

  for i, (ng, eg) in enumerate(zip(now_groups, elapsed_groups)):
    col = i % GRID_COLS
    row = i // GRID_COLS
    x = GRID_MARGIN + col * (GRID_CELL_W + GRID_COL_GAP)
    y = GRID_MARGIN + row * (GRID_CELL_H + GRID_ROW_GAP)
    parts.extend(render_grid_cell(ng, eg, crates, x, y))

  parts.append("</g>")
  parts.append("</svg>")
  return "\n".join(parts) + "\n"


def main() -> None:
  SVG_PATH.write_text(render_combined_svg(NOW_GROUPS, ELAPSED_GROUPS, CRATES, BAR_WIDTH, GROUP_WIDTH))
  GRID_SVG_PATH.write_text(render_grid_svg(NOW_GROUPS, ELAPSED_GROUPS, CRATES))
  rsvg_convert = shutil.which("rsvg-convert")
  if rsvg_convert is None:
    raise SystemExit("rsvg-convert is required to render the benchmark PNGs")
  subprocess.run([rsvg_convert, "--zoom", "2", "-o", str(PNG_PATH), str(SVG_PATH)], check=True)
  subprocess.run([rsvg_convert, "-o", str(GRID_PNG_PATH), str(GRID_SVG_PATH)], check=True)


if __name__ == "__main__":
  main()
