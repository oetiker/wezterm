#!/usr/bin/env bash
# gen-config.sh <front_end> <outfile>
#   env: FANCY=true|false          (tab bar style, default true)
#        SPAWN_TABS=true|false     (spawn 3 tabs + a split, default false)
#        CURSOR_BLINK_RATE=<ms>    (default 0 = disabled, matches Tasks 1-4)
#        TEXT_BLINK_RATE=<ms>      (default 0 = disabled, matches Tasks 1-4)
#        ANIMATION_FPS=<n>         (default 1, matches Tasks 1-4)
#        DEFAULT_CURSOR_STYLE=<s>  (default SteadyBlock, matches Tasks 1-4)
#
# The blink/animation defaults below are off (SteadyBlock, rates 0, fps 1)
# because Tasks 1-4 measured a deliberately static terminal — that was the
# point, it isolates rendering from timing. Task 4 fix round 2 (NEW-4) added
# these overrides here rather than in a Task 6 file, per controller ruling:
# Tasks 5/6 need a blinking terminal to exercise cursor blink / blink-
# attribute text / the visual bell through sample-case.sh, and gen-config.sh
# is the only place that sets these. Defaults are unchanged, so every
# existing Tasks 1-4 measurement (which calls this script without setting
# these vars) reproduces exactly as before.
#
# CURSOR_BLINK_RATE alone is not sufficient to make the cursor blink:
# `default_cursor_style` defaults to `SteadyBlock` (config/src/config.rs:1912),
# and cursor_blink_rate only takes effect when the *effective* cursor shape
# is one of the Blinking* variants (termwindow/mod.rs:1383,
# `cursor.shape.is_blinking()`). Discovered by trying to verify this
# override end-to-end: with only CURSOR_BLINK_RATE set, neither backend
# blinked, because the cursor shape itself was never blinking. Both must be
# set together for a blink test.
set -euo pipefail
FRONT_END="$1"
OUT="$2"
FANCY="${FANCY:-true}"
SPAWN_TABS="${SPAWN_TABS:-false}"
CURSOR_BLINK_RATE="${CURSOR_BLINK_RATE:-0}"
TEXT_BLINK_RATE="${TEXT_BLINK_RATE:-0}"
ANIMATION_FPS="${ANIMATION_FPS:-1}"
DEFAULT_CURSOR_STYLE="${DEFAULT_CURSOR_STYLE:-SteadyBlock}"

# (fix round 3, NEW-8) A non-zero CURSOR_BLINK_RATE with DEFAULT_CURSOR_STYLE
# left at SteadyBlock is not a harmless no-op, it's a silent trap: wezterm
# only blinks the cursor when the *effective* cursor shape is one of the
# Blinking* variants (termwindow/mod.rs:1383, cursor.shape.is_blinking()), so
# this combination emits a config that never blinks on either backend and
# reports a false `parity` for whoever measures cursor blink with it — the
# exact failure this whole corpus-must-trigger-the-defect fix effort exists
# to eliminate. The round-2 fix documented the pairing in a comment; a
# reviewer reproduced the silent false-parity anyway, because a header
# comment does not survive a copy-pasted invocation. Fail loudly instead.
# (TEXT_BLINK_RATE has no equivalent check here: whether it takes effect
# depends on the corpus emitting SGR 5 blinking text, which this script
# can't see — that half of the pairing is still only a comment, on purpose.)
if [ "$CURSOR_BLINK_RATE" != "0" ]; then
  case "$DEFAULT_CURSOR_STYLE" in
    Blinking*) ;;
    *)
      echo "gen-config.sh: CURSOR_BLINK_RATE=$CURSOR_BLINK_RATE is set but" \
           "DEFAULT_CURSOR_STYLE=$DEFAULT_CURSOR_STYLE is not a Blinking*" \
           "variant, so the cursor will never blink and a comparison will" \
           "silently read as parity. Set DEFAULT_CURSOR_STYLE=BlinkingBlock" \
           "(or BlinkingUnderline/BlinkingBar) to actually test blink." >&2
      exit 1
      ;;
  esac
fi

STARTUP=""
if [ "$SPAWN_TABS" = "true" ]; then
STARTUP=$(cat <<'LUA'
wezterm.on('gui-startup', function(cmd)
  local tab, pane, window = wezterm.mux.spawn_window {}
  pane:split { direction = 'Right', size = 0.5 }
  window:spawn_tab {}
  window:spawn_tab {}
end)
LUA
)
fi

cat > "$OUT" <<EOF
local wezterm = require 'wezterm'
${STARTUP}
return {
  front_end = '${FRONT_END}',
  font_size = 12.0,
  initial_cols = 100,
  initial_rows = 30,
  cursor_blink_rate = ${CURSOR_BLINK_RATE},
  text_blink_rate = ${TEXT_BLINK_RATE},
  animation_fps = ${ANIMATION_FPS},
  default_cursor_style = '${DEFAULT_CURSOR_STYLE}',
  enable_tab_bar = true,
  use_fancy_tab_bar = ${FANCY},
  audible_bell = 'Disabled',
  window_close_confirmation = 'NeverPrompt',
  check_for_updates = false,
  window_padding = { left = 0, right = 0, top = 0, bottom = 0 },
  colors = {
    foreground = '#c0c0c0',
    background = '#101010',
    cursor_bg = '#e0e0e0',
    cursor_fg = '#101010',
    cursor_border = '#e0e0e0',
  },
}
EOF
