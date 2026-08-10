#!/usr/bin/env bash
# Shared helpers for the PureCpu parity harness.
#
# Cleanup contract: `launch` records every class it starts and registers a
# single EXIT trap (on the shell that sourced this file) which kills all of
# them via kill_class. This fires on normal exit, `set -e` aborts, and
# signals alike, so a driver script that dies between `launch` and an
# explicit `kill_class` (e.g. capture_settled returning 1) will not leak
# `wezterm-gui ... sleep 600` processes/windows on the display. Driver
# authors do not need their own cleanup for classes started via `launch`;
# calling `kill_class` explicitly after a successful run is still fine and
# just makes cleanup happen immediately instead of at shell exit.
set -euo pipefail

PARITY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
: "${WEZTERM_BIN:=/scratch/oetiker/wezterm-builds/wezterm-gui-rebased}"
: "${PARITY_DISPLAY:=:20}"
: "${PARITY_XAUTH:?set PARITY_XAUTH to the :20 Xauthority file}"
: "${PARITY_OUT:=$PARITY_DIR/out}"

export DISPLAY="$PARITY_DISPLAY"
export XAUTHORITY="$PARITY_XAUTH"
mkdir -p "$PARITY_OUT"

# Foreground sleep is unavailable in this harness environment; block on timeout.
# NOTE: `timeout N cat </dev/null` does NOT wait — cat hits EOF immediately, so
# timeout has nothing to interrupt and this returns in ~0s. Use `tail -f /dev/null`
# instead, which blocks until timeout fires.
pause() { timeout "$1" tail -f /dev/null || true; }

declare -ag _PARITY_LAUNCHED_CLASSES=()
_PARITY_TRAP_SET=0

_parity_cleanup() {
  local class
  for class in "${_PARITY_LAUNCHED_CLASSES[@]:-}"; do
    [ -n "$class" ] && kill_class "$class"
  done
}

launch() {  # launch <class> <config> <shell-command>
  local class="$1" config="$2" cmd="$3"
  if [ "$_PARITY_TRAP_SET" -eq 0 ]; then
    trap _parity_cleanup EXIT
    _PARITY_TRAP_SET=1
  fi
  _PARITY_LAUNCHED_CLASSES+=("$class")
  WEZTERM_CONFIG_FILE="$config" WEZTERM_LOG=info setsid nohup "$WEZTERM_BIN" start \
    --always-new-process --class "$class" \
    -- bash -c "$cmd" >"$PARITY_OUT/$class.log" 2>&1 </dev/null &
}

find_window() {  # find_window <class> -> window id on stdout
  local class="$1" id="" i
  for i in $(seq 40); do
    id=$(xwininfo -root -tree 2>/dev/null \
         | awk -v pat="(\"$class\"" 'index($0, pat) { print $1; exit }')
    if [ -n "$id" ]; then echo "$id"; return 0; fi
    pause 0.5
  done
  echo "find_window: no window with class $class" >&2
  return 1
}

# check_no_config_error <class>
#
# (Task 5 review round 2, N1) A backslash-escaped WINDOW_DECORATIONS value
# (gen-config.sh's own guard) is one way to produce a config wezterm
# rejects, but it is not the only one — the reviewer demonstrated a second,
# live example this round (`&#124;`, valid Lua but an invalid
# WindowDecorations value) that the backslash guard does not and cannot
# catch, because the failure is general: ANY invalid value for ANY config
# key makes wezterm reject the whole struct and fall back to hardcoded
# defaults for every key, including front_end — so a GL-vs-PureCpu
# comparison silently becomes a backend-vs-itself comparison and reports a
# reassuring AE=0/PAE=0. Guarding one spelling of one variable (as C1's fix
# did) cannot generalise; catching it here does, because every `launch`
# already writes a log and wezterm logs "Configuration Error" there
# (confirmed for both a Lua syntax error and a struct-conversion failure)
# regardless of which config key was bad. Task 6 introduces more
# config-driven env vars (CURSOR_BLINK_RATE, TEXT_BLINK_RATE,
# DEFAULT_CURSOR_STYLE) and would otherwise inherit this gap.
#
# Fails rather than warns: numbers from a run whose front_end was silently
# ignored are worse than no numbers, because they look like a real
# comparison instead of announcing that they are not one.
check_no_config_error() {
  local class="$1"
  local log="$PARITY_OUT/$class.log"
  if grep -q "Configuration Error" "$log" 2>/dev/null; then
    echo "check_no_config_error: $log contains a Configuration Error --" \
         "wezterm rejected the whole config and fell back to hardcoded" \
         "defaults for every key, including front_end. Any numbers from" \
         "this run would compare a backend against itself and are worse" \
         "than no numbers, because they look like a real comparison. Fix" \
         "the config value that caused it; see $log for wezterm's own" \
         "error text." >&2
    return 1
  fi
  return 0
}

