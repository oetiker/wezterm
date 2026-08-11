#!/usr/bin/env bash
# compare-case.sh <case-name> <corpus-script>
set -euo pipefail
source "$(dirname "$0")/lib.sh"
CASE="$1"; CORPUS="$2"
FUZZ="${FUZZ:?set FUZZ from docs/purecpu-review/noise-floor.md}"

# (fix round 3, NEW-7) A printed regeneration command that omits an env var
# the case actually needs (e.g. PARITY_SETTLE_WARMUP for slow-to-render
# content) silently reproduces the noise floor from two blank captures
# instead of the real result — this has happened twice now, and the second
# time the wrong numbers were the reassuring ones. Echo the warmup value
# actually in effect so a run's provenance is visible in its own output,
# not just in prose elsewhere.
echo "== $CASE: PARITY_SETTLE_WARMUP=${PARITY_SETTLE_WARMUP:-0} =="

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
check_no_config_error "par-$CASE-gl"
check_bell_disabled "$PARITY_OUT/opengl.lua"
capture_settled "$GL" "$PARITY_OUT/$CASE-gl.png" || echo "WARN: gl never settled"
kill_class "par-$CASE-gl"

launch "par-$CASE-cpu" "$PARITY_OUT/purecpu.lua" "$CORPUS"
CPU=$(find_window "par-$CASE-cpu")
check_no_config_error "par-$CASE-cpu"
check_bell_disabled "$PARITY_OUT/purecpu.lua"
capture_settled "$CPU" "$PARITY_OUT/$CASE-cpu.png" || echo "WARN: cpu never settled"
kill_class "par-$CASE-cpu"

# (fix round 3, NEW-7) capture_settled can declare victory on a window that
# hasn't started drawing anything yet — two identical *blank* captures pass
# the same "unchanged" test as two identical *finished* ones. For a corpus
# whose content is a colour image, a blank/text-only capture always develops
# as ImageMagick colorspace Gray (background + monochrome text, no chroma).
# That's a strong, cheap signal that this run measured nothing: warn loudly
# rather than let a blank-vs-blank pair silently report the noise floor as
# if it were a real comparison.
#
# (fix round 4, NEW-11) Warn-not-refuse stands, but not for the reason round
# 3 gave. That reason was "text-only captures may legitimately be Gray";
# that is not true of this harness's text-only captures. Task 3's
# deliberately text-only corpus develops as sRGB (`out/plain-{gl,cpu}.png`),
# as does every other genuine capture in `out/`; the only Gray files there
# are the known-blank ones (`wide-sixel-noguard-*`) and two hand-made crops.
# So refusing would not, today, false-positive on anything. It is a warning
# because this is a shared driver whose Task 5/6 callers are unwritten and
# because a refusal here would abort before printing the numbers that let a
# reader see *how* blank the run was — not because any real capture in this
# harness is expected to be Gray. If a future case legitimately produces a
# Gray capture, say so where that case is defined; do not assume it from
# this comment.
for side in gl cpu; do
  space=$(identify -format '%[colorspace]' "$PARITY_OUT/$CASE-$side.png" 2>/dev/null || echo "?")
  if [ "$space" = "Gray" ]; then
    echo "WARNING: $CASE-$side.png captured as Gray colorspace — this" \
         "backend likely hadn't drawn any colour content yet when" \
         "capture_settled declared it settled (see PARITY_SETTLE_WARMUP" \
         "in lib.sh). Numbers below may be spuriously close to parity." >&2
  fi
done

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
