#!/usr/bin/env bash
# corpus/blink-text.sh — SGR 5 blink-attribute text and NOTHING else.
#
# Added for the measurement round. corpus/cursor.sh also carries SGR 5 text,
# but it rings the bell once a second in a background loop, so a CPU sample
# taken against it measures blink *plus* a repeating visual-bell fade and
# cannot attribute the cost to either. This corpus exists so the SGR 5 arm
# measures one animation source.
#
# Whether the text actually blinks is selected by the config
# (TEXT_BLINK_RATE), not by this corpus — same contract as cursor.sh.
#
# The row count matters and is deliberate: one blinking cell is enough to make
# dirty_pixel_rects non-empty on every animation frame, which defeats the idle
# early exit and buys a whole paint_pass. Several rows spread across the window
# also widen the dirty span, which is the thing worth measuring.
set -euo pipefail
printf 'BLINK TEXT CASE — SGR 5 only, no bell, no images\n'
printf '\n'
printf 'top:    \033[5mBLINKING\033[0m steady\n'
printf 'middle: \033[5mBLINKING\033[0m steady\n'
printf '\n'
printf 'plain filler so the window is not mostly background:\n'
printf 'ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz\n'
printf '0123456789 !"#$%%&()*+,-./:;<=>?@[]^_{|}~\n'
printf '\n'
printf 'bottom: \033[5mBLINKING\033[0m steady\n'
exec sleep 600
