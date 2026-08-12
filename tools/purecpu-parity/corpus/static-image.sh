#!/usr/bin/env bash
# corpus/static-image.sh — one large STATIC sixel, drawn once, never updated.
#
# Added for the measurement round, on the Task 10 review's instruction (F5b).
# The claim under test is the source comment's "a still image costs nothing
# here, forever". That is true in RECTS — a static image marks no dirty region
# — and it is the CPU cost that the comment does not cover:
# `attrs.images()` allocates a Vec and deep-clones every ImageCell (it is not
# a borrow), plus a HashMap lookup and a mutex acquisition per image cell, per
# frame, for as long as anything holds the animation timer open.
#
# So this corpus is deliberately paired with a config that keeps the timer
# open by other means (a blinking cursor does, by default). A static image
# with NOTHING else animating would let the idle early exit fire and would
# measure nothing — that pairing is the whole experiment, and it belongs in
# the driver, not here.
#
# The image is sized to cover most of a 100x30 window rather than to a
# specific pixel count: what scales the per-frame cost is the number of image
# CELLS, so "most of the window" is the property that matters and it is
# recorded in the report alongside the measured cell count.
set -euo pipefail
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# 880x560 covers roughly the whole text area at font_size 12 on this display.
convert -size 880x560 gradient:red-blue "$TMP/big.png"

printf 'STATIC IMAGE CASE — one sixel, drawn once, never redrawn\n'
convert "$TMP/big.png" sixel:- || printf '(sixel conversion failed)\n'
printf '\nEND\n'
exec sleep 600
