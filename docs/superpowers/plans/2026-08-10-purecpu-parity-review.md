# PureCpu Parity Review Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce a defect findings list for the fork patch and an evidence-backed parity matrix for the PureCpu renderer against the GPU backend.

**Architecture:** A static audit enumerates what the GPU path emits and what PureCpu consumes, producing a parity matrix skeleton. A shell harness then renders identical content under two `front_end` settings on an isolated X display, captures both windows, and compares them. Comparison is calibrated against a measured noise floor, because the two backends are not expected to be bit-identical.

**Tech Stack:** bash, ImageMagick (`import`, `compare`, `convert`), `xwininfo`, ThinLinc `Xvnc`, `marco`, wezterm Lua config.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-08-10-purecpu-parity-review-design.md`.
- Branch: `update-optimization-rebased`. Fork base: upstream `e723cf5`.
- Binary under test: `/scratch/oetiker/wezterm-builds/wezterm-gui-rebased`.
- Display: `:20`, auth file `$PARITY_XAUTH`. **Never touch displays `:10`–`:14`** — they belong to other users on this shared machine.
- Build parallelism: never more than 4 cores.
- No fixes in this plan. Tasks 1–8 produce documents and a harness only.
- Parity scope: inline images; tab bar and window chrome; cursor and text animation.
- Known gaps, recorded but not investigated: `window_background_image`, `window_background_opacity`, `text_background_opacity`, background blur/HSB tint.
- Verdict vocabulary, used verbatim: `parity`, `degraded`, `missing`, `known gap`.
- Every non-`known gap` verdict cites evidence: a capture pair path, or the code path making the behaviour impossible.

---

### Task 1: Static audit and parity matrix skeleton

No runtime. This task answers the question screenshots cannot: whether content exists that PureCpu structurally cannot draw.

**Files:**
- Create: `docs/purecpu-review/parity-matrix.md`

**Interfaces:**
- Produces: `docs/purecpu-review/parity-matrix.md` with one row per feature and a `Method` column valued `by-reading` or `needs-measurement`. Later tasks fill the `Verdict` and `Evidence` columns for `needs-measurement` rows.

- [ ] **Step 1: Enumerate GPU-path texture sources**

Read `wezterm-gui/src/renderstate.rs` and `wezterm-gui/src/termwindow/render/`. Record every distinct texture the GPU path samples, and every quad producer that sets texture coordinates.

```bash
cd /scratch/oetiker/wezterm
grep -rn "fn allocate\|texture()\|set_texture\|tex_coords\|cached_image" \
  wezterm-gui/src/renderstate.rs wezterm-gui/src/glyphcache.rs \
  wezterm-gui/src/termwindow/render/*.rs | tee /tmp/gpu-textures.txt
```

- [ ] **Step 2: Determine what PureCpu consumes**

`call_draw_purecpu` acquires one texture at `wezterm-gui/src/termwindow/render/purecpu.rs:154`. Confirm whether any other texture can reach a quad, and what happens to a quad whose texture is not the glyph atlas — is it drawn from the wrong texture, or skipped?

```bash
sed -n '150,200p;280,420p' wezterm-gui/src/termwindow/render/purecpu.rs
```

- [ ] **Step 3: Answer the `Software` front-end question**

`FrontEndSelection` already has a `Software` variant. Determine what it does and how it differs from `PureCpu`, so the review can state what PureCpu adds.

```bash
grep -rn "Software" config/src/frontend.rs wezterm-gui/src/frontend.rs \
  wezterm-gui/src/termwindow/mod.rs wezterm-gui/src/renderstate.rs
```

- [ ] **Step 4: Write the matrix skeleton**

Create `docs/purecpu-review/parity-matrix.md` with this exact table structure, one row per feature found in Steps 1–3, plus the four known-gap rows:

```markdown
# PureCpu Parity Matrix

Backend under test: `front_end = "PureCpu"`
Reference: `front_end = "OpenGL"` (Mesa llvmpipe 4.5)

| Feature | GPU behaviour | PureCpu behaviour | Method | Verdict | Evidence |
|---|---|---|---|---|---|
| Text glyphs (monochrome) | | | needs-measurement | | |
| Inline image: sixel | | | needs-measurement | | |
| Inline image: iTerm2 OSC 1337 | | | needs-measurement | | |
| Inline image: animated GIF | | | needs-measurement | | |
| Fancy tab bar | | | needs-measurement | | |
| Window buttons | | | needs-measurement | | |
| Rounded corners | | | needs-measurement | | |
| Split dividers | | | needs-measurement | | |
| Cursor (static) | | | needs-measurement | | |
| Cursor blink easing | | | needs-measurement | | |
| Blinking text attribute | | | needs-measurement | | |
| Visual bell | | | needs-measurement | | |
| window_background_image | n/a | n/a | out-of-scope | known gap | user decision, not investigated |
| window_background_opacity | n/a | n/a | out-of-scope | known gap | user decision, not investigated |
| text_background_opacity | n/a | n/a | out-of-scope | known gap | user decision, not investigated |
| Background blur / HSB tint | n/a | n/a | out-of-scope | known gap | user decision, not investigated |
```

Fill the `GPU behaviour` and `PureCpu behaviour` columns from reading. Where reading alone settles a row (for example, a texture that provably cannot reach the rasteriser), set `Method` to `by-reading`, write the verdict, and cite the file:line as evidence.

- [ ] **Step 5: Commit**

```bash
cd /scratch/oetiker/wezterm
git add docs/purecpu-review/parity-matrix.md
git commit -m "docs: parity matrix skeleton from static audit"
```

---

### Task 2: Harness foundation

**Files:**
- Create: `tools/purecpu-parity/lib.sh`
- Create: `tools/purecpu-parity/gen-config.sh`
- Create: `tools/purecpu-parity/corpus/plain.sh`

**Interfaces:**
- Produces: `lib.sh` exporting `launch <class> <config> <cmd>`, `find_window <class>` (echoes window id), `capture_settled <winid> <outfile>` (exit 0 = settled, 1 = never settled), `pause <seconds>`.
- Produces: `gen-config.sh <front_end> <outfile>` writing a wezterm Lua config pinning that backend.

- [ ] **Step 1: Write `tools/purecpu-parity/lib.sh`**

```bash
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
  WEZTERM_CONFIG_FILE="$config" setsid nohup "$WEZTERM_BIN" start \
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
```

- [ ] **Step 2: Write `tools/purecpu-parity/gen-config.sh`**

Colours are pinned explicitly rather than by scheme name, so the comparison cannot drift with wezterm's bundled schemes.

Two optional environment variables let later tasks reuse this one generator:
`FANCY` (default `true`) selects the fancy or retro tab bar, and `SPAWN_TABS`
(default `false`) adds a `gui-startup` handler that creates extra tabs and a
split for the chrome case.

```bash
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
```

- [ ] **Step 3: Write the plain-text corpus**

```bash
#!/usr/bin/env bash
# corpus/plain.sh — deterministic text, no animation, no images.
printf 'ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz\n'
printf '0123456789 !"#$%%&()*+,-./:;<=>?@[]^_{|}~\n'
printf 'ligatures: // != == -> <= >= ... =~\n'
printf 'box: \xe2\x94\x8c\xe2\x94\x80\xe2\x94\x90 \xe2\x94\x94\xe2\x94\x80\xe2\x94\x98 blocks: \xe2\x96\x88\xe2\x96\x93\xe2\x96\x92\xe2\x96\x91\n'
printf 'bold: \033[1mBOLD\033[0m italic: \033[3mITALIC\033[0m under: \033[4mUNDER\033[0m\n'
printf 'colors: \033[31mR\033[32mG\033[34mB\033[0m bg: \033[41mR\033[42mG\033[44mB\033[0m\n'
exec sleep 600
```

- [ ] **Step 4: Verify the harness launches and captures both backends**

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
chmod +x lib.sh gen-config.sh corpus/plain.sh
export PARITY_XAUTH=<path to :20 Xauthority>
source ./lib.sh
./gen-config.sh OpenGL  "$PARITY_OUT/opengl.lua"
./gen-config.sh PureCpu "$PARITY_OUT/purecpu.lua"
launch par-gl "$PARITY_OUT/opengl.lua"  "$PWD/corpus/plain.sh"
launch par-cpu "$PARITY_OUT/purecpu.lua" "$PWD/corpus/plain.sh"
GL=$(find_window par-gl); CPU=$(find_window par-cpu)
capture_settled "$GL"  "$PARITY_OUT/plain-gl.png"
capture_settled "$CPU" "$PARITY_OUT/plain-cpu.png"
identify "$PARITY_OUT/plain-gl.png" "$PARITY_OUT/plain-cpu.png"
```

Expected: both windows found, both captures settle (exit 0), both PNGs the same dimensions. If dimensions differ, stop and fix the config — comparison requires identical geometry.

- [ ] **Step 5: Confirm each instance really ran the intended backend**

The smoke test earlier could not tell which backend it used. Verify explicitly:

```bash
grep -i "front.end\|renderer\|purecpu\|opengl" "$PARITY_OUT/par-gl.log" "$PARITY_OUT/par-cpu.log"
for p in $(pgrep -u "$USER" -f -- "--class par-"); do
  echo "== $p"; tr '\0' '\n' < /proc/$p/environ | grep WEZTERM_CONFIG_FILE
done
```

Expected: each process points at its own generated config. If the logs do not state the backend, add `WEZTERM_LOG=info` to `launch` and re-run until the backend is confirmed in the log. Do not proceed on assumption.

- [ ] **Step 6: Commit**

```bash
cd /scratch/oetiker/wezterm
git add tools/purecpu-parity
git commit -m "test: PureCpu parity harness foundation"
```

---

### Task 3: Noise-floor calibration (gate)

This task decides whether the rest of the measurement is meaningful. **If it fails, stop and report — do not tune thresholds until diffs look acceptable.**

**Files:**
- Create: `tools/purecpu-parity/calibrate.sh`
- Create: `docs/purecpu-review/noise-floor.md`

**Interfaces:**
- Consumes: `lib.sh`, `gen-config.sh`, `corpus/plain.sh` from Task 2.
- Produces: `docs/purecpu-review/noise-floor.md` stating the fuzz percentage later tasks use, named `FUZZ`.

- [ ] **Step 1: Write `tools/purecpu-parity/calibrate.sh`**

```bash
#!/usr/bin/env bash
# Measure the difference between backends on static plain text.
set -euo pipefail
source "$(dirname "$0")/lib.sh"

GL_PNG="$PARITY_OUT/plain-gl.png"
CPU_PNG="$PARITY_OUT/plain-cpu.png"

echo "== absolute pixel difference (AE), no fuzz =="
compare -metric AE "$GL_PNG" "$CPU_PNG" null: 2>&1 || true; echo

echo "== root mean squared (RMSE) =="
compare -metric RMSE "$GL_PNG" "$CPU_PNG" null: 2>&1 || true; echo

echo "== peak absolute (PAE): worst single-channel deviation =="
compare -metric PAE "$GL_PNG" "$CPU_PNG" null: 2>&1 || true; echo

echo "== AE at increasing fuzz =="
for f in 1 2 3 5 8 12; do
  n=$(compare -metric AE -fuzz "${f}%" "$GL_PNG" "$CPU_PNG" null: 2>&1 || true)
  echo "fuzz=${f}% differing_pixels=${n}"
done

echo "== difference heat map =="
compare "$GL_PNG" "$CPU_PNG" "$PARITY_OUT/plain-diff.png" 2>/dev/null || true
echo "wrote $PARITY_OUT/plain-diff.png"
```

- [ ] **Step 2: Run calibration**

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity && ./calibrate.sh
```

- [ ] **Step 3: Inspect the heat map and decide**

View `out/plain-diff.png`. Apply this decision rule:

- **Proceed** if differences concentrate on glyph edges and the differing-pixel count collapses sharply as fuzz rises (that is antialiasing noise). Set `FUZZ` to the smallest percentage where the count approaches zero, then add 2 points of margin.
- **Stop and report** if differences are spatially structured — whole cells, offset text, uniform background shifts, or a channel-wide bias. That is a real rendering difference, not noise, and it must be characterised as a finding before any image comparison can be trusted.
- **Stop and report** if no fuzz value collapses the count. The harness cannot separate signal from noise; say so.

- [ ] **Step 4: Write `docs/purecpu-review/noise-floor.md`**

Record: the four metric values, the fuzz sweep table, the chosen `FUZZ`, the reasoning, and an inline reference to the heat map. If the decision was to stop, record that instead — the document is written either way.

- [ ] **Step 5: Commit**

```bash
cd /scratch/oetiker/wezterm
git add tools/purecpu-parity/calibrate.sh docs/purecpu-review/noise-floor.md
git commit -m "test: calibrate PureCpu/OpenGL rendering noise floor"
```

---

### Task 4: Inline images

The highest-risk area. Task 1 may already have settled it by reading; this task confirms or refutes that with pixels.

**Files:**
- Create: `tools/purecpu-parity/corpus/images.sh`
- Create: `tools/purecpu-parity/compare-case.sh`
- Modify: `docs/purecpu-review/parity-matrix.md`

**Interfaces:**
- Consumes: `lib.sh`, `FUZZ` from `docs/purecpu-review/noise-floor.md`.
- Produces: `compare-case.sh <case> <class-gl> <class-cpu>`, reusable by Tasks 5 and 6.

- [ ] **Step 1: Write `tools/purecpu-parity/corpus/images.sh`**

`wezterm imgcat` is unavailable (the mux CLI is not built), so the iTerm2 sequence is emitted by hand.

```bash
#!/usr/bin/env bash
# corpus/images.sh — sixel, iTerm2 inline image, animated GIF.
set -euo pipefail
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

convert -size 64x64 gradient:red-blue "$TMP/img.png"
convert -size 32x32 xc:green -delay 20 -size 32x32 xc:yellow "$TMP/anim.gif"

printf 'SIXEL:\n'
convert "$TMP/img.png" sixel:- || printf '(sixel conversion failed)\n'
printf '\nITERM2:\n'
b64=$(base64 -w0 < "$TMP/img.png")
printf '\033]1337;File=inline=1;width=64px;height=64px:%s\a\n' "$b64"
printf '\nANIMATED GIF:\n'
b64g=$(base64 -w0 < "$TMP/anim.gif")
printf '\033]1337;File=inline=1;width=32px;height=32px:%s\a\n' "$b64g"
printf '\nEND\n'
exec sleep 600
```

- [ ] **Step 2: Write `tools/purecpu-parity/compare-case.sh`**

```bash
#!/usr/bin/env bash
# compare-case.sh <case-name> <corpus-script>
set -euo pipefail
source "$(dirname "$0")/lib.sh"
CASE="$1"; CORPUS="$2"
FUZZ="${FUZZ:?set FUZZ from docs/purecpu-review/noise-floor.md}"

"$PARITY_DIR/gen-config.sh" OpenGL  "$PARITY_OUT/opengl.lua"
"$PARITY_DIR/gen-config.sh" PureCpu "$PARITY_OUT/purecpu.lua"
kill_class "par-$CASE-gl"; kill_class "par-$CASE-cpu"

launch "par-$CASE-gl"  "$PARITY_OUT/opengl.lua"  "$CORPUS"
launch "par-$CASE-cpu" "$PARITY_OUT/purecpu.lua" "$CORPUS"
GL=$(find_window "par-$CASE-gl"); CPU=$(find_window "par-$CASE-cpu")
capture_settled "$GL"  "$PARITY_OUT/$CASE-gl.png"  || echo "WARN: gl never settled"
capture_settled "$CPU" "$PARITY_OUT/$CASE-cpu.png" || echo "WARN: cpu never settled"

echo "== $CASE: differing pixels at fuzz ${FUZZ}% =="
compare -metric AE -fuzz "${FUZZ}%" \
  "$PARITY_OUT/$CASE-gl.png" "$PARITY_OUT/$CASE-cpu.png" null: 2>&1 || true; echo
compare "$PARITY_OUT/$CASE-gl.png" "$PARITY_OUT/$CASE-cpu.png" \
  "$PARITY_OUT/$CASE-diff.png" 2>/dev/null || true

# Ink coverage per backend: catches "drew nothing at all".
for side in gl cpu; do
  printf '%s ink stddev: ' "$side"
  convert "$PARITY_OUT/$CASE-$side.png" -format '%[standard-deviation]' info:; echo
done
kill_class "par-$CASE-gl"; kill_class "par-$CASE-cpu"
```

The ink-coverage check matters: if PureCpu draws no image, the diff is large *and* its stddev is markedly lower. That distinguishes "missing" from "degraded".

- [ ] **Step 3: Run the image case**

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
chmod +x corpus/images.sh compare-case.sh
FUZZ=<value from noise-floor.md> ./compare-case.sh images "$PWD/corpus/images.sh"
```

- [ ] **Step 4: Inspect captures and classify**

View `out/images-gl.png`, `out/images-cpu.png`, `out/images-diff.png`. Assign a verdict per image row in the matrix:

- Image visible in both, differences within `FUZZ` → `parity`
- Image visible in both, differences beyond `FUZZ` → `degraded`, describing how
- Image visible only under OpenGL → `missing`

Sixel, iTerm2, and animated GIF get **separate** verdicts. Do not generalise from one to the others; they may take different code paths.

- [ ] **Step 5: Update the matrix and commit**

Fill `Verdict` and `Evidence` for the three image rows. Evidence is the capture pair path plus, for `missing`, the file:line that explains it.

```bash
cd /scratch/oetiker/wezterm
git add tools/purecpu-parity docs/purecpu-review/parity-matrix.md
git commit -m "test: measure inline image parity for PureCpu"
```

---

### Task 5: Tab bar and window chrome

**Files:**
- Create: `tools/purecpu-parity/corpus/chrome.sh`
- Modify: `docs/purecpu-review/parity-matrix.md`

**Interfaces:**
- Consumes: `compare-case.sh` from Task 4, `FUZZ` from Task 3.

- [ ] **Step 1: Write `tools/purecpu-parity/corpus/chrome.sh`**

Multiple tabs and a split are required to exercise the tab bar and dividers.
The mux CLI is not built, so they are created from the config itself via the
`gui-startup` event, which needs no interactive input and is identical across
both backends. This is already supported by `gen-config.sh` from Task 2 through
the `SPAWN_TABS` environment variable — no new generator is needed.

The corpus itself stays trivial, because the window structure comes from the
config:

```bash
#!/usr/bin/env bash
# corpus/chrome.sh — exercise tab bar, window buttons, splits.
printf 'CHROME CASE\n'
printf 'tab bar, window buttons, rounded corners, split divider\n'
exec sleep 600
```

`SPAWN_TABS=true` yields three tabs with a vertical split in the first — enough
for tab bar, active/inactive tab styling, and divider rendering. If
`gui-startup` does not fire (check the launch log for a Lua error), record the
split-divider row as `known gap` with that reason rather than leaving it
unmeasured.

- [ ] **Step 2: Run the fancy tab bar case**

`compare-case.sh` passes `FANCY` and `SPAWN_TABS` through to `gen-config.sh`, so
both variants run through the same path:

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
chmod +x corpus/chrome.sh
FANCY=true SPAWN_TABS=true FUZZ=<value> ./compare-case.sh chrome "$PWD/corpus/chrome.sh"
```

- [ ] **Step 3: Run the retro tab bar case**

The fancy and retro tab bars are different draw paths, so both are covered:

```bash
FANCY=false SPAWN_TABS=true FUZZ=<value> ./compare-case.sh chrome-retro "$PWD/corpus/chrome.sh"
```

This writes `chrome-retro-gl.png`, `chrome-retro-cpu.png`, and
`chrome-retro-diff.png` alongside the fancy captures.

- [ ] **Step 4: Update the matrix and commit**

Fill verdicts for fancy tab bar, window buttons, rounded corners, and split dividers.

```bash
cd /scratch/oetiker/wezterm
git add tools/purecpu-parity docs/purecpu-review/parity-matrix.md
git commit -m "test: measure tab bar and window chrome parity"
```

---

### Task 6: Cursor and text animation

Animation cannot be diffed frame-for-frame across backends. Each row is assessed as a **sampled state**, and the matrix says so.

**Files:**
- Create: `tools/purecpu-parity/corpus/cursor.sh`
- Create: `tools/purecpu-parity/sample-animation.sh`
- Modify: `docs/purecpu-review/parity-matrix.md`

**Interfaces:**
- Consumes: `lib.sh`, `FUZZ`.

- [ ] **Step 1: Write `tools/purecpu-parity/corpus/cursor.sh`**

```bash
#!/usr/bin/env bash
# corpus/cursor.sh — static cursor plus a blinking-attribute run.
printf 'CURSOR CASE\n'
printf 'blink attr: \033[5mBLINKING\033[0m normal\n'
printf 'cursor rests at the end of this line: '
exec sleep 600
```

- [ ] **Step 2: Static cursor comparison**

With `cursor_blink_rate = 0` (already pinned), the cursor is steady, so the standard comparison applies:

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
chmod +x corpus/cursor.sh
FUZZ=<value> ./compare-case.sh cursor "$PWD/corpus/cursor.sh"
```

- [ ] **Step 3: Write `tools/purecpu-parity/sample-animation.sh`**

Blink parity is assessed *within* a backend: does the cursor actually change over time, and does it come to rest? Comparing a blink phase across backends is meaningless because the phases are not synchronised.

```bash
#!/usr/bin/env bash
# sample-animation.sh <class> <winid> <n-samples>
# Captures N frames and reports how many distinct frames occurred.
set -euo pipefail
source "$(dirname "$0")/lib.sh"
CLASS="$1"; WIN="$2"; N="${3:-12}"
prev=""
distinct=0
for i in $(seq "$N"); do
  f="$PARITY_OUT/$CLASS-anim-$i.png"
  import -window "$WIN" "$f"
  if [ -n "$prev" ]; then
    d=$(compare -metric AE "$prev" "$f" null: 2>&1 || true)
    [ "$d" != "0" ] && distinct=$((distinct+1))
  fi
  prev="$f"
  pause 0.25
done
echo "$CLASS: $distinct changes across $N samples"
```

- [ ] **Step 4: Run the blink sample under both backends**

Generate configs with blinking enabled, launch both backends, resolve the window
ids, then sample:

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
source ./lib.sh
chmod +x sample-animation.sh

for fe in OpenGL PureCpu; do
  ./gen-config.sh "$fe" "$PARITY_OUT/blink-$fe.lua"
  sed -i 's/cursor_blink_rate = 0/cursor_blink_rate = 500/;
          s/text_blink_rate = 0/text_blink_rate = 500/;
          s/animation_fps = 1/animation_fps = 30/' "$PARITY_OUT/blink-$fe.lua"
done

launch par-blink-gl  "$PARITY_OUT/blink-OpenGL.lua"  "$PWD/corpus/cursor.sh"
launch par-blink-cpu "$PARITY_OUT/blink-PureCpu.lua" "$PWD/corpus/cursor.sh"
GLWIN=$(find_window par-blink-gl)
CPUWIN=$(find_window par-blink-cpu)

./sample-animation.sh par-blink-gl  "$GLWIN"  12
./sample-animation.sh par-blink-cpu "$CPUWIN" 12
kill_class par-blink-gl; kill_class par-blink-cpu
```

Note that `capture_settled` is deliberately *not* used here: a blinking window
never settles, which is the property under test.

Expected under both: a non-zero change count. A count of 0 on PureCpu with a non-zero count on OpenGL means blinking does not animate — likely the idle-skip path in `do_paint_purecpu` suppressing the repaint. Record that as `missing` and cite `wezterm-gui/src/termwindow/mod.rs:1182`.

- [ ] **Step 5: Visual bell**

Add `visual_bell = { fade_in_duration_ms = 300, fade_out_duration_ms = 300 }` to both configs, emit `printf '\a'` from the corpus, and sample as in Step 4. A zero change count on PureCpu is `missing`.

- [ ] **Step 6: Update the matrix and commit**

```bash
cd /scratch/oetiker/wezterm
git add tools/purecpu-parity docs/purecpu-review/parity-matrix.md
git commit -m "test: measure cursor and text animation parity"
```

---

### Task 7: Defect findings review

Independent of parity. Reviews the patch as code.

**Files:**
- Create: `docs/purecpu-review/findings.md`

**Interfaces:**
- Produces: `docs/purecpu-review/findings.md`, findings ranked most severe first.

- [ ] **Step 1: Review the renderer**

Read `wezterm-gui/src/termwindow/render/purecpu.rs` and the `termwindow/mod.rs` integration in full. Look specifically for: panics and `unwrap` on terminal-controlled input; integer overflow and sign errors in the coordinate maths around `purecpu.rs:280-420`; out-of-bounds framebuffer indexing; dirty-rect logic that can leave stale pixels; and unbounded growth in `PureCpuState`.

```bash
cd /scratch/oetiker/wezterm
git diff e723cf5..HEAD -- wezterm-gui/src/termwindow/render/purecpu.rs \
  wezterm-gui/src/termwindow/mod.rs | head -400
```

- [ ] **Step 2: Review the font stack**

```bash
git diff e723cf5..HEAD --stat -- wezterm-font/
```

Read `skrifa_rasterizer.rs`, `harfrust_shaper.rs`, and `skia_colr.rs`. Look for: unwrap on malformed font data (fonts are attacker-controlled input); unbounded caches; and behaviour differences from the FreeType path that would change rendering, especially hinting, bitmap-strike selection, and synthetic bold/italic.

- [ ] **Step 3: Review the X11 changes**

```bash
git diff e723cf5..HEAD -- window/src/os/x11/
```

Check the `PutImage` chunking for off-by-one errors on the final chunk, and the Expose re-present path for redundant full repaints.

- [ ] **Step 4: Run the existing tests**

```bash
cd /scratch/oetiker/wezterm
CARGO_BUILD_JOBS=4 cargo test -j4 -p wezterm-gui --lib 2>&1 | tail -30
```

Record failures as findings. Note that `purecpu.rs` already carries unit tests for `blend_over`, `coalesce_to_bands`, `collect_clip_rects`, and the sRGB round trip; assess whether they cover the rasterising path at all, and say so if they do not.

- [ ] **Step 5: Write `docs/purecpu-review/findings.md`**

Each finding: file:line, one-sentence defect statement, a concrete failure scenario with inputs, and severity. No speculative findings — if it cannot be shown to fail, mark it explicitly as unverified.

- [ ] **Step 6: Commit**

```bash
cd /scratch/oetiker/wezterm
git add docs/purecpu-review/findings.md
git commit -m "docs: PureCpu and font stack defect findings"
```

---

### Task 8: Consolidation

**Files:**
- Modify: `docs/purecpu-review/parity-matrix.md`
- Create: `docs/purecpu-review/README.md`

- [ ] **Step 1: Verify no matrix row is unresolved**

```bash
grep -n "needs-measurement" docs/purecpu-review/parity-matrix.md
```

Expected: no output. Any remaining row must either get a verdict or be restated as `known gap` with the reason it could not be measured.

- [ ] **Step 2: Verify every verdict cites evidence**

```bash
awk -F'|' '/^\|/ && $6 ~ /parity|degraded|missing/ && $7 !~ /[a-z]/ {print "no evidence:", $0}' \
  docs/purecpu-review/parity-matrix.md
```

Expected: no output.

- [ ] **Step 3: Write `docs/purecpu-review/README.md`**

A one-page summary: what was measured, the headline parity result, the top findings, the known gaps, and how to re-run the harness (`tools/purecpu-parity`, display `:20` setup). State plainly what was *not* covered: background image and transparency, non-X11 platforms, and any row that could not be measured.

- [ ] **Step 4: Commit**

```bash
cd /scratch/oetiker/wezterm
git add docs/purecpu-review
git commit -m "docs: PureCpu parity review summary"
```

---

## Environment setup (prerequisite for Tasks 2–6)

Display `:20` and `marco` are already running from the design session. If they
are gone, recreate them:

```bash
COOKIE=$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')
xauth -f "$PARITY_XAUTH" add :20 MIT-MAGIC-COOKIE-1 "$COOKIE"
setsid nohup /opt/thinlinc/libexec/Xvnc :20 -depth 24 -geometry 1280x1024 \
  -SecurityTypes None -localhost -rfbport 5920 -br -auth "$PARITY_XAUTH" \
  > /tmp/xvnc20.log 2>&1 < /dev/null &
DISPLAY=:20 XAUTHORITY="$PARITY_XAUTH" setsid nohup marco > /tmp/marco20.log 2>&1 < /dev/null &
```

Confirm before use: `DISPLAY=:20 XAUTHORITY=$PARITY_XAUTH xdpyinfo | grep dimensions`
