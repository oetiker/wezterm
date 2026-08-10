#!/usr/bin/env bash
# gen-config.sh <front_end> <outfile>
#   env: FANCY=true|false      (tab bar style, default true)
#        SPAWN_TABS=true|false (spawn 3 tabs + a split, default false)
set -euo pipefail
FRONT_END="$1"
OUT="$2"
FANCY="${FANCY:-true}"
SPAWN_TABS="${SPAWN_TABS:-false}"

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
  cursor_blink_rate = 0,
  text_blink_rate = 0,
  animation_fps = 1,
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
