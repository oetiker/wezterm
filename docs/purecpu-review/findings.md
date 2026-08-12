# PureCpu fork: defect findings

Scope: the fork's own patch against upstream `e723cf5`, on branch
`update-optimization-rebased`, Linux/X11 only. Reviewed as code, not as
pixels — this document is independent of `parity-matrix.md`, though several
findings explain rows measured there and are cross-referenced.

Binary exercised for the probes below:
`/scratch/oetiker/wezterm-builds/wezterm-gui-rebased`. No Rust source was
modified: every probe is a corpus/config written outside the tree and run
against that binary. Probes are recorded so they can be re-run.

> **Disposition after the fix pass (Task 14).** This document was written as a
> review, before any fix existed, and its finding texts are left as they were
> written — they describe the defect as found. What is added per finding is a
> **Disposition** line naming the commit that addressed it, so the document does
> not read as current while describing a binary that no longer exists. The
> post-fix binary is `wezterm-gui-6311e97` (md5
> `3c3f5bef1b91c1f08a018c2da9e054d8`); the pre-fix reference kept as the control
> arm of every before/after comparison is `wezterm-gui-prefix`.
>
> **Three dispositions are deliberately not the word "fixed".** M3, M4 and M5
> are **panic sites guarded; trigger never reproduced** — no font reaching any
> of them was ever built, so nothing demonstrates a trigger now passing that
> used to abort. The guard is what changed; the evidence class did not.
>
> Ledger of dispositions: C1 `c5cc8ab` · I1/I2 `bc05562` · I3 `230117c` ·
> I4 `868159c` · I5/M1 `773b845` · M6/M7 `f78b501` · M3/M4/M5 `b10c3a5`
> (guarded) · M2 closed by re-measurement, no code change · L1 built, measured
> null, reverted · L2 `bce58ea` · L3 `230117c` · L4 obsolete (upstream
> `ef7b636`) · L5 closed on inspection, non-defect.
>
> **Two findings carry an `N` prefix: the fix pass found them, the review did
> not.** **N2** (Important) — glyph ink escaping a dirtied cell was left stale;
> **fixed** in `fbbb433`/`6311e97`, and it is the one defect in this pass that
> was **reproduced in pixels** rather than reasoned about, with a scoped
> limitation that remains open (animated-image ink is still not covered).
> **N1** (Low) — a static inline image costs 0.57 ms of CPU per frame;
> pre-existing, **unfixed**.

Severity vocabulary:

- **Critical** — denial of service, memory exhaustion, or process abort
  reachable from terminal output or from font data.
- **Important** — content is silently not drawn, or is drawn wrongly, in an
  ordinary configuration.
- **Medium** — visible rendering error, or a robustness hole with no
  demonstrated trigger.
- **Low** — performance, latent edge case, or hygiene.

**Demotion rule: a defect whose trigger is not reproduced is recorded one
class below its defect class.** This is what separates the three font-stack
panics (M3, M4, M5) from C1: all four are process aborts or memory exhaustion
reachable from attacker-controlled input, which is the Critical *defect*
class, but only C1's trigger was actually run. The rule is written down here
because the first draft of this document applied it silently and the ladder
then contradicted itself three times (review round 1, I-R2).

**The rule applies once, to the defect class, and does not stack** (Task 8,
Mi2). The Medium definition already reads "or a robustness hole with no
demonstrated trigger", which describes the *same* condition the demotion rule
tests, so applying the rule a second time to a finding that is *natively*
Medium (M6, M7) or natively Low (L3) would push it down a class for a reason
already priced into its definition. Read it as: establish the defect class
from what the defect *is*, demote by one if its trigger was not reproduced,
stop. That is what the ladder below actually does; this clause only removes
the ambiguity.

Each finding states its evidence class explicitly:

- **measured** — reproduced against the running binary, command included.
- **by reading** — the code path is unambiguous and the conclusion follows
  from it without a runtime trigger being demonstrated. Where a trigger is
  *plausible but not demonstrated*, the finding says so and names the
  evidence that is missing.

No fixes are proposed beyond a one-line direction where it is obvious; this
review produces documents only.

---

## Critical

### C1 — PureCpu grows the glyph atlas with no size ceiling; one ~1.5 KB escape sequence allocates 4 GiB

**Disposition: fixed in `c5cc8ab`.** The `PureCpu` arm was given a ceiling (`PURECPU_MAX_TEXTURE_SIZE = 8192`, `renderstate.rs:35`) and a `bail!`, so it now reaches the same `AllowImage::Scale` downscale-and-retry the `Glium` arm reaches — exactly the fix direction below. Confirmed in Task 14 from the renderer's own log: PureCpu now emits `Cannot use a texture of size 32768 as it is larger than the max 8192 supported by the PureCpu renderer; will retry render with Scale(2)`, where before its log carried no texture-space line at all. **The 8192 cap is half the 16384 this box's llvmpipe reports, and that asymmetry has its own consequence** — the two backends settle on different scale factors for the same oversized image. It is now a row in `parity-matrix.md` rather than an unexplained residual.

`wezterm-gui/src/renderstate.rs:78-118`,
`window/src/bitmaps/atlas.rs:113-119`, `window/src/bitmaps/atlas.rs:30-49`

**Defect.** `RenderContext::allocate_texture_atlas`'s `Glium` arm checks the
requested side against `caps.max_texture_size` and `bail!`s when it is
exceeded; the `PureCpu` arm has no cap and no fallible path at all — it calls
`ImageTexture::new(size, size)`, which allocates `size × size × 4` bytes of
system memory and cannot fail gracefully. The memory is **touched, not lazily
reserved**: `Image::new` is `vec![0; height * width * 4]`
(`window/src/bitmaps/mod.rs:337-346`) and `Atlas::new` (`atlas.rs:30-49`) then
builds a *second* full-size `Image` and `texture.write`s the whole rect.

**This arm is fork-introduced, not inherited.**
`git show e723cf5:wezterm-gui/src/renderstate.rs` has only `Glium` and
`WebGpu` arms in `allocate_texture_atlas` — there is no upstream software or
`PureCpu` arm for the missing ceiling to have come from.

**Why that matters.** The atlas side is chosen from attacker-controlled
content. `Atlas::allocate_with_padding` computes the next side as
`(self.side * 2).max(next_power_of_two(reserve_width.max(reserve_height)))`
(`atlas.rs:113-119`), where `reserve_width` is the *decoded pixel width of an
inline image*. A sixel or iTerm2 image wider than the current atlas forces a
doubling, and nothing bounds the doubling. Because the GL arm bails, the GPU
path falls into `AllowImage::Scale(2)` (`render/paint.rs:66-90`), halves the
sprite and retries; PureCpu never enters that fallback, so it simply
allocates.

**Failure scenario, measured.** `printf` a 17000x64 red-to-blue gradient as
sixel — the escape sequence is **1527 bytes** (long runs compress trivially
under sixel RLE):

```bash
convert -size 17000x64 gradient:red-blue png:- | convert - sixel:- | wc -c
# 1527
```

`reserve_width = 17002` -> `next_power_of_two` = 32768 -> atlas side 32768 ->
`32768 * 32768 * 4 = 4096 MiB`. Peak RSS of the two backends rendering
exactly this corpus (`tools/purecpu-parity/corpus/wide-sixel.sh`), sampled
twice a second for 15 s:

| backend | peak RSS | log |
|---|---|---|
| OpenGL — **not a control**, see below | 3274–3284 MiB | `Not enough texture space (Cannot use a texture of size 32768 as it is larger than the max 16384 supported by your GPU); will retry render with Scale(2)` |
| PureCpu | 4170 MiB | *(no texture-space fallback logged)* |

**The table is not the argument, and the OpenGL row is not a control.** That
row is llvmpipe's own working set around a *capped* 16384 atlas; it varies
run to run (3274 MiB and 3284 MiB in two runs here) and it is large for
reasons that have nothing to do with this defect. The argument is the pair of
asymmetries: GL **refuses** the 32768 allocation and says so in its log, five
to twenty-seven times per run, while PureCpu's log is empty; and PureCpu's
4170 MiB is 4096 MiB + process overhead almost exactly, i.e. the atlas is
really there. PureCpu's figure is stable — 4170 MiB in every run, here and on
the reviewer's independent reproduction.

The next doubling is the problem: a sixel roughly twice as wide selects side
65536 -> **16 GiB**. This machine has 25 GiB total. `Vec` allocation failure
in Rust aborts the process; before that, the OOM killer is the likely
outcome, and the victim is not necessarily wezterm. Amplification from the
1527-byte input to the measured 4 GiB allocation is about 2.8e6.

