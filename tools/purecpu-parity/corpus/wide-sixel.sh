#!/usr/bin/env bash
# corpus/wide-sixel.sh — a sixel wide enough that the atlas must grow past
# GL_MAX_TEXTURE_SIZE in one dimension, to exercise sixel's only reachable
# route to the crop/rescale mismatch: AllowImage::Scale
# (wezterm-gui/src/termwindow/render/paint.rs:66-90).
#
# (added in Task 4 fix round 2, CRIT-1/NEW-3) Round 1 argued this route was
# impractical to trigger because it assumed a square image needs to exceed
# the texture-size ceiling in both dimensions. It doesn't: the atlas only
# has to grow past the ceiling in ONE dimension, and a wide-and-short
# gradient sixel-encodes to a couple of KB regardless of pixel width (long
# runs of near-identical colour compress trivially under sixel's RLE). A
# 17000x64 gradient reaches that in practice.
set -euo pipefail
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

convert -size 17000x64 gradient:red-blue "$TMP/wide.png"

printf 'WIDE SIXEL:\n'
convert "$TMP/wide.png" sixel:- || printf '(sixel conversion failed)\n'
printf '\nEND\n'
exec sleep 600
