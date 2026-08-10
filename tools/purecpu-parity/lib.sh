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

capture_settled() {  # capture_settled <winid> <outfile>
  local id="$1" out="$2" prev="${2%.png}-prev.png" i diff
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
