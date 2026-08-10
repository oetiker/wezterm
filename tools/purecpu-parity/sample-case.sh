#!/usr/bin/env bash
# sample-case.sh <case-name> <corpus-script> <crop-geometry> [count] [interval]
#
# Region-sampling driver for time-driven content that compare-case.sh's
# capture_settled cannot measure (see the sample_region header comment in
# lib.sh for why). Launches one backend, waits for the corpus to reach a
# steady visual state, then samples <crop-geometry> repeatedly and prints the
# raw colour sequence; then does the same for the other backend. Same
# sequential-capture discipline as compare-case.sh (one window at a time) —
# not required for focus correctness here since neither backend's animation
# depends on focus, but kept for consistency and so both scripts can share
# lib.sh's cleanup contract without surprises.
#
# Added in Task 4 fix round 1 (CRIT-2). Reusable by Tasks 5 and 6 for cursor
# blink, blink-attribute text and the visual bell.
set -euo pipefail
source "$(dirname "$0")/lib.sh"
CASE="$1"; CORPUS="$2"; GEOM="$3"
COUNT="${4:-12}"
INTERVAL="${5:-0.5}"

"$PARITY_DIR/gen-config.sh" OpenGL  "$PARITY_OUT/opengl.lua"
"$PARITY_DIR/gen-config.sh" PureCpu "$PARITY_OUT/purecpu.lua"
kill_class "par-$CASE-gl"; kill_class "par-$CASE-cpu"

for side in gl cpu; do
  cfg="opengl.lua"
  [ "$side" = "cpu" ] && cfg="purecpu.lua"
  launch "par-$CASE-$side" "$PARITY_OUT/$cfg" "$CORPUS"
  W=$(find_window "par-$CASE-$side")
  check_no_config_error "par-$CASE-$side"
  pause 3
  printf '%s: ' "$side"
  sample_region "$W" "$GEOM" "$COUNT" "$INTERVAL"
  echo
  kill_class "par-$CASE-$side"
done
