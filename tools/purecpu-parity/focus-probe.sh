#!/usr/bin/env bash
# Regenerate the evidence behind Findings 1 and 2 of
# docs/purecpu-review/noise-floor.md.
#
# Finding 1: window focus state changes what wezterm draws (solid block cursor
# when focused, hollow outline when not). Comparing a focused capture against an
# unfocused one fabricates a whole-cell difference that survives 12% fuzz.
#
# Finding 2: PureCpu draws some fancy tab-bar title glyph runs one pixel left of
# OpenGL, deterministically and independently of focus.
#
# This script deliberately launches BOTH backends at once, which is exactly what
# production comparison drivers must NOT do — it is how the focus mismatch is
# produced on purpose so it can be measured. Under `marco` the last-mapped
# window takes focus, so running both launch orders yields all four combinations
# of (backend x focus state):
#
#   run A: gl then cpu  -> focus-A-gl.png (unfocused), focus-A-cpu.png (focused)
#   run B: cpu then gl  -> focus-B-cpu.png (unfocused), focus-B-gl.png (focused)
#
# Usage: PARITY_XAUTH=<:20 Xauthority> ./focus-probe.sh
set -euo pipefail
source "$(dirname "$0")/lib.sh"

: "${TAB_H:=32}"
CORPUS="$PARITY_DIR/corpus/plain.sh"
GL_CFG="$PARITY_OUT/opengl.lua"
CPU_CFG="$PARITY_OUT/purecpu.lua"

[ -f "$GL_CFG" ]  || "$PARITY_DIR/gen-config.sh" OpenGL  "$GL_CFG"
[ -f "$CPU_CFG" ] || "$PARITY_DIR/gen-config.sh" PureCpu "$CPU_CFG"

declare -A CFG=( [gl]="$GL_CFG" [cpu]="$CPU_CFG" )

probe_run() {  # probe_run <tag> <first backend> <second backend>
  local tag="$1" first="$2" second="$3"
  launch "par-$first"  "${CFG[$first]}"  "$CORPUS"
  pause 4
  launch "par-$second" "${CFG[$second]}" "$CORPUS"
  pause 4

  local idf ids
  idf=$(find_window "par-$first")
  ids=$(find_window "par-$second")
  echo "run $tag: first=$first($idf) second=$second($ids)"
  echo -n "  active window (expect $second = $ids): "
  xprop -root _NET_ACTIVE_WINDOW 2>/dev/null | awk '{print $NF}'

  capture_settled "$idf" "$PARITY_OUT/focus-$tag-$first.png"
  capture_settled "$ids" "$PARITY_OUT/focus-$tag-$second.png"

  kill_class "par-$first"
  kill_class "par-$second"
  pause 2
}

probe_run A gl cpu
probe_run B cpu gl

GLF="$PARITY_OUT/focus-B-gl.png"   # OpenGL,  focused
GLU="$PARITY_OUT/focus-A-gl.png"   # OpenGL,  unfocused
CPUF="$PARITY_OUT/focus-A-cpu.png" # PureCpu, focused
CPUU="$PARITY_OUT/focus-B-cpu.png" # PureCpu, unfocused

ae() { compare -metric AE ${3:+-fuzz "$3"} "$1" "$2" null: 2>&1 || true; }
strip() { convert "$1" -crop "1000x${TAB_H}+0+0" +repage "$2"; }
body()  { convert "$1" -crop "1000x$(( $(identify -format '%h' "$1") - TAB_H ))+0+$TAB_H" +repage "$2"; }

echo
echo "=== Finding 1: focus, not backend, produces the whole-cell difference ==="
printf '%-34s %-12s %-8s %s\n' comparison "focus" AE "AE@fuzz12%"
printf '%-34s %-12s %-8s %s\n' "run A: gl vs cpu" mismatched \
  "$(ae "$GLU" "$CPUF")" "$(ae "$GLU" "$CPUF" 12%)"
printf '%-34s %-12s %-8s %s\n' "run B: gl vs cpu" mismatched \
  "$(ae "$GLF" "$CPUU")" "$(ae "$GLF" "$CPUU" 12%)"
printf '%-34s %-12s %-8s %s\n' "gl(B) vs cpu(A)" "both focused" \
  "$(ae "$GLF" "$CPUF")" "$(ae "$GLF" "$CPUF" 12%)"
printf '%-34s %-12s %-8s %s\n' "gl(A) vs cpu(B)" "both unfocused" \
  "$(ae "$GLU" "$CPUU")" "$(ae "$GLU" "$CPUU" 12%)"