**Deliberately not measured.** I did not run the 65536 case. It would
allocate 16 GiB on a shared 25 GiB machine and could take other users' work
down with it; the arithmetic above is `next_power_of_two` and a multiply and
does not need a demonstration that risks the host. The 32768 case *was*
measured, and it is already past what the GPU path permits.

**Evidence class:** measured (4 GiB allocation, backend asymmetry, log
asymmetry); by reading for the 16 GiB extrapolation.

**Fix direction.** Give the `PureCpu` arm a ceiling and a `bail!`, so it
reaches the same `AllowImage::Scale` fallback the GL arm already reaches.

Reproduce. Save as a script and run it as `./c1-repro.sh PureCpu` and
`./c1-repro.sh OpenGL`, with `PARITY_XAUTH` exported (`lib.sh` fails closed
without it):

```bash
#!/usr/bin/env bash
set -euo pipefail
source /scratch/oetiker/wezterm/tools/purecpu-parity/lib.sh
FE="${1:-PureCpu}"
cls="par-mem-$(echo "$FE" | tr 'A-Z' 'a-z')"
kill_class "$cls"
"$PARITY_DIR/gen-config.sh" "$FE" "$PARITY_OUT/mem-$FE.lua"
launch "$cls" "$PARITY_OUT/mem-$FE.lua" "$PARITY_DIR/corpus/wide-sixel.sh"
find_window "$cls" >/dev/null
peak=0
for i in $(seq 40); do
  # Take the max over EVERY matching pid, not the first.  `pgrep -f
  # -- "--class $cls"` matches the setsid parent and the bash -c wrapper as
  # well as the gui process, and their order is not stable: `| head -1`
  # picks the parent often enough to print ~4 MiB for every sample and make
  # this finding look fabricated by three orders of magnitude (review round
  # 1, I-R1).  Filtering on /proc/$p/comm does not help either — comm is
  # truncated to 15 characters, so this binary reads `wezterm-gui-reb`, not
  # `wezterm-gui`.  The max is unambiguous: the gui process dwarfs both.
  for p in $(pgrep -u "$USER" -f -- "--class $cls" || true); do
    rss=$(awk '/VmRSS/{print $2}' "/proc/$p/status" 2>/dev/null || true)
    [ -n "${rss:-}" ] && [ "$rss" -gt "$peak" ] && peak=$rss
  done
  pause 0.5
done
echo "$FE peak RSS: $peak kB = $((peak/1024)) MiB"
echo -n "  'Not enough texture space' lines in log: "
grep -c "Not enough texture space" "$PARITY_OUT/$cls.log" || true
kill_class "$cls"
```

Verified output, both arms, from a clean shell:

```
PureCpu peak RSS: 4270776 kB = 4170 MiB
  'Not enough texture space' lines in log: 0
OpenGL  peak RSS: 3363024 kB = 3284 MiB
  'Not enough texture space' lines in log: 27
