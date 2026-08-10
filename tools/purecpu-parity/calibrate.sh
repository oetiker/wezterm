#!/usr/bin/env bash
# Measure the difference between backends on static plain text.
#
# Usage: ./calibrate.sh [gl.png] [cpu.png]
#
# IMPORTANT — the two captures must be in the SAME window-focus state.
# wezterm renders two things differently when a window is unfocused: the
# cursor becomes a hollow outline instead of a solid block, and the fancy tab
# bar switches to its inactive titlebar colours. Comparing a focused capture
# against an unfocused one injects a whole-cell difference that looks exactly
# like a renderer defect. Drivers must therefore launch and capture one window
# at a time (launch -> capture -> kill_class -> next), so that each capture is
# taken while its window holds focus. See docs/purecpu-review/noise-floor.md.
#
# Results are reported separately for the tab-bar strip and the terminal body,
# because they are rendered by different code paths and have different noise
# characteristics. TAB_H is the tab strip height in pixels: the window is
# 693px for 30 rows of 22px (660px) plus a 32px tab strip; the first body text
# row starts at y=32.
set -euo pipefail
source "$(dirname "$0")/lib.sh"

GL_PNG="${1:-$PARITY_OUT/plain-gl.png}"
CPU_PNG="${2:-$PARITY_OUT/plain-cpu.png}"
: "${TAB_H:=32}"

H=$(identify -format '%h' "$GL_PNG")
W=$(identify -format '%w' "$GL_PNG")
BODY_H=$((H - TAB_H))
TOTAL=$((W * H))

ae() { compare -metric AE ${3:+-fuzz "$3"} "$1" "$2" null: 2>&1 || true; }

crop() {  # crop <src> <geometry> <dst>
  convert "$1" -crop "$2" +repage "$3"
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

crop "$GL_PNG"  "${W}x${TAB_H}+0+0"        "$work/tab-gl.png"
crop "$CPU_PNG" "${W}x${TAB_H}+0+0"        "$work/tab-cpu.png"
crop "$GL_PNG"  "${W}x${BODY_H}+0+$TAB_H"  "$work/body-gl.png"
crop "$CPU_PNG" "${W}x${BODY_H}+0+$TAB_H"  "$work/body-cpu.png"

echo "== inputs =="
echo "gl : $GL_PNG"
echo "cpu: $CPU_PNG"
echo "geometry ${W}x${H} (${TOTAL} px), tab strip ${TAB_H}px, body ${BODY_H}px"
echo

echo "== whole frame =="
for m in AE RMSE PAE MAE; do
  printf '%-5s %s\n' "$m" "$(compare -metric $m "$GL_PNG" "$CPU_PNG" null: 2>&1 || true)"
done
echo

echo "== per-channel AE (whole frame): a channel-wide imbalance means a colour bias =="
for ch in Red Green Blue; do
  printf '%-6s AE=%s PAE=%s\n' "$ch" \
    "$(compare -metric AE -channel $ch "$GL_PNG" "$CPU_PNG" null: 2>&1 || true)" \
    "$(compare -metric PAE -channel $ch "$GL_PNG" "$CPU_PNG" null: 2>&1 || true)"
done
echo

echo "== AE at increasing fuzz, whole frame / tab strip / terminal body =="
printf '%-9s %-12s %-12s %-12s %s\n' fuzz frame tab body body_pct
for f in 0 0.25 0.5 0.75 1 2 3 5 8 12 20 30 50; do
  nf=$(ae "$GL_PNG" "$CPU_PNG" "${f}%")
  nt=$(ae "$work/tab-gl.png" "$work/tab-cpu.png" "${f}%")
  nb=$(ae "$work/body-gl.png" "$work/body-cpu.png" "${f}%")
  pct=$(awk -v n="$nb" -v t="$((W * BODY_H))" 'BEGIN{printf "%.4f%%", 100*n/t}')
  printf '%-9s %-12s %-12s %-12s %s\n' "${f}%" "$nf" "$nt" "$nb" "$pct"
done
echo

echo "== body only: is the difference confined to text, or in the empty background too? =="
# The corpus prints 7 lines; everything below is untouched default background.
TEXT_H=$((TAB_H + 160))
crop "$GL_PNG"  "${W}x160+0+$TAB_H"                    "$work/txt-gl.png"
crop "$CPU_PNG" "${W}x160+0+$TAB_H"                    "$work/txt-cpu.png"
crop "$GL_PNG"  "${W}x$((H - TEXT_H))+0+$TEXT_H"       "$work/bg-gl.png"
crop "$CPU_PNG" "${W}x$((H - TEXT_H))+0+$TEXT_H"       "$work/bg-cpu.png"
echo "text rows (y $TAB_H..$TEXT_H)   AE=$(ae "$work/txt-gl.png" "$work/txt-cpu.png")"
echo "empty rows (y $TEXT_H..$H)  AE=$(ae "$work/bg-gl.png" "$work/bg-cpu.png")"
echo

echo "== residual differences that survive 12% fuzz: where are they? =="
compare -metric AE -fuzz 12% -highlight-color red -lowlight-color white \
  "$GL_PNG" "$CPU_PNG" "$work/hl.png" 2>/dev/null || true
convert "$work/hl.png" -fill black -opaque red -fill white +opaque black \
  -colorspace Gray -threshold 50% -negate "$work/mask.png"
echo -n "bounding box of residual: "
convert "$work/mask.png" -trim info: 2>&1 | awk '{print $3, $4}'
echo

echo "== difference heat maps =="
compare "$GL_PNG" "$CPU_PNG" "$PARITY_OUT/plain-diff.png" 2>/dev/null || true
compare -fuzz 1% "$GL_PNG" "$CPU_PNG" "$PARITY_OUT/plain-diff-fuzz1.png" 2>/dev/null || true
echo "wrote $PARITY_OUT/plain-diff.png       (every differing pixel)"
echo "wrote $PARITY_OUT/plain-diff-fuzz1.png (only differences above the 1% noise floor)"
