#!/usr/bin/env bash
# corpus/images.sh — sixel, iTerm2 inline image, animated GIF.
#
# Round-1 review (task-4-review.md, CRIT-1/CRIT-2) found that the brief's
# original verbatim corpus requested every image at its own native pixel
# size, which makes purecpu.rs:346's `blit_w = tex_w.min(dest_w)` a no-op —
# the corpus could not reach the predicted crop-instead-of-scale defect at
# all. Per human ruling recorded in the plan's Global Constraints (commit
# cdf403f), the corpus may be extended past the brief's verbatim text when it
# cannot exercise the behaviour under test. The blocks below marked "(added
# in fix round 1)" are that extension; the original three blocks are
# untouched.
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

# (added in fix round 1) iTerm2 requested at a non-native display size, so
# the per-cell texture rect and destination rect differ and a real rescale
# is required of whichever backend gets it right.
printf '\nITERM2 SCALED (200x132px, native is 64x64):\n'
printf '\033]1337;File=inline=1;width=200px;height=132px:%s\a\n' "$b64"
printf '\nITERM2 CELLS (20x4 cells):\n'
printf '\033]1337;File=inline=1;width=20;height=4:%s\a\n' "$b64"

# (added in fix round 1) sixel has no display-size escape parameter of its
# own — a sixel image is always requested at native pixel size — so its only
# possible route to the same class of defect is AllowImage::Scale
# (paint.rs:72-76): the atlas downscales the *cached sprite* itself when the
# texture atlas fails to grow to fit it. That only fires when growing the
# atlas exceeds the backend's texture-size ceiling. Attempted here with a
# moderately large (300x300) sixel; see the Task 4 fix report for whether
# this reproduced anything and why a harness-practical size cannot reach the
# GPU's actual ceiling (GL_MAX_TEXTURE_SIZE=16384 measured on this machine).
convert -size 300x300 gradient:red-blue "$TMP/big.png"
printf '\nSIXEL OVERSIZED (300x300):\n'
convert "$TMP/big.png" sixel:- || printf '(sixel conversion failed)\n'

printf '\nEND\n'
exec sleep 600