capture_settled() {  # capture_settled <winid> <outfile>
  #
  # (fix round 2, Task 4 NEW-3) capture_settled defines "settled" as two
  # captures 0.4s apart being identical — which is also true of a window
  # that hasn't started drawing anything yet. A corpus whose content takes
  # many seconds to actually reach the framebuffer (e.g. an image large
  # enough that the atlas needs an expensive grow/retry cycle) can look
  # "settled" on its first 0.4s poll purely because it's still blank, long
  # before real rendering begins — capturing a false-early snapshot rather
  # than the real result. `PARITY_SETTLE_WARMUP` (seconds, default 0) pauses
  # before the first poll so slow-to-start content has a chance to actually
  # start; it does not change behaviour for any existing case that doesn't
  # set it.
  local id="$1" out="$2" prev="${2%.png}-prev.png" i diff
  local warmup="${PARITY_SETTLE_WARMUP:-0}"
  if [ "$warmup" != "0" ]; then
    pause "$warmup"
  fi
  import -window "$id" "$prev"
  for i in $(seq 20); do
    pause 0.4
    import -window "$id" "$out"
    diff=$(compare -metric AE "$prev" "$out" null: 2>&1 || true)
    if [ "$diff" = "0" ]; then rm -f "$prev"; return 0; fi
    mv "$out" "$prev"
  done
  mv "$prev" "$out"
  echo "capture_settled: window $id never settled" >&2
  return 1
}

kill_class() {  # kill_class <class>
  pkill -f -- "--class $1" || true
}

# sample_region <winid> <crop-geometry> <count> <interval>
#
# `capture_settled` cannot measure animation: it *defines* success as two
# consecutive captures, `$interval` apart, being pixel-identical, so it is
# blind by construction to anything that is still changing when the driver
# looks at it — an animated GIF frame, a blinking cursor, blinking text, a
# ringing visual bell. Waiting for those to "settle" either times out or,
# worse, silently reports whatever frame the sampling luckily landed on.
#
# `sample_region` takes the opposite approach on purpose: instead of waiting
# for stability, it samples one pixel of `<crop-geometry>` (an ImageMagick
# geometry string, e.g. `4x4+14+262`) `<count>` times, `<interval>` seconds
# apart, and prints the raw colour sequence — space-separated, one call per
# import — with no judgement about whether it changed. The caller (or a
# human) compares the two backends' sequences directly. Added in Task 4 fix
# round 1 (CRIT-2) to demonstrate the animated-GIF idle-skip defect; Tasks 5
# and 6 reuse it as-is for cursor blink, blink-attribute text and the visual
# bell, which share the same do_paint_purecpu idle-skip mechanism
# (termwindow/mod.rs:1436-1447).
sample_region() {
  local id="$1" geom="$2" count="$3" interval="$4" i color
  for i in $(seq "$count"); do
    color=$(import -window "$id" -crop "$geom" +repage png:- 2>/dev/null \
             | convert - -format '%[pixel:p{0,0}]' info:)
    printf '%s ' "$color"
    pause "$interval"
  done
}
