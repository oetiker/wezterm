#!/usr/bin/env bash
# corpus/images.sh — sixel, iTerm2 inline image, animated GIF.
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
