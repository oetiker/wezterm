#!/usr/bin/env bash
# corpus/blink-descender.sh — SGR 5 text whose INK ESCAPES ITS CELL.
#
# corpus/blink-text.sh blinks the word "BLINKING", which is all capitals: with
# JetBrains Mono at the harness's font_size = 12 no ASCII glyph leaves its cell
# at all (measured with `wezterm-gui ls-fonts --rasterize-ascii`: the tallest,
# `[`/`{`, is 17 px of sprite at bearing_y = 14 in a 22 px cell, so it sits
# comfortably inside).  That corpus therefore cannot show the Task 16 defect,
# and a round run only against it would report "no overhang" for the wrong
# reason.
#
# Two things are needed for a demonstration and BOTH are load-bearing:
#
#   1. descenders — g j y q p — so there is ink low in the cell, and
#   2. `line_height` BELOW 1.0 in the config, which shrinks the cell without
#      shrinking the glyphs (`RenderMetrics::scale_line_height` scales
#      cell_size; nothing rescales the rasterized sprite).  At line_height =
#      0.75 the descenders hang about 1 px past the bottom of the cell.
#
# Measured at line_height = 0.75, 1280x1024 on :20, 12 captures 0.25 s apart:
# the pre-fix binary animates screen rows 67..80 and leaves row 81 — 54 pixels
# of descender ink — bit-identical in every frame, while the fixed binary
# animates 67..81.  That frozen row is the defect.
set -euo pipefail
printf 'T16 OVERHANG DEMO — needs line_height < 1.0 in the config\n'
printf '\n'
printf '\033[5mgggjjjyyyqqqppp\033[0m\n'
printf '\n'
printf 'static filler line\n'
exec sleep 600
