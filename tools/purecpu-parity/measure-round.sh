#!/usr/bin/env bash
# measure-round.sh — the batched CPU measurement round.
#
# Runs three configs against four arms, sequentially, one window at a time.
# Prints one line per sample; the caller reads the log.
#
# THE FOUR ARMS
#   floor    pre-fix binary, PureCpu            — what "idle" cost before the pass
#   subject  fixed binary,   PureCpu            — what we shipped
#   ceiling  fixed binary,   PureCpu + force_full_repaint
#   gl       fixed binary,   OpenGL             — the renderer we are matching
#
# The ceiling is the known-good upper bound: every frame repaints the whole
# window. The gl arm is the control the plan calls the strongest instrument in
# this project — it answers "did the fix reach the GPU path's own level, or
# merely move?", which floor/subject/ceiling alone cannot.
#
# THE THREE CONFIGS, and why each is here. The Task 10 review (F5) rewrote
# this list, and the reason is the whole point of the round:
#
#   idle-cursor    A blinking cursor and nothing else. The subject should sit
#                  AT THE FLOOR. This is the only case the Task 10 report
#                  actually measured.
#   blink-text     One SGR 5 region. A single blinking cell makes
#                  dirty_pixel_rects non-empty every animation frame, so the
#                  idle early exit does NOT fire and a whole paint_pass runs.
#                  THE SUBJECT IS EXPECTED NEAR THE CEILING HERE AND THAT IS
#                  NOT A FAILURE. A round run only on idle-cursor would have
#                  confirmed nothing about the paths Task 10 added while
#                  looking like a pass.
#   static-image   A large static sixel with a blinking cursor holding the
#                  animation timer open. Tests the source comment's claim that
#                  "a still image costs nothing here, forever" — true in
#                  rects, and the CPU question is whether attrs.images()
#                  deep-cloning every ImageCell per frame makes it false in
#                  CPU.
#
# ANIMATION_FPS=60 throughout, deliberately: the harness default is 1 (Tasks
# 1-4 measured a static terminal on purpose), but 60 is the shipped default
# and the only setting at which the per-frame costs under test are real.
# Recorded here rather than left implicit.
set -euo pipefail
PARITY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
: "${PARITY_XAUTH:?set PARITY_XAUTH}"
: "${FLOOR_BIN:=/scratch/oetiker/wezterm-builds/wezterm-gui-prefix}"
: "${SUBJECT_BIN:?set SUBJECT_BIN to the freshly built binary}"
OUT="${PARITY_OUT:-$PARITY_DIR/out}"
SECS="${SECS:-30}"

mkdir -p "$OUT"

for b in "$FLOOR_BIN" "$SUBJECT_BIN"; do
  [ -x "$b" ] || { echo "measure-round.sh: not executable: $b" >&2; exit 1; }
done
echo "# floor   $FLOOR_BIN   md5=$(md5sum "$FLOOR_BIN" | awk '{print $1}')"
echo "# subject $SUBJECT_BIN md5=$(md5sum "$SUBJECT_BIN" | awk '{print $1}')"
echo "# seconds per sample: $SECS"

# gen_cfg <name> <front_end> <force_full> ... env for gen-config comes from caller
run_case() {  # run_case <case> <corpus> <blink-env...>
  local case="$1" corpus="$2"; shift 2
  local cfg

  echo "## case=$case corpus=$corpus env=$*"

  # floor + subject + ceiling are all PureCpu; gl is the control.
  for arm in floor subject ceiling gl; do
    local bin front_end ff
    case "$arm" in
      floor)   bin="$FLOOR_BIN";   front_end=PureCpu; ff=false ;;
      subject) bin="$SUBJECT_BIN"; front_end=PureCpu; ff=false ;;
      ceiling) bin="$SUBJECT_BIN"; front_end=PureCpu; ff=true  ;;
      gl)      bin="$SUBJECT_BIN"; front_end=OpenGL;  ff=false ;;
    esac
    cfg="$OUT/m-$case-$arm.lua"
    env "$@" FORCE_FULL_REPAINT="$ff" "$PARITY_DIR/gen-config.sh" "$front_end" "$cfg"
    WEZTERM_BIN="$bin" PARITY_XAUTH="$PARITY_XAUTH" PARITY_OUT="$OUT" \
      "$PARITY_DIR/cpu-case.sh" "m-$case-$arm" "$cfg" "$PARITY_DIR/corpus/$corpus" "$SECS"
  done
}

# 1. Idle cursor: blinking cursor, no blink text, no images.
run_case idle-cursor plain.sh \
  ANIMATION_FPS=60 CURSOR_BLINK_RATE=400 DEFAULT_CURSOR_STYLE=BlinkingBlock TEXT_BLINK_RATE=0

# 2. SGR 5 blink text, steady cursor, so blink text is the only animation.
run_case blink-text blink-text.sh \
  ANIMATION_FPS=60 CURSOR_BLINK_RATE=0 DEFAULT_CURSOR_STYLE=SteadyBlock TEXT_BLINK_RATE=400

# 3. Static full-screen image, with a blinking cursor holding the timer open.
run_case static-image static-image.sh \
  ANIMATION_FPS=60 CURSOR_BLINK_RATE=400 DEFAULT_CURSOR_STYLE=BlinkingBlock TEXT_BLINK_RATE=0

echo "## done"