```

---

## Important

### I1 — Dirty-rect tracking only ever consults the *active* pane, so output in any other pane of a split is not repainted until something else forces a full repaint

**Disposition: fixed in `bc05562`** (with I2 — dirty rects are computed for every pane at its own origin).

`wezterm-gui/src/termwindow/mod.rs:1275-1330`, early exit at
`mod.rs:1436-1447`

**Defect.** `do_paint_purecpu` derives its entire dirty set from
`self.get_active_pane_or_overlay()`. No other pane in the tab is consulted.
When a background pane writes to its terminal, the mux invalidates the
window, `do_paint_purecpu` runs, finds no changed rows in the *active* pane
and no cursor movement, hits `state.dirty_pixel_rects.is_empty()` and returns
before `paint_impl` — so the background pane's new output is not drawn.

**Not "never", precisely.** The pane catches up at the next *full* repaint,
and `mod.rs:1226-1252` lists several ordinary triggers for one: a config,
shape or quad-generation change (which includes focus change, `mod.rs:532`),
a viewport scroll, or a selection change. So the pane is stale for as long as
nobody touches the window, not forever — which for a background build is
still the whole time you are watching it.

**Failure scenario, measured.** Two panes side by side. The **left**
(inactive) pane runs a script that toggles a 10-cell colour bar at its home
position twice a second; the **right** (active) pane is idle. One pixel
inside the left pane's bar, sampled 10 times at 0.4 s intervals (4 s total,
about 8 toggles):

```
gl  : red green green red green red green red green red      (alternating)
cpu : red green green green green green green green green green
```

PureCpu changes once and is then **flat for the remaining 3.6 s**, while the
pane behind it keeps toggling. (I read the single early transition as the
tail of the initial full repaint, but that is *inference* — I did not
establish it, and the independent reviewer saw the same shape without
establishing it either. Nothing in the finding depends on it.)
Screenshot `tools/purecpu-parity/out/splitright-cpu.png` shows the left
pane's bar frozen mid-cycle.

This is not a corner case: a build running in one pane while you read in
another is the ordinary reason to use splits.

**Evidence class:** measured.

**Fix direction.** Walk the same pane list `paint_impl` renders
(`get_panes_to_render()`), not just the active pane.

Reproduce: driver listed under I2.

### I2 — Dirty-rect geometry ignores the pane's position in the split grid, so even the *active* pane is not repainted when it is not at the window's top-left

**Disposition: fixed in `bc05562`** (with I1).

`wezterm-gui/src/termwindow/mod.rs:1305` (`content_top`, the row origin) and
`mod.rs:1321` (row -> y),
`mod.rs:1345-1371` and `mod.rs:1396-1408` (cursor cell -> x, y)

**Defect.** The dirty rectangle for viewport row *r* is placed at
`content_top + r * cell_h`, and the cursor cell at
`padding_left + border.left + col * cell_w`. Both are *window*-relative. The
paint pass places the same content at
`top_pixel_y + (line_idx + pos.top) * cell_height` and
`left_pixel_x`, which is itself `padding_left + border.left + pos.left *
cell_width` (`render/pane.rs:340-342`, `437-440`) — note the `pos.left` term
lives *inside* `left_pixel_x`, it is not added on top of it.
The `pos.top` / `pos.left` terms — the pane's origin in the split grid — are
missing from the dirty-rect computation. For a single-pane window both are
zero and nothing goes wrong, which is why this survives every non-split test
in this review.

**Failure scenario A (y axis), measured.** A top/bottom split; the **bottom**
pane is the active one and runs the same twice-a-second colour toggle.
Because its `pos.top` is 15, every dirty band it produces lands 15 rows too
high — inside the *top* pane. The band is duly cleared and redrawn with the
top pane's (unchanged) content, so nothing visibly happens anywhere. Sampling
one pixel of the bottom pane's bar, 10 samples at 0.4 s:

```
gl  : green green red green red green red green red red
cpu : red green green green green green green green green green
```

Screenshot: `tools/purecpu-parity/out/splitbot-cpu.png` — the bottom pane's
bar is frozen although it is the pane with keyboard focus.

**Failure scenario B (x axis), measured.** A left/right split; the **right**
pane is active (`pos.left = 50`) and runs a script that only ever moves the
cursor between column 1 and column 16 of one line — no cell content changes,
so no line seqno changes and the *only* dirty rects produced are the two
cursor-cell rects. Those are computed at `col * cell_w` with no `pos.left`
term, i.e. in the **left** pane. Sampling the right pane's column-1 cell, 10
samples at 0.4 s (gray levels; 156 = cursor present, 59 = cursor absent):

```
gl  : 156 156 59 156 59 156 59 156 59 59
cpu : 58 156 156 156 156 156 156 156 156 156
```

The cursor stops moving on screen. This is the everyday case of editing a
command line in the right-hand pane of a split.

**Evidence class:** measured, both axes.

**Fix direction.** Add the active pane's `pos.top` / `pos.left` (in cells)
before converting to pixels — the same offsets `render/pane.rs` already
applies.

Reproduce (one driver, three parameterisations; used for I1 and both I2
scenarios). Corpora, written outside the tree:

```bash
# clock.sh      loop { printf '\033[H\033[41m          \033[0m'; sleep 0.5
#                      printf '\033[H\033[42m          \033[0m'; sleep 0.5 }
# idle.sh       printf 'IDLE PANE\n'; exec sleep 600
# static.sh     printf 'STATIC PANE ####################\n'; exec sleep 600
# cursormove.sh printf 'ABCDEFGHIJKLMNOP'
#               loop { printf '\033[1G'; sleep .5; printf '\033[16G'; sleep .5 }
```

Config (per front end), whose `gui-startup` handler spawns pane 1 running
`$FIRST.sh` and splits `$DIR` running `$SECOND.sh`; the split pane is the
active one:

```lua
wezterm.on('gui-startup', function(cmd)
  local tab, pane, window = wezterm.mux.spawn_window { args = { FIRST } }
  pane:split { direction = DIR, size = 0.5, args = { SECOND } }
end)
```

The rest of the config is `gen-config.sh`'s defaults (100x30, font_size 12,
no padding, pinned colours, blink off). Then, per backend: `launch` ->
`find_window` -> `check_no_config_error` -> `pause 4` -> `import` a
screenshot -> `sample_region "$W" "$GEOM" 10 0.4` (both helpers from
`tools/purecpu-parity/lib.sh`).

| case | DIR | FIRST | SECOND | GEOM |
|---|---|---|---|---|
| I1 | `Right` | clock | idle | `4x4+4+40` |
| I2-A | `Bottom` | idle | clock | `4x4+4+380` |
| I2-B | `Right` | static | cursormove | `2x8+503+40` |

Note `direction = 'Down'` is not a valid wezterm split direction — it makes
the handler error out after `spawn_window`, leaving a single pane and a
probe that silently tests nothing. Use `'Bottom'`.

### I3 — The idle skip suppresses every time-driven repaint, and nothing else marks that content dirty

**Disposition: fixed in `230117c`.** Blink, bell and animated images now have their own dirty-rect producers, so the idle skip no longer suppresses them. Re-measured in Task 14: the animated-GIF, blinking-text and visual-bell rows of `parity-matrix.md` all moved from `degraded`/`missing` to `parity`, each graded against the pre-fix binary as a control in the same session.

`wezterm-gui/src/termwindow/mod.rs:1436-1447`; bell handler at
`mod.rs:1574-1594`

**Defect.** `do_paint_purecpu` returns before `paint_impl` whenever
`force_full_repaint` is false and `dirty_pixel_rects` is empty. The dirty set
is built only from (a) rows whose line seqno changed, (b) cursor movement,
and (c) a quantised cursor-blink phase transition. Nothing marks cells
carrying the SGR 5 blink attribute; nothing marks the visual bell — the
`Alert::Bell` handler calls `window.invalidate()` but pushes no `DirtyRect`;
and nothing marks animated-image frame advance, which in the GPU path happens
as a side effect of running the paint pass (`glyphcache.rs:929-970`). All
three therefore stop at the first still frame.

**Failure scenario, measured (Tasks 4 and 6).** One pixel, 16 samples at
0.4 s:

- SGR 5 blinking text: `gl` sweeps 25-191; `cpu` is `gray(16)` — the
  background colour — for the whole window, i.e. the word is frozen
  *invisible*, and deterministically so: `ColorEase::intensity_continuous()`
  starts near 0 and `render/screen_line.rs:810-821` sets `fg = bg` at
  intensity 0.
- Visual bell, rung once per second: `gl` flashes and fades; `cpu` is flat
  `gray(16)`.
- Animated GIF: `gl` cycles yellow/green; `cpu` reports one frame for all 12
  samples.

The control experiment is decisive: setting `purecpu_force_full_repaint =
true` (`config/src/config.rs:708-714`) restores all three at the GPU path's
own levels, which isolates the idle skip as the cause rather than the
rasteriser.

**Evidence class:** measured (full sequences and regeneration commands in
`parity-matrix.md`, rows "Blinking text attribute", "Visual bell", "Inline
image: animated GIF").

**Fix direction.** Either give blink/bell/animation their own dirty-rect
producers, or consult `has_animation` before taking the early exit.

### I4 — Cursor-blink detection tests the *raw* pane cursor shape, so the documented way to enable blinking is inert

**Disposition: fixed in `868159c`.** The shape is resolved through `effective_shape` before `is_blinking()` is tested. Re-measured in Task 14: config-driven blink now runs, and the row moved `missing` -> `degraded` — PureCpu blinks as a two-level square wave (`gray(161)` / `gray(224)`) where OpenGL eases, so it is no longer absent but not yet equivalent.

`wezterm-gui/src/termwindow/mod.rs:1383`

**Defect.** `cursor.shape.is_blinking()` is evaluated on the shape returned
by `pane.get_cursor_position()`, which is `CursorShape::Default` unless the
running program issues DECSCUSR. `CursorShape::Default.is_blinking()` is
`false` (`wezterm-surface/src/lib.rs:79-85`), so `cursor_blinking` is always
false for a config-driven setup and the whole blink block
(`mod.rs:1380-1412`) never fires. The GPU path resolves the shape first,
through `params.config.default_cursor_style.effective_shape(cursor.shape)`
(`render/mod.rs:604-611`, `config/src/config.rs:1921-1933`) — which is
exactly what turns `Default` into `BlinkingBlock`.

**Failure scenario, measured (Task 6).** With
`default_cursor_style = 'BlinkingBlock'`, `cursor_blink_rate = 600`,
`animation_fps = 30`, one pixel inside the cursor cell sampled 16 times over
6.4 s: `gl` varies continuously across 17-222; `cpu` is `gray(224)` for all
16 samples. Sending DECSCUSR (`\033[1 q`) directly, so the raw shape *is*
`BlinkingBlock`, makes PureCpu blink — confirming the gap is specifically the
missing `default_cursor_style` resolution and not a blanket inability to
blink.

**Evidence class:** measured.

**Fix direction.** Resolve through `effective_shape` before calling
`is_blinking()`, as `render/mod.rs:604-611` does.

### I5 — Textured quads are blitted 1:1 and cropped; the rasteriser contains no resampler

**Disposition: fixed in `773b845`** (with M1). The rasteriser gained a nearest-neighbour source step, which is the fix direction stated below. Re-measured in Task 14: every failure scenario listed here that was measurable now reports bit-identical — the non-native iTerm2 image (PAE 61423 -> 0), the 300x300 sixel (PAE 1542 -> 0) and DECDWL/DECDHL (body PAE 45232 -> 257, with the previously-blank DECDHL bottom band going from 0 ink pixels to 1820 against OpenGL's 1824). The `AllowImage::Scale` scenario is **not** closed by this commit and never was its target; it is the atlas-cap asymmetry, now its own matrix row. `IS_BG_IMAGE` was deliberately left on the old crop path (out of scope).

`wezterm-gui/src/termwindow/render/purecpu.rs:344-360`

**Defect.** `blit_w = tex_w.min(dest_w)`, `blit_h = tex_h.min(dest_h)`, with
the atlas source stepped in lockstep with the destination
(`atlas_row = tex_px_y + row`, `atlas_col = tex_px_x + col`). There is no
scaling anywhere in `call_draw_purecpu`. The GPU path interpolates texture
coordinates across the quad and therefore rescales whenever source and
destination sizes differ. Every content class that relies on that rescale is
drawn at the wrong size, cropped to the smaller of the two rectangles.

**Failure scenarios.** All reachable from an ordinary escape sequence or an
ordinary config:

- **Inline image at a non-native size** (measured): an iTerm2 OSC 1337 image
  requested at 200x132 px when its sprite is 64x64. OpenGL draws a stretched
  gradient block; PureCpu draws a dotted grid of tiny cropped tiles, one
  top-left fragment per covered cell, background showing through the rest.
  AE = 39360, PAE = 61423 (0.937 of full scale).
- **`AllowImage::Scale`-downscaled sprites** (measured): every pixel of the
  visible strip differs.
- **DECDWL / DECDHL double-width and double-height lines** (by reading):
  `render/screen_line.rs:636-647` scales the destination by
  `width_scale` / `height_scale` while the atlas source stays at base size,
  so PureCpu draws the base-size glyph in the top-left corner of the enlarged
  cell. Reachable from `printf '\033#6'`. **Not reproduced with a capture** —
  missing evidence is a DECDWL corpus run through `compare-case.sh`.
- **Scaled fallback / bitmap glyphs** (`glyph.scale != 1`, by reading):
  colour-emoji fonts set `scale` below 1 (`glyphcache.rs:749-798`); the
  sprite is cropped rather than downscaled.
- **Fancy tab bar glyph runs with `glyph.scale != 1`** (by reading,
  `termwindow/box_model.rs:902-913`).

**Evidence class:** measured for images; by reading for DECDWL/DECDHL and
scaled glyphs.

**Fix direction.** A nearest-neighbour source step
(`src_col = tex_px_x + col * tex_w / dest_w`) would match the GPU's nearest
sampler for every non-background case at modest cost.

### N2 — Glyph ink that escapes its cell is left stale, because dirty rects are cell geometry and glyphs are not clipped to cells

`wezterm-gui/src/termwindow/render/screen_line.rs` (the glyph arm — the
`// TODO: clipping, but we can do that based on pixels` is not an oversight to
be fixed elsewhere, it is *why* ink leaves the cell),
`wezterm-gui/src/termwindow/render/purecpu.rs` (`collect_quad_dest_rects`,
`call_draw_purecpu`)

