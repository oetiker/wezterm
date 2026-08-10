#!/usr/bin/env bash
# gen-config.sh <front_end> <outfile>
#   env: FANCY=true|false          (tab bar style, default true)
#        SPAWN_TABS=true|false     (spawn 3 tabs + a split, default false)
#        CURSOR_BLINK_RATE=<ms>    (default 0 = disabled, matches Tasks 1-4)
#        TEXT_BLINK_RATE=<ms>      (default 0 = disabled, matches Tasks 1-4)
#        ANIMATION_FPS=<n>         (default 1, matches Tasks 1-4)
#        DEFAULT_CURSOR_STYLE=<s>  (default SteadyBlock, matches Tasks 1-4)
#        VISUAL_BELL=true|false    (default false; adds a 300ms/300ms fade
#                                    visual_bell block when true, Task 6)
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
# WINDOW_DECORATIONS is unset by default, which means "don't emit the key at
# all" so every existing case reproduces wezterm's own default
# (TITLE | RESIZE, wezterm-input-types/src/lib.rs:2123-2126) exactly as
# before. Task 5 sets it to "INTEGRATED_BUTTONS|RESIZE" to exercise the
# window-buttons render path (fancy_tab_bar.rs:314,372 gates WindowButton
# items on WindowDecorations::INTEGRATED_BUTTONS), which is otherwise
# unreachable through any existing env var.
WINDOW_DECORATIONS="${WINDOW_DECORATIONS:-}"

# (Task 5 review round 1, C1) A backslash in WINDOW_DECORATIONS — e.g. the
# `\|` a Markdown table cell needs to keep a literal pipe from being read as
# a column separator, if that escaped form is copy-pasted straight out of
# the rendered matrix into a shell — becomes `window_decorations =
# 'INTEGRATED_BUTTONS\|RESIZE'` below, which is not valid Lua. wezterm then
# rejects the *entire* config, including `front_end`, and both windows fall
# back to the same default front end: the comparison silently measures a
# backend against itself and reports a reassuring AE=0/PAE=0 that means
# nothing. Every regeneration command in this matrix must survive verbatim
# copy-paste (this is the third time in the plan a printed command has
# contradicted its own evidence cell), so refuse the input instead of
# emitting Lua that quietly defeats the whole run.
case "$WINDOW_DECORATIONS" in
  *'\'*)
    echo "gen-config.sh: WINDOW_DECORATIONS=$WINDOW_DECORATIONS contains a" \
         "backslash, which becomes an invalid Lua escape once it lands in" \
         "window_decorations = '...' below (e.g. a Markdown-escaped pipe" \
         "'\\|' pasted verbatim from a table cell). wezterm rejects the" \
         "whole config on a syntax error, both windows fall back to the" \
         "default front_end, and the comparison silently measures a" \
         "backend against itself. Use a literal '|' (e.g." \
         "INTEGRATED_BUTTONS|RESIZE), not an escaped one." >&2
    exit 1
    ;;
esac
CURSOR_BLINK_RATE="${CURSOR_BLINK_RATE:-0}"
TEXT_BLINK_RATE="${TEXT_BLINK_RATE:-0}"
ANIMATION_FPS="${ANIMATION_FPS:-1}"
DEFAULT_CURSOR_STYLE="${DEFAULT_CURSOR_STYLE:-SteadyBlock}"
# Default false so every existing Tasks 1-5 case (which never sets this)
# reproduces byte-for-byte unchanged; Task 6 sets it to exercise the visual
# bell's fade-mix path (render/mod.rs:233-258), which is otherwise never
# emitted by this generator.
VISUAL_BELL="${VISUAL_BELL:-false}"

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
#
# (fix round 4, NEW-10) The guard is symmetric, because the *other* direction
# is the likelier mistake. wezterm's own cursor_blink_rate default is 800
# (config/src/config.rs:1686) and is 0 here only because this harness forces
# blinking off for Tasks 1-4; so someone who reads the wezterm docs, sets
# DEFAULT_CURSOR_STYLE=BlinkingBlock and expects the documented default rate
# gets a config that never blinks (cursor_blink_rate == 0 short-circuits
# blinking independently of shape: termwindow/mod.rs:1150, mod.rs:1384,
# render/mod.rs:670) — and the same silent false `parity`, from a script whose
# guard would have told them nothing.
case "$DEFAULT_CURSOR_STYLE" in
  Blinking*)
    if [ "$CURSOR_BLINK_RATE" = "0" ]; then
      echo "gen-config.sh: DEFAULT_CURSOR_STYLE=$DEFAULT_CURSOR_STYLE is a" \
           "Blinking* variant but CURSOR_BLINK_RATE=0 (this harness's" \
           "default, not wezterm's 800), so the cursor will never blink and" \
           "a comparison will silently read as parity. Set" \
           "CURSOR_BLINK_RATE=<ms> (e.g. 400) to actually test blink." >&2
      exit 1
    fi
    ;;
  *)
    if [ "$CURSOR_BLINK_RATE" != "0" ]; then
      echo "gen-config.sh: CURSOR_BLINK_RATE=$CURSOR_BLINK_RATE is set but" \
           "DEFAULT_CURSOR_STYLE=$DEFAULT_CURSOR_STYLE is not a Blinking*" \
           "variant, so the cursor will never blink and a comparison will" \
           "silently read as parity. Set DEFAULT_CURSOR_STYLE=BlinkingBlock" \
           "(or BlinkingUnderline/BlinkingBar) to actually test blink." >&2
      exit 1
    fi
    ;;
