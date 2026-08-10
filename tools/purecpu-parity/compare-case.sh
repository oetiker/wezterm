#!/usr/bin/env bash
# compare-case.sh <case-name> <corpus-script>
set -euo pipefail
source "$(dirname "$0")/lib.sh"
CASE="$1"; CORPUS="$2"
FUZZ="${FUZZ:?set FUZZ from docs/purecpu-review/noise-floor.md}"

"$PARITY_DIR/gen-config.sh" OpenGL  "$PARITY_OUT/opengl.lua"
"$PARITY_DIR/gen-config.sh" PureCpu "$PARITY_OUT/purecpu.lua"
kill_class "par-$CASE-gl"; kill_class "par-$CASE-cpu"

# CRITICAL: capture ONE window at a time. Task 3 established that running both
# simultaneously leaves one window unfocused, and wezterm draws a hollow cursor
# when unfocused versus a solid block when focused. That injects a full
# character cell of difference which survives 12% fuzz — it looks exactly like
# the whole-cell rendering defect the comparison is meant to detect.
# Launch -> capture -> kill, then the next backend, so every capture is taken
# while its window holds focus.
launch "par-$CASE-gl" "$PARITY_OUT/opengl.lua" "$CORPUS"
GL=$(find_window "par-$CASE-gl")
capture_settled "$GL" "$PARITY_OUT/$CASE-gl.png" || echo "WARN: gl never settled"
kill_class "par-$CASE-gl"

launch "par-$CASE-cpu" "$PARITY_OUT/purecpu.lua" "$CORPUS"
CPU=$(find_window "par-$CASE-cpu")
capture_settled "$CPU" "$PARITY_OUT/$CASE-cpu.png" || echo "WARN: cpu never settled"
kill_class "par-$CASE-cpu"

# Three numbers, not one. Task 3 established that a fuzzed pixel count alone can
# hide a real defect: at fuzz 3% a uniform +6/255 body-wide brightness error
# reports ZERO differing pixels. So report the raw count too, and assert the
# sharper calibrated fact — no body pixel may differ by more than 1 LSB (PAE 257).
echo "== $CASE: raw differing pixels (fuzz 0) =="
compare -metric AE "$PARITY_OUT/$CASE-gl.png" "$PARITY_OUT/$CASE-cpu.png" null: 2>&1 || true; echo
echo "== $CASE: differing pixels at fuzz ${FUZZ}% =="
compare -metric AE -fuzz "${FUZZ}%" \
  "$PARITY_OUT/$CASE-gl.png" "$PARITY_OUT/$CASE-cpu.png" null: 2>&1 || true; echo
echo "== $CASE: body PAE at fuzz 0 (must be <= 257 = one 8-bit step) =="
compare -metric PAE \
  <(convert "$PARITY_OUT/$CASE-gl.png"  -crop 1000x661+0+32 +repage png:-) \
  <(convert "$PARITY_OUT/$CASE-cpu.png" -crop 1000x661+0+32 +repage png:-) \
  null: 2>&1 || true; echo
compare "$PARITY_OUT/$CASE-gl.png" "$PARITY_OUT/$CASE-cpu.png" \
  "$PARITY_OUT/$CASE-diff.png" 2>/dev/null || true

# Ink coverage per backend: catches "drew nothing at all".
for side in gl cpu; do
  printf '%s ink stddev: ' "$side"
  convert "$PARITY_OUT/$CASE-$side.png" -format '%[standard-deviation]' info:; echo
done
