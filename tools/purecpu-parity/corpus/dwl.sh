#!/usr/bin/env bash
# corpus/dwl.sh — DECDWL / DECDHL double-width and double-height lines,
# for the "Double-width / double-height lines" matrix row.
#
# The escapes are line attributes: `\033#6` (DECDWL), `\033#3`/`\033#4`
# (DECDHL top/bottom half) set a bit on the line the cursor is *currently*
# on (term/src/terminalstate/performer.rs:635-650), so each one must be
# emitted while the cursor sits on the line it is meant to affect, before
# that line's text. `\033#5` (DECSWL) is emitted explicitly on the control
# lines rather than relied on by default, so a line's attribute can never
# leak from whatever ran before.
#
# The corpus carries its own positive control: the SAME text appears once as
# an ordinary single-width line and once as a double-width line. If the two
# render identically within a backend, the escape never reached
# `Line::set_double_width` and the case is measuring nothing — see the
# "reachability control" note in the matrix row. A parity result on this
# corpus is only meaningful if that control shows a difference first.
set -euo pipefail

# Control line: explicitly single-width, same text as the DECDWL line below.
printf '\033#5CONTROL SINGLE WIDTH 0123456789\n'

# DECDWL: double-width, single-height.
printf '\033#6DECDWL DOUBLE WIDTH 0123456789\n'

# DECDHL: the same text twice, top half then bottom half. Both halves are
# implicitly double-width too (set_double_height_* inserts DOUBLE_WIDTH).
printf '\033#3DECDHL DOUBLE HEIGHT 01234\n'
printf '\033#4DECDHL DOUBLE HEIGHT 01234\n'

# A second single-width line, to show the grid returns to normal afterwards.
printf '\033#5CONTROL SINGLE WIDTH AGAIN\n'

exec sleep 600