esac

STARTUP=""
if [ "$SPAWN_TABS" = "true" ]; then
STARTUP=$(cat <<'LUA'
wezterm.on('gui-startup', function(cmd)
  -- (Task 5 review round 1, I4) Registering a gui-startup handler at all
  -- replaces wezterm's normal startup entirely, so a handler that spawns
  -- windows/tabs/panes without forwarding `cmd.args` discards the corpus
  -- command the CLI was invoked with -- every tab and pane then runs the
  -- operator's own login shell and its ~/.bashrc prompt instead of the
  -- corpus script. That leaked into every capture and crop this task took:
  -- tab titles read the shell's name, not the corpus's, and two lines of
  -- prompt text (with real nerd-font colour icons) appeared in the body.
  -- Passing `args = cmd.args` through to every spawn/split call below makes
  -- the window's content the same corpus.sh text `launch()` was asked to
  -- run, on every tab and in the split pane, so captures are hermetic and
  -- do not depend on the operator's shell configuration.
  local tab, pane, window = wezterm.mux.spawn_window { args = cmd.args }
  pane:split { direction = 'Right', size = 0.5, args = cmd.args }
  window:spawn_tab { args = cmd.args }
  window:spawn_tab { args = cmd.args }
  -- spawn_tab activates each new tab as it's created, so without this the
  -- window starts on tab 3 and the split created above (in tab 1) is not on
  -- screen for any capture. Task 5 needs the divider visible, so reactivate
  -- the split tab last.
  tab:activate()
end)
LUA
)
fi

# (Task 5 review round 1, I1) Built as an array and printed one line per
# element, rather than substituting a possibly-empty ${WINDOW_DECORATIONS_LINE}
# into a fixed heredoc, so that leaving WINDOW_DECORATIONS unset reproduces
# byte-for-byte what this script emitted before Task 5 (no extra blank line).
# Every prior Task 1-4 case leaves WINDOW_DECORATIONS unset.
CONFIG_LINES=(
  "local wezterm = require 'wezterm'"
  "${STARTUP}"
  "return {"
  "  front_end = '${FRONT_END}',"
  "  font_size = 12.0,"
  "  initial_cols = 100,"
  "  initial_rows = 30,"
  "  cursor_blink_rate = ${CURSOR_BLINK_RATE},"
  "  text_blink_rate = ${TEXT_BLINK_RATE},"
  "  animation_fps = ${ANIMATION_FPS},"
  "  default_cursor_style = '${DEFAULT_CURSOR_STYLE}',"
  "  enable_tab_bar = true,"
  "  use_fancy_tab_bar = ${FANCY},"
)
if [ -n "$WINDOW_DECORATIONS" ]; then
  CONFIG_LINES+=("  window_decorations = '${WINDOW_DECORATIONS}',")
fi
if [ "$VISUAL_BELL" = "true" ]; then
  CONFIG_LINES+=("  visual_bell = { fade_in_duration_ms = 300, fade_out_duration_ms = 300 },")
fi
CONFIG_LINES+=(
  "  audible_bell = 'Disabled',"
  "  window_close_confirmation = 'NeverPrompt',"
  "  check_for_updates = false,"
  "  window_padding = { left = 0, right = 0, top = 0, bottom = 0 },"
  "  colors = {"
  "    foreground = '#c0c0c0',"
  "    background = '#101010',"
  "    cursor_bg = '#e0e0e0',"
  "    cursor_fg = '#101010',"
  "    cursor_border = '#e0e0e0',"
  "  },"
  "}"
)
printf '%s\n' "${CONFIG_LINES[@]}" > "$OUT"
