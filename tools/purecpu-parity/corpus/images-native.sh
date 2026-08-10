#!/usr/bin/env bash
# corpus/images-native.sh — the original three native-size image blocks from
# corpus/images.sh (sixel, iTerm2, animated GIF), isolated.
#
# (added in Task 4 fix round 2, NEW-1) corpus/images.sh now has enough
# content after these three blocks (added in fix round 1, CRIT-1) that by
# settle-time the terminal has scrolled them off the top of the window —
# `compare-case.sh images corpus/images.sh` no longer reproduces the
# native-size AE=0/PAE=0 figures cited in the matrix, even though those
# figures are correct (they were measured against this exact content before
# the corpus grew). This script is that content, byte-for-byte, with nothing
# after it, so the native-size sub-case has its own reproducible regeneration
# command instead of silently rotting when the main corpus grows again.
set -euo pipefail
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

convert -size 64x64 gradient:red-blue "$TMP/img.png"
convert -size 32x32 xc:green -delay 20 -size 32x32 xc:yellow "$TMP/anim.gif"

printf 'SIXEL:\n'
convert "$TMP/img.png" sixel:- || printf '(sixel conversion failed)\n'
printf '\nITERM2:\n'
b64=$(base64 -w0 < "$TMP/img.png")
printf '\033]1337;File=inline=1;width=64px;height=64px:%s\a\n' "$b64"
printf '\nANIMATED GIF:\n'
b64g=$(base64 -w0 < "$TMP/anim.gif")
printf '\033]1337;File=inline=1;width=32px;height=32px:%s\a\n' "$b64g"
printf '\nEND\n'
exec sleep 600
