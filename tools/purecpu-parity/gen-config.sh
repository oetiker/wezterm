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
