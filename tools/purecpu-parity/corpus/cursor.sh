#!/usr/bin/env bash
# corpus/cursor.sh — static cursor, blink-attribute text, and a repeating
# visual bell, for Task 6's cursor/text-animation rows.
#
# One corpus serves all four rows (static cursor, cursor blink, blink-
# attribute text, visual bell): which behaviour is under test is selected by
# the config the driver generates (CURSOR_BLINK_RATE/DEFAULT_CURSOR_STYLE,
# TEXT_BLINK_RATE, VISUAL_BELL), not by the corpus content. The bell is rung
# once per second in a background loop, rather than once, so that whichever
# case samples it finds it still ringing across sample-case.sh's whole
# sampling window (a single `\a`'s fade is only 300ms+300ms — most of a
# 12-sample/0.5s-interval run would otherwise land after it had already
# faded back to rest).
set -euo pipefail
printf 'CURSOR CASE\n'
printf 'blink attr: \033[5mBLINKING\033[0m normal\n'
printf 'cursor rests at the end of this line: '
while true; do
  sleep 1
  printf '\a'
done
