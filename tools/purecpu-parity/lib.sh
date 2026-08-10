#!/usr/bin/env bash
# Shared helpers for the PureCpu parity harness.
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
pause() { timeout "$1" cat </dev/null || true; }

launch() {  # launch <class> <config> <shell-command>
  local class="$1" config="$2" cmd="$3"
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