**Found during the fix pass, not by the review** (hence the `N` prefix, shared
with N1). **Fixed in `fbbb433`, narrowed in `6311e97`.** It is recorded here
because it is the **one defect in this pass that was reproduced in pixels rather
than reasoned about**, and because the fix leaves a scoped limitation that is
still open.

**Defect.** Every dirty rect PureCpu produces is *cell* geometry — a row band, a
cursor cell, a blinking cell. A glyph quad is placed at the rasterized sprite's
own width and height with the glyph's bearings applied, and **nothing clips it
to the cell**: not `screen_line.rs`, and not the blit, which clamps to the
framebuffer rather than to the cell. So when a cell is marked dirty and
repainted, the part of its glyph's ink that lies *outside* the cell is neither
cleared nor redrawn. It keeps whatever was last presented there — frozen ink
sitting next to an animating glyph.

**Two things the fix pass had to correct about where the ink goes**, both worth
keeping because they are the kind of thing that gets assumed:

- **Box-drawing and block characters overhang by exactly zero.**
  `customglyph.rs::block_sprite` allocates an image of exactly `cell_size` and
  `screen_line.rs` forces `top = 0.` for a block key, so with the default
  `custom_block_glyphs = true` they fill their cell precisely.
- **Horizontal overhang is real** and is present in this tree's default
  configuration: `ls-fonts` reports the Noto Color Emoji fallback rasterized to
  a **21 px** sprite in a **19.19 px** two-cell span — ~1.8 px of ink to the
  right. Negative `bearing_x` (the `shapecache.rs` fixtures carry `-15.0`,
  `-18.0`) puts ink to the left.

**Why a demonstration needed an unusual config, and why that is not a cheat.**
Measured with `wezterm-gui ls-fonts --rasterize-ascii` against the harness
config (JetBrains Mono, `font_size = 12`, 96 dpi, cell ~9.59 x 22 px), **no
ASCII glyph in this font overhangs vertically at `line_height = 1.0`** — the
tallest, `[`/`{`, is a 17 px sprite at `bearing_y = 14` in a 22 px cell. That is
why `corpus/blink-text.sh`, which blinks the all-capitals word "BLINKING",
**cannot** show this defect, and why a run against it would have reported "no
overhang" for entirely the wrong reason.

**Failure scenario, measured in pixels.** `corpus/blink-descender.sh` (new,
committed) blinks `gggjjjyyyqqqppp`, with the blink-text config plus
**`line_height = 0.75`** — which shrinks the cell *without* rescaling the glyph,
so the descenders hang about 1 px below it. 12 captures 0.25 s apart per binary:

| binary | row | non-background px | distinct states across frames |
|---|---|---|---|
| pre-fix | 79 (inside the cell) | 39 | 6 |
| pre-fix | **81 (below the cell)** | **54** | **1** |
| fixed | 79 | 39 | 3 |
| fixed | **81** | **57** | **3** |

Row 81 in the pre-fix binary holds 54 pixels of ink and is **bit-identical in
every one of the 12 frames** while the glyph body two rows above it blinks
through six distinct states. That is frozen descender ink, seen directly. In the
fixed binary the same row animates.

**Fix.** Not the cell-margin widening first proposed: no bound is derivable at
the dirty walk, because `AnimatedCellScan` has `Line`s and `attrs()` but no
shaping, no cluster, no `CachedGlyph` and no sprite sizes — reaching a glyph
extent from there means re-running the shaper and glyph cache for every animated
cell on every frame, which *is* the paint pass. Any margin chosen there would be
a guess. Instead `call_draw_purecpu` collects the frame's **quad destination
rects** — the exact extent that already exists one stage later — and grows the
dirty set to the quads that overlap it, before clearing and blitting. Both the
current frame's and the previous frame's quads are used, because ink that was
there last frame and is gone this frame must also be erased. Growth is bounded
by where quads actually paint (~1 px here), not by a margin, which is why it
does not walk the blink case toward the 37.6% full-repaint ceiling.

**The limitation that remains, and it is not hypothetical.** Only **vertex
buffer 1** is collected, because that is where glyphs go; backgrounds, selection
and the cursor sit on buffers 0 and 2 at cell geometry, and growing a rect to
*their* extent would grow it to the pane-sized background quad and turn every
incremental frame into a full repaint. **Animated images are on buffer 0**
(`screen_line.rs`, `populate_image_quad`) and are additionally shifted by
`padding_left`/`padding_top`, so **an animated image's ink that escapes its cell
is still not repainted**. Four of the five dirty-rect producers are covered —
both cursor-movement paths, the bell's `CursorCell`, blinking text, and the row
bands — and the fifth, animated images, is not. The limitation is stated in
`collect_quad_dest_rects`'s own doc comment, at the place where the decision to
exclude buffer 0 is made, so it cannot drift away from its cause. Closing it
means admitting buffer 0, which cannot be done wholesale.

`6311e97` narrowed the fix after review: buffer 1 turned out to be a necessary
but not sufficient filter — the window-border strips are on it too — so only
*sampled* quads may grow a rect.

**Evidence class:** **measured, in pixels**, with a pre-fix control captured in
the same experiment.

---

## Medium

**Ordering note (Task 8, Mi1).** Findings are presented in id order within each
class, *not* most-severe-first, and in this class the two orderings differ:
**M3, M4 and M5 are defect-class Critical** (process aborts reachable from font
data) recorded Medium under the demotion rule, while M1 and M2 ahead of them are
cosmetic rendering errors. If you are reading this section to decide what to fix
first, read M3/M4/M5 first. The ids are left as they are because Task 7's review
verified the numbering is contiguous and every internal cross-reference resolves
against it; renumbering to fix a presentation order would put that at risk for
no gain. The README's ordering reflects severity, not id.

### M1 — Destination coordinates are truncated rather than rounded, displacing sub-pixel-positioned quads one pixel left/up

**Disposition: fixed in `773b845`** (with I5) — destination coordinates are rounded rather than truncated. Re-measured in Task 14 with the same shift test that found it: over the tab-strip crop that carried the worst of it, the *unshifted* alignment now wins (`dx=0` AE=172 against `dx=1` AE=1023), where before `dx=1` was the bit-exact match. **The second, explicitly-unverified class in this finding — the 99 columns of ~77/255 non-integer divergence — went away with it**, which is consistent with the nearest-sampling-at-a-fractional-offset explanation this finding declined to assert, but does not prove it: the two changes landed in one commit and were not separated.

`wezterm-gui/src/termwindow/render/purecpu.rs:257-264`

**Defect.**

```rust
let dest_x = (tl.position[0] + half_w) as i32;
```

`as i32` truncates toward zero. The GPU rasterises by pixel centre: pixel *i*
is covered when *i* + 0.5 falls inside the quad, so the first covered pixel
is `round(x0)`, not `floor(x0)`. For any quad whose destination edge is not
integral, PureCpu therefore starts up to one pixel earlier than the GPU, and
`dest_w = floor(x1) - floor(x0)` can differ from `round(x1) - round(x0)` by
one, which then feeds `blit_w = tex_w.min(dest_w)` and drops a column.

**Failure scenario.** Terminal-body cells sit on an integral grid
(`cell_size.width` is an integer, `padding_left` is 0 in the harness), which
is why the body measures at the 1-LSB noise floor. The fancy tab bar does
not: `box_model` computes element positions in floats. Task 5 measured, on
the tab strip of `out/chrome-{gl,cpu}.png`, that **37 columns of the strip
are a bit-exact one-pixel-left shift** — comparing GPU column *x* against
PureCpu column *x-1* gives `AE = 0` — including the active tab's title
letters (single-row crops `50x1+20+12` and `50x1+20+14`). The retro tab bar,
which draws titles as ordinary monospace cells on the integral grid,
reproduces none of it.

Task 5 also found 99 strip columns that are *neither* identical nor a clean
shift, with peak error 77/255. Truncation alone does not explain those; a
nearest-sampled quad at a fractional offset duplicates or drops source
columns in a way that is not a rigid translation, which is consistent with
what was seen, but I did not isolate it. **Explicitly unverified for that
second class** — missing evidence is a per-glyph trace of the emitted quad
positions for the inactive tabs.

**Evidence class:** measured (the 1 px shift); by reading (the mechanism);
**unverified** for the residual 77/255 class.

**Fix direction.** Round: `(tl.position[0] + half_w).round() as i32`.

### M2 — Overlapping dirty rects composite the same pixel more than once

