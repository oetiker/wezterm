#!/usr/bin/env bash
# corpus/gif-loop.sh — the animated GIF from corpus/images.sh, isolated.
#
# (added in Task 4 fix round 1, CRIT-2) corpus/images.sh now also emits
# non-native-size iTerm2 images and an oversized sixel (added for CRIT-1)
# after this GIF block; by the time that corpus settles, those later blocks
# have scrolled the GIF cell off the top of the window, and
# compare-case.sh's capture_settled only ever captures the final, settled
# state. This script emits the identical GIF — same 2-frame, infinite-loop
# construction, same size and placement as the first lines of
# corpus/images.sh — with nothing after it, so sample-case.sh (which samples
# a live region over several seconds, not a single settled snapshot) can
# watch it without the region moving out from under it.
set -euo pipefail
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

convert -size 32x32 xc:green -delay 20 -size 32x32 xc:yellow "$TMP/anim.gif"

printf 'ANIMATED GIF:\n'
b64g=$(base64 -w0 < "$TMP/anim.gif")
printf '\033]1337;File=inline=1;width=32px;height=32px:%s\a\n' "$b64g"
printf '\nEND\n'
exec sleep 600
