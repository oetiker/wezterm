#!/usr/bin/env bash
# cpu-case.sh <class> <config> <corpus-script> [seconds]
#
# Sample one arm's CPU consumption. Launches a single wezterm window with the
# given config and corpus, lets it settle, then reads utime+stime out of
# /proc/<pid>/stat twice, <seconds> apart, and prints:
#
#   <class> pid=<pid> ticks=<delta> seconds=<n> cpu_pct=<pct>
#
# Added for the measurement round (Task 10 Step 6's method, which the plan
# describes in a comment but never implemented as a script).
#
# WHY A SCRIPT AND NOT AN AD-HOC PIPELINE. Three traps this encodes so nobody
# has to remember them:
#
#  1. THE PID. findings.md C1 records that `pgrep ... | head -1` picks the
#     wrong process in this project and has already produced a wrong number
#     once. The gui process is the one with the largest RSS. We select on RSS
#     and print the pid so the choice is auditable rather than implicit.
#  2. THE SETTLE. First paint, font rasterisation and atlas population are
#     one-off costs that dwarf the steady-state signal over a 30s window.
#     Sampling starts only after SETTLE seconds.
#  3. SEQUENTIAL, ONE WINDOW AT A TIME. Focus state is a confound in any
#     two-window comparison on this display, and two windows also contend for
#     the same CPU. Each arm runs alone; the caller sequences them.
#
# This measures ONE arm. It deliberately does not know about floor/subject/
# ceiling — the caller supplies WEZTERM_BIN and the config, so which arm is
# which stays visible at the call site instead of being buried here.
set -euo pipefail
source "$(dirname "$0")/lib.sh"

CLASS="$1"; CONFIG="$2"; CORPUS="$3"
SECS="${4:-30}"
SETTLE="${SETTLE:-8}"

kill_class "$CLASS"
launch "$CLASS" "$CONFIG" "$CORPUS"
W=$(find_window "$CLASS")
check_no_config_error "$CLASS"
check_bell_disabled "$CONFIG"

pause "$SETTLE"

# The gui process is the largest-RSS process whose command line carries our
# class. `ps` over an explicit pid list rather than `pgrep | head -1`.
pids=$(pgrep -f -- "--class $CLASS" || true)
if [ -z "$pids" ]; then
  echo "cpu-case.sh: no process found for class $CLASS" >&2
  kill_class "$CLASS"
  exit 1
fi
PID=$(ps -o pid=,rss= -p $(echo "$pids" | tr '\n' ',' | sed 's/,$//') \
      | sort -k2 -n | tail -1 | awk '{print $1}')

read_cpu() {  # read_cpu <pid> -> utime+stime in clock ticks
  awk '{print $14 + $15}' "/proc/$1/stat"
}

t0=$(read_cpu "$PID")
pause "$SECS"
t1=$(read_cpu "$PID")

kill_class "$CLASS"

ticks=$((t1 - t0))
hz=$(getconf CLK_TCK)
pct=$(awk -v t="$ticks" -v s="$SECS" -v hz="$hz" 'BEGIN { printf "%.2f", (t / hz) * 100.0 / s }')
printf '%s pid=%s ticks=%s seconds=%s hz=%s cpu_pct=%s\n' \
  "$CLASS" "$PID" "$ticks" "$SECS" "$hz" "$pct"