**Disposition: closed by re-measurement, with no code change** (Task 11). The plan forbade fixing M2 before re-measuring, and the re-measurement found nothing left to fix. **The control is what makes the negative mean anything**: in one experiment, identical corpus, config and display, the pre-fix binary reproduced this finding's signature *to the digit* — `n=87, max|resid|=1.91, mean=-0.05`, with the same cell-0-and-nowhere-else per-column shape — while the subject returned **`n=0`**, not one label-row pixel above the 1 LSB floor. So the instrument was proven on the box that day and the defect was gone from the subject. Body PAE fell 13107 -> 1285. **The 1285 residue is not M2**: it is the PureCpu/OpenGL atlas-cap asymmetry, which now has its own matrix row. Which commit removed the double composite was **not** determined and was not guessed; the experiment that would settle it (build at `b7936ac` and re-run the same control/subject pair) is named in the ledger rather than dropped.

`wezterm-gui/src/termwindow/render/purecpu.rs:77-99` (`collect_clip_rects`),
`purecpu.rs:353-475` (blit loop)

**Defect by construction.** `collect_clip_rects` pushes **one clip rect per
overlapping dirty rect**, and the blit loop runs the whole blend once per
clip rect. Two dirty rects that overlap therefore alpha-composite every quad
in their intersection twice. `do_paint_purecpu` can produce overlapping
rects: the full-width row band (`mod.rs:1316-1329`) covers the same pixels as
the per-cursor-cell rects (`mod.rs:1338-1371`) and the blink-transition rect
(`mod.rs:1396-1408`) whenever they refer to the same row. `clear_rect` runs
once per rect too, but clearing twice is idempotent, so it does not cancel
the double blend.

That is a statement about what the code *can* do, and it is read off the
source. It is **not** the explanation of the instance measured below: two
deliberate attempts to make those particular rects overlap produced no double
composite at all (see "What is not established"). Either those frames did not
take the incremental path, or overlapping row/cursor rects do not in fact
reach the blit loop together. Both halves of this finding are real; they are
simply not yet joined.

**Observed instance, measured.** In the wide-sixel capture pair
(`tools/purecpu-parity/out/wide-sixel-{gl,cpu}.png`) the label row
"WIDE SIXEL:" (ink occupies y 38–49) differs between backends in **cell 0
and nowhere else**. `gen-config.sh:166` sets `initial_cols = 100` with zero
`window_padding` (`:185`) and the capture is 1000 px wide, so a cell is
exactly **10 px**. Per-column max difference across the label row, in LSB:

```
x      0  1  2  3  4  5  6  7  8  9 | 10 11 12 13 14 15 16 17 18 19
diff  44 43 43 43 43 44 43 45 44 31 |  0  1  1  1  1  1  1  0  0  0
```

The bar marks the cell boundary: x 0-9 is cell 0, x 10-19 is cell 1. Cell 1
and every cell to its right sit at the 1 LSB noise floor.

Re-binned per 10 px cell: cell 0 = 45 LSB, cells 1–5 = 0 or 1 LSB. **An
earlier draft of this finding reported "the first two character cells". That
was wrong — an artefact of binning the PAE in 8 px blocks that are not
cell-aligned, so the second bin's entire signal was x = 8, 9, still cell 0
(review round 1, C-R1).**

The differences are confined to antialiased pixels: fully covered pixels are
192 on both, background is 16 on both. The PureCpu values are *exactly* what
you get by compositing the glyph a second time over OpenGL's own result. With
foreground 192 and background 16, the coverage implied by a GPU pixel is
`a = (gl - 16) / 176`, and the prediction is `192*a + gl*(1-a)`. Evaluated
over **all 87 label-row pixels that differ by more than 1 LSB** — not a
hand-picked sample:

| statistic | value |
|---|---|
| pixels differing by > 1 LSB | 87 |
| max abs residual vs. prediction | **1.91 LSB** |
| mean residual | **−0.05 LSB** |

A mean residual of −0.05 across 87 pixels, with no residual above 2 LSB, is
not a coincidence: PureCpu composited those glyph pixels twice. The
signature is also **stable** — three independent re-captures are
byte-identical to each other and to the stored PNG over the label row.

Recompute:

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity/out
for s in gl cpu; do
  convert wide-sixel-$s.png -crop 30x12+0+38 +repage -colorspace Gray -depth 8 txt: \
    | tail -n +2 | sed 's/:.*gray(/ /;s/)//' > /tmp/$s.txt
done
paste /tmp/gl.txt /tmp/cpu.txt | awk '
{ gl=$2; cpu=$4; d=cpu-gl; if (d<0) d=-d;
  if (d>1) { n++; a=(gl-16)/176; r=cpu-(192*a+gl*(1-a));
             s+=r; ar=(r<0?-r:r); if (ar>mx) mx=ar } }