printf '%-34s %-12s %-8s %s\n' "gl focused vs gl unfocused" "same backend" \
  "$(ae "$GLF" "$GLU")" "$(ae "$GLF" "$GLU" 12%)"
printf '%-34s %-12s %-8s %s\n' "cpu focused vs cpu unfocused" "same backend" \
  "$(ae "$CPUF" "$CPUU")" "$(ae "$CPUF" "$CPUU" 12%)"

echo
echo "--- the focus delta is entirely the cursor cell, nothing in the tab strip ---"
for p in "$GLF:$GLU:gl" "$CPUF:$CPUU:cpu"; do
  IFS=: read -r a b tag <<<"$p"
  strip "$a" /tmp/fp-s1.png; strip "$b" /tmp/fp-s2.png
  body  "$a" /tmp/fp-b1.png; body  "$b" /tmp/fp-b2.png
  echo "$tag focused-vs-unfocused: tab strip AE=$(ae /tmp/fp-s1.png /tmp/fp-s2.png) body AE=$(ae /tmp/fp-b1.png /tmp/fp-b2.png)"
done

echo
echo "=== Finding 2: 1px horizontal shift of tab title glyph runs (focus matched) ==="
echo "--- per-cluster: is cpu[x] equal to gl[x+1]? ---"
for spec in "29 12 12" "49 12 10"; do
  set -- $spec; x=$1; y=$2; w=$3
  convert "$GLF"  -crop "${w}x12+${x}+${y}"       +repage /tmp/fp-a.png
  convert "$CPUF" -crop "${w}x12+$((x-1))+${y}"   +repage /tmp/fp-b.png
  convert "$CPUF" -crop "${w}x12+${x}+${y}"       +repage /tmp/fp-c.png
  echo "x=$x w=$w  no shift AE=$(ae /tmp/fp-a.png /tmp/fp-c.png)   gl[x] vs cpu[x-1] AE=$(ae /tmp/fp-a.png /tmp/fp-b.png)"
done
echo "--- whole title is NOT uniformly shifted: best global dx over crop 62x12+8+12 ---"
for dx in -2 -1 0 1 2; do
  convert "$GLF"  -crop 62x12+8+12            +repage /tmp/fp-a.png
  convert "$CPUF" -crop "62x12+$((8-dx))+12"  +repage /tmp/fp-b.png
  echo "dx=$dx AE=$(ae /tmp/fp-a.png /tmp/fp-b.png)"
done

echo
echo "=== figures embedded in docs/purecpu-review/noise-floor.md ==="
compare "$PARITY_OUT/plain-gl.png" "$PARITY_OUT/plain-cpu.png" \
  "$PARITY_OUT/noise-floor-diff-mismatched.png" 2>/dev/null || true
compare "$GLF" "$CPUF" "$PARITY_OUT/noise-floor-diff-matched.png" 2>/dev/null || true
compare -fuzz 1% "$GLF" "$CPUF" "$PARITY_OUT/noise-floor-diff-matched-fuzz1.png" 2>/dev/null || true

# Cursor-vs-focus strip: 16x26 at (0,162) covers the cursor cell plus a margin,
# 5x zoom, grey separator border. Order: gl focused, gl unfocused, cpu focused,
# cpu unfocused.
i=0
for src in "$GLF" "$GLU" "$CPUF" "$CPUU"; do
  convert "$src" -crop 16x26+0+162 +repage -scale 500% \
    -bordercolor '#404040' -border 3 "/tmp/fp-cur-$i.png"
  i=$((i + 1))
done
convert /tmp/fp-cur-0.png /tmp/fp-cur-1.png /tmp/fp-cur-2.png /tmp/fp-cur-3.png \
  +append "$PARITY_OUT/noise-floor-cursor-focus.png"

# Tab title zoom: 90x24 at (0,6) covers "1: sleep x", 6x zoom, OpenGL stacked
# above PureCpu.
convert "$GLF"  -crop 90x24+0+6 +repage -scale 600% /tmp/fp-t1.png
convert "$CPUF" -crop 90x24+0+6 +repage -scale 600% /tmp/fp-t2.png
convert /tmp/fp-t1.png /tmp/fp-t2.png -append "$PARITY_OUT/noise-floor-tabbar-zoom.png"

rm -f /tmp/fp-*.png
ls -1 "$PARITY_OUT"/noise-floor-*.png "$PARITY_OUT"/focus-*.png