END { printf "n=%d max|resid|=%.2f mean=%+.2f\n", n, mx, s/n }'
# n=87 max|resid|=1.91 mean=-0.05
```

**What is not established.** A deliberate reproduction — print a row, let it
settle, then append one glyph so that the row band *and* both cursor-cell
rects fire in the same incremental frame — did **not** reproduce it: body PAE
stayed at 257. Command:

```bash
# corpus: printf 'WWWWWWWWWWWWWWWWWWWW'; sleep 6; printf 'W'; exec sleep 600
cd /scratch/oetiker/wezterm/tools/purecpu-parity
PARITY_SETTLE_WARMUP=9 FUZZ=1 ./compare-case.sh dblblend <corpus>
# raw AE 1007 / fuzz-1% AE 157 / body PAE 257  -> no double composite
```

A second, tighter attempt was then built against the corrected one-cell
geometry (review round 1, I-R3): settle a blank window so the initial full
repaint completes, then emit one line from home, so that in the *same*
incremental frame the row-0 band and the previous-cursor cell rect at (0, 0)
fire and overlap in cell 0 alone — precisely the frame the named mechanism
predicts.

```bash
# corpus: sleep 5; printf 'ABCDEFGHIJ\n'; exec sleep 600
# per backend: launch -> find_window -> check_no_config_error
#              -> PARITY_SETTLE_WARMUP=8 capture_settled
# label row per-cell max diff: 1 1 1 1 1 1 1 1 1 1 0 0  -> no double composite
```

Zero pixels above the noise floor, cell 0 included. **The row-band /
cursor-rect route has therefore been tested and eliminated, across two
independent deliberate attempts, the second matched to the corrected
geometry.** It is not merely unproven — it is the one candidate I could name,
and it is out.

So: **the double composite is proven; the rect pair that produced it is
unknown, and the only candidate that was namable has been eliminated.** The
missing evidence is now specific: **an instrumented dump of
`state.dirty_pixel_rects` at the frame that produced the affected capture.**
Nothing short of that will settle it — the by-construction mechanism in
`collect_clip_rects` is real, but so is the possibility that this instance
came from somewhere else entirely (M7's uncleared-row branch is one such
place).

This finding does close out the "unexplained wide-sixel label-row
antialiasing anomaly" carried forward from Task 4 fix rounds 2 and 3: the
*what* is now known exactly and to 1.91 LSB across 87 pixels (a second alpha
composite of the same glyph), the *which rects* is not.

**Evidence class:** measured (the double composite, 87 pixels); by reading
(the mechanism); the link between the two **refuted for the only named
candidate, otherwise unknown**.

**Fix direction.** Reduce the dirty rects to a disjoint set before blitting,
or clip each quad against the union rather than blending once per rect.

### M3 — Unconditional panic in the empty-path fallback of the COLR glyph rasteriser

**Disposition: panic site guarded in `b10c3a5`; trigger never reproduced.** Deliberately not called fixed. No font reaching this path was ever built — the tooling gap this finding records (`fontTools` unavailable) was never closed — so nothing demonstrates a trigger now passing that used to abort. The guard is what changed; the evidence class below has not.

`wezterm-font/src/rasterizer/paint_ops.rs:329-334`

**Defect class Critical, recorded Medium** under the demotion rule above: a
process abort reachable from font data, whose trigger is not reproduced. It
sits here with M4 and M5, the other two font-stack panics, because their
reachability arguments are of the same kind and none of the three was run.

**Defect.**

```rust
pb.finish().unwrap_or_else(|| {
    // Return an empty path as fallback
    let mut pb2 = tiny_skia::PathBuilder::new();
    pb2.move_to(0.0, 0.0);
    pb2.finish().unwrap()     // <- always None, always panics
})
```

The fallback that exists to handle "this path could not be finished" is
itself unfinishable. `tiny_skia::PathBuilder::finish` returns `None` when the
builder is empty **and** when it holds exactly one verb
(`tiny-skia-path-0.11.4/src/path_builder.rs:411-419`:
`if self.verbs.len() == 1 { return None; }`). A single `move_to` is exactly
one verb. So whenever the `unwrap_or_else` arm is entered at all, the process
panics.

Verified against the pinned dependency (`Cargo.lock`: `tiny-skia 0.11.4`)
with a standalone program that calls the same two APIs:

```
empty builder finish()  -> None
move_to-only finish()   -> None
```

**Reachability.** `draw_ops_to_path` is called from `skia_colr.rs:137` for
`PaintOp::PushClip(draw_ops)`, and those draw ops come from
`PaintOpCollector::push_clip_glyph` (`skrifa_rasterizer.rs:551-554` ->
`glyph_outline_draw_ops`, `skrifa_rasterizer.rs:453-464`).
`glyph_outline_draw_ops` returns an **empty `Vec`** whenever
`outlines.get(glyph_id)` yields `None`, and also whenever the referenced
glyph has zero contours. A COLRv1 glyph whose clip layer references a blank
glyph (a space, or a glyph id with no outline entry) therefore produces
`PushClip(vec![])` -> empty `PathBuilder` -> `finish()` `None` -> the
fallback -> panic. `finish()` also returns `None` for non-finite coordinates
(`Rect::from_points` fails), which the `unitsPerEm = 0` case in M5 produces.

**What is proven and what is not.** *Proven:* the fallback panics
unconditionally when reached, and the shape of input that reaches it is a
`PushClip` with an empty draw-op list. *Not demonstrated:* an actual font file
that produces such a layer. `fontTools` is not installed on this machine and
no COLRv1 font present here exercises the path, so I could neither craft nor
find one. **Missing evidence: a COLRv1 font whose clip layer references an
outline-less glyph, rendered through this binary.** Treat the reachability as
by-reading and the panic itself as verified.

**Evidence class:** by reading (reachability) + measured (the tiny-skia
half). Trigger **not reproduced**.

**Fix direction.** Skip the op when the path cannot be finished, rather than
building a degenerate path and unwrapping it.

### M4 — `cpal.color_record_indices()[0]` panics on a CPAL table with zero palettes

**Disposition: panic site guarded in `b10c3a5`; trigger never reproduced.** Deliberately not called fixed. No font reaching this path was ever built — the tooling gap this finding records (`fontTools` unavailable) was never closed — so nothing demonstrates a trigger now passing that used to abort. The guard is what changed; the evidence class below has not.

`wezterm-font/src/rasterizer/skrifa_rasterizer.rs:370`

**Defect class Critical, recorded Medium** under the demotion rule above:
process abort from font data, trigger not reproduced.

**Defect.** `let first_color_index = cpal.color_record_indices()[0].get() as usize;`
indexes an array whose length is the table's `numPalettes`. Every other
fallible step in `render_colr` uses `?` or `ok()?`; this one does not.
`read_fonts` returns the slice as parsed, so a font whose CPAL declares
`numPalettes = 0` yields an empty slice and the index panics. The enclosing
function already returns `Option`, so the fix is `.get(0)?`.

**Reachability.** Requires a font with a COLR table (so
`color_glyphs().get(glyph_id)` succeeds) and a malformed CPAL. Fonts are
attacker-controlled input in the sense that matters here — a user installs or
downloads one — but this is not reachable from terminal output.

**Not reproduced.** Missing evidence is a crafted font with
`numPalettes = 0`; `fontTools` is not available on this machine.

**Evidence class:** by reading. Trigger **not reproduced**.

### M5 — `unitsPerEm = 0` divides by zero in both the shaper and the rasteriser

**Disposition: panic site guarded in `b10c3a5`; trigger never reproduced.** Deliberately not called fixed. No font reaching this path was ever built — the tooling gap this finding records (`fontTools` unavailable) was never closed — so nothing demonstrates a trigger now passing that used to abort. The guard is what changed; the evidence class below has not.

**Defect class Critical, recorded Medium** under the demotion rule above,
trigger not reproduced. (The original text justified the Critical defect
class by saying M5 "reaches M3's abort". **That is wrong — see the
correction below.** The class stands on its own footing: an `inf` scale
breaks layout wholesale and saturates every advance to `i32::MAX`.)

`wezterm-font/src/shaper/harfrust_shaper.rs:242`,
`wezterm-font/src/rasterizer/skrifa_rasterizer.rs:383-384`,
`harfrust_shaper.rs:633-634`, `harfrust_shaper.rs:738-739`

**Defect.** `pixel_size / units_per_em` with no zero check. `unitsPerEm` is a
`u16` read straight from the `head` table. At zero the scale becomes `inf`;
advances then go through `(pos.x_advance as f64 * scale_26_6) as i32`
(`harfrust_shaper.rs:307-310`), where an infinite float saturates to
`i32::MAX` in Rust — no panic, but glyph advances of two billion pixels and
completely broken layout.

**Correction (Task 12): M5's infinite scale does not reach M3's panic. M3
and M5 are independent defects.** The original text claimed the infinite
scale propagates into COLR path coordinates, so `Rect::from_points` returns
`None` and M3's unconditional panic fires. It does not.
`glyph_outline_draw_ops` draws with `Size::unscaled()`, so the points the
pen collects are in font units and the scale never enters them; the scale is
carried separately in a `tiny_skia::Transform` applied at rasterisation
time. An `inf` in that transform produces a degenerate or empty raster, not
an out-of-range point set, and M3's `Rect::from_points` sees the same
unscaled points it would have seen with a sane `unitsPerEm`. Neither finding
is a precondition of the other, and fixing one does not close the other.

The author was aware of the hazard elsewhere: `harfrust_shaper.rs:680` guards
the same division with `if ch > 0.0 && upem > 0.0`.

**Guarded, not reproduced.** M5's division sites were given zero checks in
commit `b10c3a5`; no font that reaches any of M3/M4/M5 was ever built, so
none of the three is "fixed" in the sense of a demonstrated trigger now
passing. The guard is what changed; the evidence class below has not.

**Not reproduced.** Missing evidence is a font with `head.unitsPerEm = 0`
(same tooling gap as M4).

**Evidence class:** by reading. Trigger **not reproduced**.

### M6 — A dirty band that extends past the bottom of the window is silently not presented

**Disposition: fixed in `f78b501`** (with M7) — out-of-range bands are clamped instead of dropped. Trigger still not reproduced: this finding never had one, and the fix removes the silent-skip branch rather than demonstrating it firing.

`wezterm-gui/src/termwindow/render/purecpu.rs:501-519`

**Defect.**

```rust
if y + h > screen_height as usize { continue; }
```

The band is skipped rather than clamped. The framebuffer for that band has
already been cleared and re-blitted, so the on-screen content and the
framebuffer diverge: the region keeps whatever was last presented until the
next full repaint. Clamping `h` to `screen_height - y` would present the
visible part.

**Not reproduced.** In the harness geometry, rows are sized so that
`content_top + rows * cell_h` fits inside `pixel_height`, and I did not find
an input that pushes a band past the bottom edge. **Missing evidence: a
window geometry, `window_padding` or `tab_bar_at_bottom` combination that
produces `y + h > screen_height`.** Listed because it is a silent-skip branch
in the presentation path, not because I saw it fire.

**Evidence class:** by reading. **Unverified.**

### M7 — `clear_rect` silently skips a row instead of clamping when the computed row end exceeds the framebuffer

**Disposition: fixed in `f78b501`** (with M6) — out-of-range clears are clamped, and the rect arithmetic was made saturating (`ed7ba9e`). Trigger still not reproduced, as above.

`wezterm-gui/src/termwindow/render/purecpu.rs:102-115`

**Defect.** The guard is `if row_end <= fb.len()`, so a row whose computed
end is out of range is left **uncleared** — and the blit loop, which has its
own per-pixel `dx`/`dy` bounds checks, will still composite into whatever
part of that row is in range. That combination (skip the clear, keep the
blend) is another route to the double composite of M2. Separately, `x1` is
computed as `(rect.x + rect.width).min(fb_w as i32) as usize`: for a rect
with a negative right edge this casts a negative `i32` to a huge `usize`, and
`(y * fb_w + x1) * 4` then overflows — a debug-build panic, a wrapped value
in release.

**Not reproduced.** `do_paint_purecpu` only ever pushes rects with
non-negative `x` and positive `width`, so I could not construct the negative
case from the current call sites; it is a robustness hole in a function that
accepts arbitrary rects, not a live bug. **Missing evidence: a caller that
produces a negative right edge.**

**Evidence class:** by reading. **Unverified.**

---

## Low

### L1 — Every presented band copies the whole band and creates/destroys an X GC

`window/src/os/x11/window.rs:2177-2237`

`present_software_frame_region` starts with `let pixels = pixels.to_vec();` —
a full heap copy of the band, needed because the closure is deferred through
`XConnection::with_window_inner` — and then issues `CreateGc` ... `PutImage`
... `FreeGc` per call. `call_draw_purecpu` calls it **once per coalesced
band**, so a frame with five dirty bands performs five copies and five GC
create/destroy pairs. For a full repaint at 1280x1024 that is a 5 MiB
copy on top of the `PutImage` itself. A cost, not a correctness bug, in a
renderer whose whole purpose is to be cheaper than the GPU path.

**Correction (Task 14): the original text said "five GC create/destroy *round
trips*". They are not round trips, and the word carried the whole argument.**
`CreateGc` and `FreeGc` are **no-reply** X requests. They are queued into the
same output buffer as the `PutImage` they bracket and flushed with it, so
nothing blocks waiting for a server response and there is no per-band latency
to save. The remaining cost is real but small: request encoding, server-side
resource creation, and the `to_vec` copy — which is a different and much
cheaper claim than the one this finding originally made.

**Built, measured null on both sides, and reverted (Task 13).** L1 was
implemented — a cached GC hoisted out of the per-band path — and then backed
out, because it bought nothing measurable. Client side: across repeated
alternating-order runs the arms are indistinguishable (`static-image`
5.10 / 5.00 without the cache against 5.10 / 5.13 with it; `blink-text` 6.30 /
6.37 against 6.33 / 6.60 — a spread smaller than the within-arm spread). The
first round's apparent "+0.66 pp, cache is worse" was load noise on a busy box
and did not survive a quieter replication. **Server side, which is where a GC
optimisation would have to show up if anywhere:** sampling the `:20` X server's
own `/proc/<pid>/stat` across the same runs gives `static-image` 7, 7 ticks
without the cache against 8, 7 with it — a one-tick (10 ms) spread over 1800
`CreateGc`/`FreeGc` pairs, bounding the saving at roughly 5.5 µs per pair and
itself dominated by noise — and `blink-text` mean 92 against 93, against a
within-arm spread of 8. **So the null bounds both the client and the server,
and L1 stays reverted.** This record is written here because it previously
existed only in a source comment and in untracked files.

**Evidence class:** by reading for the mechanism; **measured null** for the
optimisation, on both the client and the X server side.

### L2 — `HintingInstance` is constructed per glyph rasterisation

`wezterm-font/src/rasterizer/skrifa_rasterizer.rs:161-174`

When hinting is enabled, `HintingInstance::new(outlines, skrifa_size,
location, target)` is built inside `render_outline`, i.e. once per glyph
rather than once per (font, size) pair, which is what skrifa's own
documentation recommends. Affects first-render latency only, since rasterised
glyphs land in the atlas cache.

**Disposition: fixed in `bce58ea`.** A `hinting_cache` keyed by the pixel
size's bit pattern now holds one `Arc<HintingInstance>` per size per
rasterizer. Two tests guard it: one asserts three rasterizations at one size
share a single instance while a second size gets its own, and one asserts a
cached instance rasterizes **identically** to a fresh one, so the cache is
invisible in the output. The key is size alone, which is sound only because
the variation location is always the default at the single call site; Task 14
made that enforceable with a `debug_assert!(location.coords().is_empty())`
rather than leaving it as a comment.

**Evidence class:** by reading. Not measured — the cost was latency on first
render, and no first-render latency benchmark was built.

### L3 — `schedule_blink_timer_if_needed` divides by `animation_fps` in integer arithmetic

`wezterm-gui/src/termwindow/mod.rs:1153-1155`

`Duration::from_millis(1000 / fps)` is zero for `animation_fps > 1000`, which
schedules an immediate timer that invalidates, repaints and reschedules — a
busy loop. `animation_fps` is user config, not attacker input, and 1000+ is
not a plausible value, so this is hygiene. **Not reproduced.**

**Disposition: fixed in `230117c`, and it was worse than this finding says.**
The integer division was not only zero above 1000 fps — it was *quantised
everywhere*: at the shipped default `animation_fps = 60` it asked for a 16 ms
frame against a true 16.667 ms, a 4% fast clock, and it collapsed every fps
above 500 to the same 1 ms. The same expression existed independently at two
sites, the timer that grants animation frames and the ease that asks for them,
so the two clocks could disagree. `bce58ea` (F4) hoisted both into one
`colorease::animation_frame_interval(fps)` using `Duration::from_secs_f64(1.0 /
fps.max(1))`, so there is now a single definition shared with the OpenGL path.
Task 14 confirmed the OpenGL path is bit-identical across the pass despite that
sharing. **The `.max(1)` in the shared helper turns out to be load-bearing
rather than defensive**, and Task 14 corrected two source comments that called
it belt-and-braces: `ColorEase::intensity_one_shot` reads
`config::configuration().animation_fps as u64` with no clamp of its own, and
`animation_fps` is an unvalidated `u8`, so `animation_fps = 0` is a legal
config value that reaches the helper — where `1.0 / 0` is `inf` and
`Duration::from_secs_f64` panics. `fps_zero_does_not_divide_by_zero` is a
genuine regression test for a config-reachable panic.

**Evidence class:** by reading. **Trigger unverified** — no busy loop and no
zero-fps panic was ever run; the arithmetic was fixed on inspection.

### L4 — `SetSelectionOwner` now uses `CURRENT_TIME`

`window/src/os/x11/window.rs:935-965`

The fork replaces `self.copy_and_paste.time` with `xcb::x::CURRENT_TIME` in
both the disown and the assert branches. The in-code comment explains why:
the stored timestamp only advances on key/button events delivered to *this*
window, so it lags behind the selection's `lastTimeChanged` and the server
silently dropped the request, making OSC 52 a no-op whenever another window
had copied more recently. ICCCM discourages `CURRENT_TIME` precisely because
it cannot lose a race against a concurrent owner, which is the trade being
made deliberately here. Recorded so the trade is visible, not as a defect to
fix. Note this is an OSC 52 fix riding along in the same patch, unrelated to
PureCpu.

**Closed as superseded, not fixed (Task 13).** The finding predates commit
`ef7b636`, "x11: fix OSC 52 clipboard silently doing nothing" (upstream PR
wezterm/wezterm#8043), which introduced this very `CURRENT_TIME` on purpose:
`copy_and_paste.time` only advances on key/button events delivered to this
window, so it lags the selection's `lastTimeChanged` and the X server
silently drops a `SetSelectionOwner` carrying a stale timestamp — OSC 52
then does nothing at all, with no error anywhere. Reverting to the stored
timestamp would reinstate that no-op in exchange for the ICCCM race
`CURRENT_TIME` cannot lose. The trade is deliberate and upstream's; there is
no code change to make here. Recorded as **obsolete**, and deliberately not
deleted, so the next reader does not re-derive it and "fix" it back.

**Evidence class:** by reading. **Obsolete — superseded by `ef7b636`.**

### L5 — Removed backends are silently substituted

`wezterm-font/src/rasterizer/mod.rs:44-54`,
`wezterm-font/src/shaper/mod.rs:145-152`,
`wezterm-font/src/locator/mod.rs:224-233`

`font_rasterizer = "FreeType"`, `font_shaper = "Harfbuzz"` and
`font_locator = "FontConfig"` are still accepted by the config schema but now
log a warning and fall through to skrifa / harfrust / fontdb. A user who
pinned one of them for a reason gets a different implementation with only a
log line to say so. Defensible for a personal fork; recorded because the
config keys still advertise choices that no longer exist.

**Disposition: closed on inspection as a non-defect (Task 14). No code
change, and none is wanted.** The finding's own title is what is wrong: the
substitution is **not silent**. All three sites warn, and each names both what
was asked for and what was used instead —
`"FreeType/Harfbuzz rasterizers have been removed, using skrifa instead"`
(`rasterizer/mod.rs:46-48`),
`"HarfBuzz shaper has been removed, using Harfrust instead"`
(`shaper/mod.rs:146`), and
`"FontConfig locator has been removed, using FontDb instead"`
(`locator/mod.rs:225`). That is the behaviour the finding asks for, already
present at every site it cites. What remains is a documentation observation,
not a defect: the config schema still accepts enum variants that no longer
select anything. Recorded as closed rather than deleted so the next reader
does not re-derive it.

**Evidence class:** by reading; **closed on inspection, non-defect.**

### N1 — A static inline image costs 0.57 ms of CPU every frame, for as long as anything holds the animation timer open

`wezterm-gui/src/termwindow/render/screen_line.rs` (`attrs.images()`),
measured through `tools/purecpu-parity/measure-round.sh`, case `static-image`

**New finding, added in Task 14 from the measurement round's results.** It is
recorded here rather than folded into another finding because it is a distinct,
quantified cost with its own mechanism.

**Not a regression from the fix pass, and not in any task's scope.** It is
pre-existing behaviour that the fix pass merely made *visible*: the pre-fix
binary never ran the frame loop in this configuration at all, so it could not
pay this cost. **It is unfixed.**

**Defect.** `attrs.images()` does not borrow — it allocates a `Vec` and
**deep-clones every `ImageCell`**, plus a `HashMap` lookup and a mutex
acquisition per image cell, and it does this **per frame**. A still image marks
no dirty region, so a source comment reasoned that "a still image costs nothing
here, forever". That is true in *rects* and false in *CPU*: the per-frame image
scan runs whenever a frame runs, and something as ordinary as a blinking cursor
— the default — holds the animation timer open and makes frames run.

**Failure scenario, measured.** `measure-round.sh`, 30 s samples,
`animation_fps = 60` (the shipped default), one window at a time, CPU read from
`utime + stime` in `/proc/<pid>/stat`:

| case | subject CPU |
|---|---|
| `idle-cursor` — blinking cursor, no image | **1.43%** |
| `static-image` — the same config plus one large static sixel | **4.83%** |

The image contributes **3.40 percentage points**, which at 60 fps is
**0.57 ms of CPU on every single frame**, spent re-cloning an image that never
changes. The two rows differ only in the presence of the image, which is what
makes the subtraction meaningful.

**What this does *not* say.** It is not a parity gap — OpenGL runs the same
`attrs.images()` scan, and on this box the GL arm costs 248.9% CPU on this very
case against PureCpu's 4.83%. It is a cost worth removing on its own terms, not
a place where PureCpu loses to the renderer it is matched against.

**Evidence class:** measured (the cost, by subtraction of two arms in one
round); by reading (the deep-clone mechanism).

**Fix direction.** Return a borrow, or skip the image scan entirely on frames
where no image cell's generation has changed.

---

## Test suite

> **Post-fix status (Task 14).** The suite is now
> `cargo test -j4 -p wezterm-gui --bin wezterm-gui` → **127 passed, 0 failed**
> and `cargo test -j4 -p wezterm-font` → **18 passed, 0 failed**, against the
> 29 + 1 recorded below. **The central complaint in this section is closed**,
> and the paragraph after it is kept because the *rest* of it is still true.
> Details below the original text.

`CARGO_BUILD_JOBS=4 cargo test -j4 -p wezterm-gui -p wezterm-font`:
**29 passed, 0 failed** in `wezterm-gui`, 1 passed in `wezterm-font`,
doc-tests empty. No failures to report as findings.

Two notes.

**The plan's command does not run.** Task 7 Step 4 specifies
`cargo test -p wezterm-gui --lib`, which fails with `error: no library
targets found in package wezterm-gui` — the crate is a binary, not a library.
Deviation: I ran `cargo test -p wezterm-gui -p wezterm-font` instead, which
covers the binary's unit tests plus the font crate this task also reviews.

**`purecpu.rs`'s own unit tests do not touch the rasterising path.** All
eleven (`purecpu.rs:613, 621, 629, 650, 661, 669, 681, 693, 703, 709, 730`)
are pure-function tests of helpers:

| test | what it covers |
|---|---|
| `srgb_linear_roundtrip`, `srgb_linear_boundary` | the two scalar conversion functions |
| `hsv_roundtrip` | `rgb_to_hsv` / `hsv_to_rgb` |
| `blend_over_opaque`, `blend_over_transparent`, `blend_over_semi` | one pixel, one call |
| `coalesce_to_bands_*` (three) | y-range merging |
| `collect_clip_rects_intersection` | rectangle intersection, non-overlapping rects only |
| `clear_rect_zeroes_region` | a 4x4 buffer, in-range rect only |

Nothing exercises `call_draw_purecpu`: no test builds a `Vertex`, no test
walks a quad, no test blits from an atlas, and no test covers the coordinate
maths of `purecpu.rs:257-360`. Every Important finding above, plus M1, M2, M6
and M7, sits in code the unit tests cannot reach — and note that
`collect_clip_rects_intersection` uses a single dirty rect, so it cannot
observe M2, while `clear_rect_zeroes_region` uses an in-range rect, so it
cannot observe M7.

The `shapecache.rs` snapshot tests were *updated* to the new font stack's
metrics (`bitmap_pixel_width` 16 -> 5 and 20 -> 8, `bearing_x` 0.0 -> 3.0 —
see `git diff e723cf5..HEAD -- wezterm-gui/src/shapecache.rs`), which records
that shaping output changed but does not assert that the new values are
correct.

### What changed, and what did not (Task 14)

**The complaint above is closed, and it was closed by evidence rather than by
adding tests and asserting they help.** The original point was not "there are
few tests" but the sharper one that **a green suite said nothing**, because a
`panic!` planted inside the blit loop left the suite green. Task 15 built a
framebuffer harness for that loop and then mutation-tested it: a mutation at
`purecpu.rs:553` **kills 7 tests**, and all 7 die **at the mutated statement**
rather than via some distant downstream assertion — including both re-pointed
`purecpu_sampler` tests. The discriminating negative is what makes that a
result rather than a coincidence:
`blit_skips_a_quad_that_misses_every_dirty_rect` correctly does **not** die,
because it `continue`s before reaching the clip loop. So the loop is now
covered in the specific sense the complaint demanded: breaking it turns the
suite red.

**Still true, and worth stating plainly rather than letting the numbers imply
otherwise.** Task 15's coverage is of the blit loop, not of everything around
it. Not covered:

- **nothing in `call_draw_purecpu` itself**;
- inside `blit_vertex_buffer`: the whole **`subpixel_aa` arm**, **`apply_hsv`**,
  **per-vertex HSV**, **`mix_value != 0`**, and the **degenerate-quad
  `continue`**;
- **none of Task 10's `mod.rs` plumbing**, `AnimatedCellScan`, or
  `image_next_frame_due` — no test executes any of it.

Note the asymmetry that leaves: the `subpixel_aa` arm is untested by the suite
but *is* measured end-to-end in `parity-matrix.md`'s subpixel row, while the
`mod.rs` animation plumbing is neither unit-tested nor covered by a static
capture — it is exercised only by the sampled animation rows, which are graded
on a colour sequence rather than on an assertion.

---

## Considered and rejected

Recorded so a reviewer does not have to re-derive them.

- **`call_draw_purecpu` draws all three buffers of the triple buffer.** It
  does not. The `for idx in 0..3` loop (`purecpu.rs:223`) iterates the render
  layer's three *z sub-layers*, exactly as `call_draw_webgpu`
  (`render/draw.rs:89-90`) and `call_draw_glium` do.
- **`stops.sort_by(... .partial_cmp(...).unwrap())` panics on NaN**
  (`skia_colr.rs:325`, `:475`). Stop offsets originate from skrifa's
  `ColorStop::offset`, a fixed-point value converted to `f32`; it cannot be
  NaN.
- **`interpolate_color_line` underflows on an empty stop list**
  (`skia_colr.rs:540-552`: `while i < stops.len() - 1`). Its only caller
  (`skia_colr.rs:494`) is guarded by `if stops.is_empty() { return }` at
  `:477`.
- **`&s[start..next_start]` slices on a non-char boundary**
  (`harfrust_shaper.rs:533`). Cluster values are the byte offsets we hand the
  shaper; monotone-grapheme clustering merges them but never introduces new
  ones, so they remain char boundaries.
- **`max_request_bytes - PUTIMAGE_HEADER_MARGIN` underflows**
  (`x11/window.rs:2205`). Requires `maximum_request_length < 64` four-byte
  units; X11 mandates at least 4096.
- **Focus change is not repainted.** `focus_changed` bumps
  `self.quad_generation` (`mod.rs:532`), which `do_paint_purecpu` reads at
  `mod.rs:1229-1233` and turns into a full repaint. Correct as written.
- **`blend_over` composites in sRGB space while the GPU blends in linear
  space.** Measurable in principle; Task 3's noise floor puts ordinary body
  text at 1 LSB, so whatever the difference is, it is below the level at
  which it could be called a defect. Not pursued.

---

## Out of scope

macOS, Windows and Wayland; upstreamability; `window_background_image`,
`window_background_opacity`, `text_background_opacity` and background
blur/HSB tint (settled out of scope by the user); the parity verdicts
themselves, which live in `parity-matrix.md`.
