# PureCpu Parity Matrix

Backend under test: `front_end = "PureCpu"`
Reference: `front_end = "OpenGL"` (Mesa llvmpipe 4.5)

> **Which binary these numbers came from (Task 14, the fix pass's verification
> checkpoint).** Every row below was re-measured against
> `/scratch/oetiker/wezterm-builds/wezterm-gui-6311e97` — the **post-fix**
> binary, built from commit `6311e97`, md5 `3c3f5bef1b91c1f08a018c2da9e054d8`.
> The pre-fix binary these documents originally described is
> `wezterm-gui-prefix` (md5 `fcf25b09103c7967a1e75b89140252d7`), which is kept
> as the control arm of every before/after comparison and must not be
> overwritten. Where a verdict changed, the Evidence cell states both the old
> and the new number, so no cell mixes pre- and post-fix figures silently.
>
> **Nine rows changed verdict in the fix pass**: sixel, iTerm2, animated GIF,
> the fancy tab bar, DECDWL/DECDHL, blinking text, the visual bell and
> subpixel-antialiased text all reached `parity`; cursor blink went from
> `missing` to `degraded`. One row is **new**: the PureCpu/OpenGL atlas-cap
> asymmetry, which is what the wide-sixel residual actually is.
>
> **The GL backend is bit-identical across the pass.** Captured from the pre-fix
> and post-fix binaries sequentially, same corpus, same generated config:
> `AE = 0, PAE = 0` on all three of `plain`, `chrome` and `images-native`, with
> non-zero ink stddev on both sides (4344.39 / 3005.55 / 4431.11, identical per
> case) so it is not a null. No PureCpu fix leaked into the OpenGL path.

Verdict vocabulary: `parity`, `degraded`, `missing`, `known gap`.
Method: `by-reading`, `needs-measurement`, `out-of-scope`.

**The Method column records how the row was *classified at the outset*, not how
it was finally settled** (Task 4, MIN-2 — worth stating plainly because it
reads the other way round). `needs-measurement` means "static reading was
suggestive but not conclusive, so this row must be measured": the behaviour
columns were pre-filled from reading in Task 1, and Tasks 3-6 then overwrote
Verdict/Evidence with measured results. So a `needs-measurement` row with a
Verdict **has** been measured — the Evidence cell names the command and the
numbers. A row that was never measured is `out-of-scope`/`known gap` and says
why in its Evidence. One consequence: grepping for `needs-measurement` does not
find unresolved rows; **the resolved-ness of a row lives in its Verdict and
Evidence cells, and every row in this table now has both** (verified in Task 8:
21 data rows, all 6 cells, every Verdict a bare vocabulary token, every
non-`known gap` verdict carrying evidence; re-verified mechanically in Task 14
after the rewrite: **22 data rows, all 6 cells, every Verdict still a bare
vocabulary token**).

`Double-width / double-height lines` was originally classified `by-reading` and
was then **measured anyway**, in a Task 8 addendum, because its reading rested
on an untested precondition. Its Method cell was moved to `needs-measurement`
to match, so no cell in this table disagrees with its own Evidence.
**`Subpixel antialiased text` was moved the same way in Task 14**, for the same
reason and by the same rule: it rested on code alone only because the harness
could not express `freetype_render_target`, and once `fd6142a` added
`gen-config.sh`'s `RENDER_TARGET` knob it was measured with a control.

Counting what is actually behind the verdicts, **as of Task 14**: the table has
**22 data rows** (21 plus the new atlas-cap row). **16 carry a command and
numbers**, **1 rests on a code citation alone** (scaled fallback/bitmap glyphs —
its precondition is font-dependent and a null result there would be
indistinguishable from parity), and **5 are out-of-scope**. The verdict tally is
**15 `parity`, 2 `degraded`, 0 `missing`, 5 `known gap`**; before the fix pass it
was 6 `parity`, 7 `degraded`, 3 `missing`, 5 `known gap`.

**The two remaining `degraded` rows are different in kind.** Cursor blink is a
real residual defect — PureCpu blinks, but as a two-level square wave against
OpenGL's eased sweep. The atlas-cap row is not a rasteriser defect at all: both
backends do the documented `AllowImage::Scale` thing, with different constants
(8192 vs 16384), so an oversized image ends up at different resolutions. Closing
that one is a memory-budget decision, not a bug fix.

**Prerequisite for every regeneration command below (fix round 1, M3):** all of
them require `PARITY_XAUTH` (and, if not `:20`, `PARITY_DISPLAY`) exported in
the shell first — `lib.sh` requires it via `: "${PARITY_XAUTH:?...}"` and fails
closed with a clear error if it's missing, so an unset value can never produce
a silently wrong number, only a loud one. Not repeated per command/row.

| Feature | GPU behaviour | PureCpu behaviour | Method | Verdict | Evidence |
|---|---|---|---|---|---|
| Text glyphs (monochrome) | Coverage mask sampled from the one atlas with a nearest sampler, tinted by `fg_color` mixed with `alt_color` by `mix_value`, `foreground_text_hsb` applied, output converted to sRGB (`glyph-frag.glsl:145-160`) | Same algorithm reimplemented per-pixel; `has_color==0.0` branch tints `fg` by atlas alpha and applies `foreground_text_hsb`, then linear→sRGB (`purecpu.rs:412-429`, `455-460`) | needs-measurement | parity | **Caveat: `parity` here means "within 1 LSB", not bit-identical** — this row is graded on Task 3's noise floor, which was measured on exactly this content (`corpus/plain.sh`: static plain monochrome text, cursor blink and text blink disabled) and is the calibration every other body row is graded against. Resolved in Task 8 from that existing measurement rather than by a new one; the row's Verdict and Evidence cells had been left empty since Task 1 (Task 6 re-review, N4). Regenerate with the commands `noise-floor.md`'s "Reproducing" section prints verbatim: `cd /scratch/oetiker/wezterm/tools/purecpu-parity && ./focus-probe.sh`, then `./calibrate.sh out/focus-B-gl.png out/focus-A-cpu.png` (focus-matched pair — a focus-mismatched pair measures the hollow-vs-solid cursor, not the glyphs; see `noise-floor.md` Finding 1). Re-run from a clean shell in Task 8 and the numbers reproduced exactly: **body `PAE = 257` (one 8-bit step — no body pixel differs by more than the smallest representable amount), body `AE = 2434` of 661000 px (0.368%) at fuzz 0, and body `AE = 0` from 0.5% fuzz upward**; `calibrate.sh` exits 0 with `PASS body PAE 257 <= 257` and `PASS body AE at fuzz 1% is 0`. The differences are confined to antialiased glyph edges: text rows (y 32-192) `AE=2434`, empty background rows (y 192-693, 501000 px) **`AE=0`**, and all three channels agree (R/G/B `AE` 2347/2341/2348, `PAE` 257 each). So PureCpu's reimplementation of the coverage-mask tint reproduces the GPU's monochrome text to within one 8-bit rounding step everywhere, with zero difference on background. **Post-fix (Task 14), the tab strip cleared too:** its fuzz-0 `AE` is now **24**, down from 188, and it reaches **0 from 0.5% fuzz upward** — where before the fix residuals there survived every fuzz value (162 at 0.5%, 157 at 1%, 43 at 50%). That was `noise-floor.md` Finding 2, the fancy tab bar's 1 px glyph shift, and it is closed: the shift test now reports `AE = 0` at `dx = 0` and 88/69 at `dx = 1`, i.e. the runs that used to be a bit-exact one-pixel-left displacement are now bit-exact *unshifted*. Body figures are unchanged by the pass, as they should be. See `noise-floor.md` |
| Inline image: sixel | Image is decoded into the same glyph atlas; each covered cell gets a quad whose texture rect is the cell's fraction of the sprite and whose destination is exactly one cell — the GPU rescales source to destination (`render/mod.rs:468-518`) | Textured quads are blitted 1:1: `blit_w = tex_w.min(dest_w)`, `blit_h = tex_h.min(dest_h)`, source stepped in lockstep with destination. No scaling exists in the rasteriser, so whenever the image's per-cell source region is not exactly `cell_width x cell_height` the image is cropped (or leaves gaps), never resampled (`purecpu.rs:344-360`) | needs-measurement | parity | **Was `degraded`; now `parity` (Task 14 re-measurement against `wezterm-gui-6311e97`).** The crop-instead-of-scale defect and the row-stepping rounding are both gone, fixed by `773b845` (I5/M1, a nearest-neighbour source step plus rounded destination coordinates). Native size, `FUZZ=1 ./compare-case.sh images-native "$PWD/corpus/images-native.sh"`, region crop `64x64+0+55`: AE=0, PAE=0 — unchanged, still bit-identical. The row that moved is the 300x300 sixel, `FUZZ=1 ./compare-case.sh images "$PWD/corpus/images.sh"`, region crop `300x300+0+341`: **AE=3300, PAE=1542 (6 LSB, failed the gate) -> AE=0, PAE=0 (bit-identical)**. Checked against a null result rather than assumed: the crop develops as sRGB with standard-deviation 12649.4 and mean 21764.5 on **both** backends, so both really drew the gradient — a blank-vs-blank pair would also report AE=0 and is the mistake this review has already made once. **The one sixel case that still fails is not this row**: the 17000x64 `wide-sixel` strip is the PureCpu/OpenGL atlas-cap asymmetry, which now has its own row below, and its residual `PAE 1285` belongs there. See the Task 14 report |
| Inline image: iTerm2 OSC 1337 | Same path as sixel (`populate_image_quad`, `render/mod.rs:441-521`); `AllowImage::Scale(n)` may store a downscaled sprite in the atlas, widening the source/destination ratio | Same 1:1 crop as sixel; a downscaled sprite makes the mismatch larger | needs-measurement | parity | **Was `degraded`; now `parity` (Task 14).** Fixed by `773b845`. Native size, region crop `64x64+0+143`: AE=0, PAE=0 — unchanged. The severe sub-case is the one that moved: the same image requested at a non-native 200x132 px and at 20x4 cells, `FUZZ=1 ./compare-case.sh images "$PWD/corpus/images.sh"`, region crop `200x200+0+77`: **AE=39360, PAE=61423 (0.937, near-maximal) -> AE=0, PAE=0**. The pre-fix visual was a dotted grid of tiny cropped tiles, one top-left fragment per covered cell, against OpenGL's solid stretched block; post-fix the two are bit-identical. Not a null result: the crop is sRGB, standard-deviation 12814.6, mean 21845 on both backends |
| Inline image: animated GIF | Frame advance happens inside `cached_image`, i.e. as a side effect of running the paint pass; the GPU path repaints on the scheduled animation timer so frames advance (`glyphcache.rs:929-970`, `render/mod.rs:468-473`) | `do_paint_purecpu` returns before `paint_impl` whenever no terminal row is dirty and no cursor-blink transition occurred, so the paint pass — and therefore frame advance — does not run for a still screen (`termwindow/mod.rs:1436-1447`). Additionally the same 1:1 crop applies to each frame | needs-measurement | parity | **Was `degraded`; now `parity` (Task 14).** Fixed by `230117c` (I3/L3 — time-driven content is marked dirty so it repaints when otherwise idle). Sampled the same way as before, since animation still cannot be diffed across backends: `./sample-case.sh gifanim "$PWD/corpus/gif-loop.sh" "4x4+14+69" 12 0.5`. Pre-fix `cpu` reported `srgb(0,128,0)` for all 12 samples — frozen on one frame for the whole 6 s window. Post-fix `cpu` returns `srgb(0,128,0) srgb(255,255,0) srgb(255,255,0) ...` — **both GIF frames observed**, the same two values `gl` reports in the same run. The sequences are not expected to match digit for digit (the two backends' animation phases are unsynchronised; see the grading-basis note below); the invariant is that both frames appear, and they now do |
| Atlas size cap: PureCpu 8192 vs OpenGL 16384 | `allocate_texture_atlas`'s `Glium` arm bails when the requested side exceeds `caps.max_texture_size`, which on this box's llvmpipe is **16384**; the bail is what drives `render/paint.rs:66-90` into its `AllowImage::Scale(n)` downscale-and-retry, so an oversized image is stored halved and the GPU samples the halved sprite | The `PureCpu` arm gained its own ceiling in `c5cc8ab` (the C1 fix) and reaches the same fallback — but the ceiling is `PURECPU_MAX_TEXTURE_SIZE = 8192` (`renderstate.rs:35`), half the GL cap, chosen because 8192 costs 256 MiB at RGBA. The two backends therefore enter `AllowImage::Scale` at **different** points and settle on different scale factors for the same image | needs-measurement | degraded | **New row (Task 14).** This is not a new defect — it is the correct identification of a residual that was previously misattributed. The wide-sixel strip's leftover difference was attributed in earlier drafts to "PureCpu has no cap and blits full-resolution"; that stopped being true when `c5cc8ab` added the cap, and Task 11 recorded the residual as the `AllowImage::Scale` asymmetry without pinning the numbers. It is **not** M2, which is separately closed. Regenerate: `FUZZ=1 PARITY_SETTLE_WARMUP=20 ./compare-case.sh wide-sixel "$PWD/corpus/wide-sixel.sh"` (the warmup is load-bearing, not decorative — without it `capture_settled` declares victory on two blank captures). Whole frame AE=64294 (fuzz 0) / 64000 (fuzz 1%); body **PAE=1285, i.e. exactly 5 LSB** — improved 10x from the pre-fix 13107 (51 LSB) once M2's double composite went away, and this 1285 is all that is left. Visible strip `1000x64+0+55`: **AE=64000 of 64000 — every pixel differs** — by a uniform few-LSB colour offset, not a crop. **The mechanism is now confirmed from both backends' own logs in the same run, which is what upgrades this from inference:** the OpenGL log says `Cannot use a texture of size 32768 as it is larger than the max 16384 supported by your GPU; will retry render with Scale(2)`, while the PureCpu log says `... larger than the max 8192 supported by the PureCpu renderer; will retry render with Scale(2)` **and then again at Scale(4)** — because at Scale(2) the atlas still wants 16384, which fits GL's cap and not PureCpu's. So GL samples a half-resolution sprite and PureCpu a quarter-resolution one, and the uniform offset is that one extra halving. Not a rasteriser defect: both backends are doing the documented thing with different constants. Closing it means matching the caps (or deriving PureCpu's from available memory), which is a deliberate memory-budget decision, not a bug fix |
| Fancy tab bar | Elements rendered through `box_model`; glyph quads are sized `texture.coords.size * glyph.scale`, so a fallback/bitmap glyph with `scale != 1` is rescaled by the GPU (`termwindow/box_model.rs:889-914`) | Solid-colour and 1:1 glyph quads are correct; any `glyph.scale != 1` quad is cropped rather than scaled (`purecpu.rs:346-347`) | needs-measurement | parity | **Was `degraded`; now `parity` within 1 LSB (Task 14).** Both defect classes are gone, fixed by `773b845` (M1: destination coordinates are rounded rather than truncated). Regenerate: `FANCY=true SPAWN_TABS=true FUZZ=1 ./compare-case.sh chrome "$PWD/corpus/chrome.sh"`. Whole frame **AE=2070 (fuzz 0) / 674 (fuzz 1%) -> AE=1441 / 0**; tab-strip crop `1000x32+0+0` **AE=801, no fuzz value clearing it -> AE=172, PAE=257 (exactly 1 LSB)**. **The 1 px shift is closed and tested as such**, not merely absorbed: over the crop `196x32+94+0` that carried the worst of it (tab 2 and 3 titles, separators, close icons), the shifted alignment used to be the better match, and now the *unshifted* one is — `dx=0` gives AE=172 against `dx=1` AE=1023. The 99 columns of larger, non-integer divergence (peak error 77/255) that a shift could not resolve are gone with it. Rounded corners, split divider and body-rest crops in the same capture are all AE=0/PAE=0. `parity` here means within 1 LSB, like body text, not bit-identical |
| Retro tab bar | Rendered as ordinary cells plus `util_sprites.white_space` / `filled_box`, both allocated in the same atlas (`render/tab_bar.rs:49-50`, `utilsprites.rs:159-162`) | Same quads; solid-colour quads take the `IS_SOLID_COLOR` fill path, glyphs the 1:1 path (`purecpu.rs:300-341`) | needs-measurement | parity | **Caveat: this is the one row graded on a threshold borrowed from the body noise floor (with stated justification below), not on the same threshold-free footing as Window buttons/Rounded corners/Split dividers, which are bit-identical (`AE=0`/`PAE=0`) and need no threshold at all — parity here is "within 1 LSB", not "no difference".** `FANCY=false SPAWN_TABS=true FUZZ=1 ./compare-case.sh chrome-retro "$PWD/corpus/chrome.sh"` (`tools/purecpu-parity/`) — `out/chrome-retro-gl.png` vs `out/chrome-retro-cpu.png` (hermetic corpus, as above). Whole-frame AE=1427 (fuzz 0) / **0 (fuzz 1%)** — the fancy-tab-bar shift/divergence defects do not reproduce here (a genuine defect is never absorbed by fuzz; this is a different, unaffected render path, confirmed separately below). Tab-strip crop `1000x32+0+0`: AE=281 raw, PAE=257 (exactly 1 LSB, the Task 3 body-gate boundary value); narrowed to the retro bar's actual 21px height (crop `1000x21+0+0`): AE=158, PAE=257 — ordinary antialiasing noise, same order of magnitude as the plain-text noise floor Task 3 established for the terminal body, and it clears at fuzz 1% (AE=0). The retro tab bar draws tab titles as ordinary monospace terminal cells rather than `box_model` glyph runs with float-computed positions, which is consistent with it not sharing the fancy bar's defects. |
| Window buttons | Built as `box_model` Elements (polys + glyphs) drawn from the same atlas (`render/window_buttons.rs`) | Same; poly sprites are generated at their final pixel size so the 1:1 blit is exact | needs-measurement | parity | Window buttons only render in `box_model`'s fancy-tab-bar path when `window_decorations` includes `INTEGRATED_BUTTONS` (`fancy_tab_bar.rs:314,372`); the harness's `gen-config.sh` did not previously expose this, so it was extended with a `WINDOW_DECORATIONS` env var (default unset, so every existing case reproduces byte-for-byte unchanged — verified with `diff` against the pre-Task-5 generator's output for every front-end/FANCY combination). **The regeneration command needs a raw pipe character in `WINDOW_DECORATIONS`, which this table cell cannot hold without breaking its own column count — see the fenced command directly below the table (review round 1, C1): a table-cell copy of this command previously required a backslash-escaped pipe that reached the shell intact and made `gen-config.sh` emit invalid Lua, silently dropping the whole config (including `front_end`) to defaults and reporting a false, self-comparing AE=0/PAE=0.** `out/chrome-buttons-gl.png` vs `out/chrome-buttons-cpu.png` (hermetic corpus content in both panes) show minimise/maximise/close icons rendered top-right. Whole-frame AE=2070/674/257 (fuzz 0 / fuzz 1% / body PAE), identical to the Fancy tab bar row's numbers since it's the same corpus and window layout plus buttons. Region crop `140x32+860+0` (the button cluster): AE=0, PAE=0 — bit-identical. This case could have shown the same class of defect as the tab-title glyphs (the buttons are also `box_model` polys/glyphs) but did not, so the parity verdict is not a structural inability to detect a defect. |
| Rounded corners | `poly_quad` rasterises the corner poly into the atlas at exactly the requested corner size and marks it `IS_GRAY_SCALE` (`render/mod.rs:290-328`, `termwindow/box_model.rs:1038-1092`) | `has_color==4.0` branch: `fg` tinted by atlas alpha; source and destination sizes are equal by construction, so the 1:1 blit is exact (`purecpu.rs:406-411`) | needs-measurement | parity | Exercised in every fancy-tab-bar capture above (every fancy tab, active and inactive, has `border_corners` set — `fancy_tab_bar.rs:183-195,229-241`); only the active tab's two corners were actually measured, since inactive tabs share the bar background and their corners are not visually separable from it. Top-left and top-right corners of the active tab, `out/chrome-gl.png` vs `out/chrome-cpu.png`, crops `10x10+0+0` and `10x10+79+0` (re-located after the hermetic-corpus fix changed tab-title text from "bash" to "sleep", which widened the active tab and moved its right edge): AE=0, PAE=0 — bit-identical, unaffected by the adjacent title-text shift. |
| Split dividers | `filled_rectangle` → `IS_SOLID_COLOR` quad, texture ignored by the shader (`render/split.rs:32,55`, `render/mod.rs:266-287`, `glyph-frag.glsl:115-118`) | `IS_SOLID_COLOR` fill path, colour-only, size-independent (`purecpu.rs:300-341`) | needs-measurement | parity | `SPAWN_TABS=true`'s `gui-startup` handler (`tools/purecpu-parity/gen-config.sh`) splits the first tab right at 50%; that handler unconditionally activates the last-spawned tab (tab 3) once done, which put the split off-screen for every capture until fixed by adding an explicit `tab:activate()` at the end of the handler to reselect the split tab (see `gen-config.sh` diff — this is what "gui-startup does not exercise what the row claims" means in practice: the event fired, but the config it produced could not show a divider in any capture without this fix). With the fix (and the later hermetic-corpus fix, review round 1 I4, which does not move the divider — it is independent of tab-title text), `out/chrome-gl.png` vs `out/chrome-cpu.png` show the vertical divider at x=495; column crop `1x661+495+32` (full body height): AE=0, PAE=0 — bit-identical. |
| Cursor (static) | Block/bar/underline drawn as a poly or solid quad at exact pixel size | Same quads; 1:1 blit is exact | needs-measurement | parity | Animation cannot be diffed frame-for-frame across backends (phases are not synchronised — see "Grading basis for animation rows" below); this row is the one exception, since with `cursor_blink_rate=0` (the harness default) the cursor is genuinely static in both backends and the standard cross-backend capture applies. `cd tools/purecpu-parity && FUZZ=1 ./compare-case.sh cursor "$PWD/corpus/cursor.sh"` — `out/cursor-gl.png` vs `out/cursor-cpu.png`. Whole-frame AE=1095 (fuzz 0) / 132 (fuzz 1%); body PAE=257, exactly the Task 3 gate boundary (both captures warn `Gray` colorspace, expected and correct here per the Fancy-tab-bar row's precedent — this corpus is deliberately monochrome text). The whole-frame noise is ordinary glyph antialiasing elsewhere in the frame, not the cursor: isolated to the cursor cell alone (crop `24x24+375+70`, located by inspecting `out/cursor-gl.png`/`out/cursor-cpu.png` directly), `compare -metric AE -fuzz 1% <(convert out/cursor-gl.png -crop 24x24+375+70 +repage png:-) <(convert out/cursor-cpu.png -crop 24x24+375+70 +repage png:-) null:` and the PAE equivalent both give **AE=0, PAE=0 — bit-identical**. Reproduced from a clean shell. **Bar and underline shapes (fix round 1):** the GPU-behaviour claim above names all three cursor shapes, but the run above only exercises the default `SteadyBlock`; also measured `DEFAULT_CURSOR_STYLE=SteadyBar FUZZ=1 ./compare-case.sh cursor-steadybar "$PWD/corpus/cursor.sh"` and the same with `SteadyUnderline` — both AE=1095/132/PAE=257 whole-frame (ink stddev 2951.04/2951.66 for bar, 2940.8/2941.41 for underline — different from `SteadyBlock`'s 3080, confirming the shape genuinely changed and the instrument is not blind to it), and both give cursor-cell crop `24x24+375+70` **AE=0, PAE=0**, same as block. `parity` holds for all three shapes. |
| Cursor blink | `ColorEase::intensity_continuous()` is evaluated on the CPU every animation frame and delivered as `fg_color_mix` / `cursor_border_mix`, giving a continuous eased fade (`render/mod.rs:680-696`) | Config-driven blink (`default_cursor_style` set to a `Blinking*` variant, the documented way to enable it) never starts: `do_paint_purecpu`'s blink-detection tests the raw, unresolved cursor shape, which never satisfies `is_blinking()` for this path. An app that drives the shape itself via DECSCUSR does blink, but quantised to ~2 paints/cycle with a shallow floor, not the continuous eased fade (see Evidence) | needs-measurement | degraded | **Was `missing`; now `degraded` (Task 14).** Fixed by `868159c` (I4 — the cursor shape is resolved through `effective_shape` before `is_blinking()` is tested), so config-driven blink now runs. It is `degraded` rather than `parity` because PureCpu's blink is a two-level square wave where OpenGL's is an eased sweep. Graded with a control/subject pair in one experiment, which is what makes the result readable: `CURSOR_BLINK_RATE=600 DEFAULT_CURSOR_STYLE=BlinkingBlock ANIMATION_FPS=30 ./sample-case.sh <case> "$PWD/corpus/cursor.sh" "4x4+382+78" 24 0.15`. **Pre-fix binary, `cpu`: `gray(224)` for all 24 samples** — the documented defect reproducing exactly. **Post-fix binary, `cpu`: alternates `gray(161)` and `gray(224)` for nine full cycles.** Reference `gl` in the same runs alternates about `gray(161)` and `gray(215)` on both binaries. **A sampling-interval caveat that matters if you re-run this:** at 0.4 s, 0.1 s and 0.2 s the reference `gl` arm aliases against the blink cycle and reads nearly flat, at which point a flat `cpu` arm proves nothing and the row is ungradeable. 0.15 s resolves both arms. Do not grade this row on an interval where the reference is flat. The app-driven DECSCUSR sub-probe was **not** re-run in Task 14 (it needs the hand-written config fenced below, which the every-config-from-gen-config rule excludes from routine runs), so its quantisation sub-finding stands on the Task 6 measurement |
| Blinking text attribute | `blink_state` / `rapid_blink_state` intensity is applied CPU-side to the cell foreground each paint, and `update_next_frame_time` schedules the next frame (`render/screen_line.rs:797-825`) | Nothing marks cells carrying the blink attribute as dirty, so once the window settles the word is frozen forever at whichever intensity the eased fade happened to be at on that one paint — deterministically the fully-invisible end, not by chance (see Evidence) | needs-measurement | parity | **Was `missing`; now `parity` (Task 14).** Fixed by `230117c` (I3/L3). Regenerate: `TEXT_BLINK_RATE=400 ANIMATION_FPS=30 ./sample-case.sh <case> "$PWD/corpus/cursor.sh" "1x1+122+60" 16 0.4`. **Pre-fix binary, `cpu`: `gray(16)` — the background colour — for all 16 samples**, i.e. the word frozen fully invisible, reproducing the documented defect in the same session as the subject. **Post-fix binary, `cpu`: `gray(22) gray(142) gray(128) gray(127) gray(120) gray(128) gray(130) gray(136) gray(110) gray(134) ...`** — varying across the same band the reference `gl` arm covers in the same run (`gray(108)`-`gray(146)`). Range and variance are the invariant here, never the digits: both arms sample an eased fade whose phase is not synchronised with the sampler |
| Visual bell | `get_intensity_if_bell_target_ringing` returns a continuously varying mix that is applied to the cell/cursor background each paint (`render/mod.rs:233-258`, `535-565`, target defaults to `BackgroundColor`: `config/src/bell.rs:62-69`, applied in `render/pane.rs:174-206`) | The bell's `Alert::Bell` handler invalidates the window but marks no dirty rect, so `do_paint_purecpu`'s idle-skip still takes the early-exit branch and the bell's background-mix computation never runs (see Evidence) | needs-measurement | parity | **Was `missing`; now `parity` (Task 14).** Fixed by `230117c` (I3/L3). Regenerate: `VISUAL_BELL=true ./sample-case.sh <case> "$PWD/corpus/cursor.sh" "1x1+500+200" 16 0.4`, against a corpus that rings the bell once a second. **Pre-fix `cpu`: flat `gray(16)` for all 16 samples — the bell never visibly rang.** **Post-fix `cpu`: `gray(169) gray(16) gray(94) gray(132) gray(97) gray(172) gray(97) gray(172) gray(16) ...`** — flashing and fading through the same three-to-four level band as `gl` (`gray(169) gray(16) gray(93) gray(16) gray(98) gray(174) ...`). Still scoped to the default `BackgroundColor` bell target: the sampled pixel is empty background, so `VisualBellTarget::CursorColor` remains unmeasured |
| Double-width / double-height lines (DECDWL/DECDHL) | Glyph destination is scaled by `width_scale` / `height_scale` while the atlas source stays at base size; the GPU rescales (`render/screen_line.rs:50-63`, `632-647`) | 1:1 blit clips to the smaller of source and destination, so the glyph is drawn at base size in the top-left of the enlarged cell (`purecpu.rs:346-347`). The DECDHL **bottom** half is worse than that: `render_screen_line` early-returns for a double-height-bottom line because the *top* line's quad is drawn at twice the cell height and is meant to cover both row bands (`screen_line.rs:31-38`) — the 1:1 blit clips that quad to the source height, so the bottom band receives no quad at all and stays empty (`purecpu.rs:346-347`) | needs-measurement | parity | **Was `degraded` with a scoped `missing` sub-case; now `parity` (Task 14).** Fixed by `773b845` (I5 — the blit scales instead of cropping), and this row is the sharpest evidence that the scaler is real, because the corpus carries its own reachability control. Regenerate: `FUZZ=1 ./compare-case.sh dwl "$PWD/corpus/dwl.sh"`. Whole frame **AE=10476 (fuzz 0) / 9349 (fuzz 1%) -> AE=3009 / 0**; body **PAE=45232 (176 LSB, failing the gate by two orders of magnitude) -> PAE=257 (1 LSB)**. Per band, all now at the noise floor where three of them were at 45232: control single-width `1000x22+0+32` and `1000x22+0+120` PAE=257 (unchanged), DECDWL `1000x22+0+54` PAE=257, DECDHL top `1000x22+0+76` PAE=257, DECDHL bottom `1000x22+0+98` PAE=257. **Ink-pixel counts (threshold 30%) are what show the geometry is now right rather than merely close**, since a ratio of 2.01 was the old signature of drawing at base size in a doubled cell: control 1152/1149 (ratio 1.00, unchanged), DECDWL **2284/2278 (was 2284/1139, ratio 2.01 -> 1.00)**, DECDHL top **1688/1688 (was 1688/804, 2.10 -> 1.00)**, DECDHL bottom **2008/2004 (was 2008/119, ratio 16.9 -> 1.00)**. The blank bottom half is filled in: over y=99-110, where OpenGL lays down 1824 ink px and PureCpu used to lay down **exactly 0**, PureCpu now lays down **1820**. The scoped `missing` sub-case is closed |
| Scaled fallback / bitmap glyphs (`glyph.scale != 1`) | Destination is `sprite size * glyph.scale`; the GPU rescales (`render/screen_line.rs:380-393`, `termwindow/box_model.rs:902-913`). `scale` is set below 1 for oversized or unscaled bitmap glyphs such as colour-emoji fonts (`glyphcache.rs:749-798`) | 1:1 blit crops the sprite to the destination rectangle instead of downscaling | by-reading | parity | **Was `degraded`; now `parity` by reading, and still deliberately not measured (Task 14).** The verdict flips because the *mechanism* it rested on no longer exists: it was the same `purecpu.rs:346-347` 1:1 crop, and `773b845` replaced that with a nearest-neighbour source step, which the DECDWL/DECDHL row above now demonstrates with numbers on the same code path. What has **not** changed is the reason this row was never measured: the precondition is font-dependent — some installed font must actually yield `glyph.scale != 1` — and a capture that used no such font would return a null result indistinguishable from parity, which is the Task 4 CRIT-1 mistake this review made once already. So read this as an untested prediction that now points at parity instead of at a defect, not as a result. Missing evidence is unchanged: establish which installed font produces a scaled sprite, then capture it |
| Subpixel antialiased text (`freetype_render_target = "HorizontalLcd"`) | Layer 1 is drawn with dual-source blending using the per-channel `colorMask` output (`render/draw.rs:172-193`, `244`, `260-266`; `glyph-frag.glsl:14-17`, `147-152`) | The rasteriser has a single scalar alpha and no per-channel mask; `blend_over` blends one alpha for all channels (`purecpu.rs:529-543`). **PureCpu does not lose the glyph: it falls back to ordinary grayscale antialiasing**, because the LCD path already stores a usable scalar alpha alongside the per-channel coverage | needs-measurement | parity | **Was `degraded` by reading; now `parity` and MEASURED (Task 14).** Two things changed since that verdict. `fd6142a` implemented per-channel subpixel antialiasing in the rasteriser to match the GL dual-source blend, and the same commit taught `gen-config.sh` the `RENDER_TARGET` knob — so the config the harness could not previously express is now expressible and this row no longer rests on code alone. Measured as a control/subject pair in one session: `RENDER_TARGET=HorizontalLcd FUZZ=1 ./compare-case.sh <case> "$PWD/corpus/plain.sh"`. **Pre-fix binary: body PAE=29041 (113 LSB), AE=10417 at fuzz 0 and still 10027 at fuzz 1%, cpu ink stddev 4800.87 against gl's 4281.4** — the documented grayscale fallback, clearly divergent, and the control that proves the knob reaches the renderer. **Post-fix binary: body PAE=257 (1 LSB), AE=6489 at fuzz 0 and 0 at fuzz 1%, cpu ink stddev 4283.25 against gl's 4281.4.** `parity` within 1 LSB, on the same footing as body text. `gen-config.sh` rejects an invalid `RENDER_TARGET` outright, so a typo cannot silently drop the config to defaults and self-compare |
| Window background image sampling filter | `IS_BG_IMAGE` quads are sampled through `atlas_linear_sampler` (bilinear) rather than the nearest sampler (`glyph-frag.glsl:119-127`, `render/draw.rs:220-223`) | `has_color==2.0` branch does the same 1:1 nearest crop as every other textured quad (`purecpu.rs:394-399`) | out-of-scope | known gap | **Still `known gap`, and the reading has been updated rather than left stale (Task 14).** Background images remain out of scope by the user's scoping decision, so this has still never been captured. But the predicted defect is no longer the generic one: `773b845` gave the rasteriser a nearest-neighbour resampler, and `IS_BG_IMAGE` (`has_color == 2.0`) was **deliberately kept on the pre-fix crop path** when it landed — the source says so at `termwindow/render/purecpu.rs` ("`has_color == 2.0` (IS_BG_IMAGE) stays on the pre-Task-7 path", and `blit_end` still crops for it). So the gap is now a deliberate scope boundary rather than an unnoticed consequence of the missing resampler, and it is two divergences deep: PureCpu crops where the GPU rescales, and the GPU rescales *bilinearly* through `atlas_linear_sampler` where PureCpu's new sampler is nearest. Treat it as an untested prediction, as before |
| window_background_image | n/a | n/a | out-of-scope | known gap | user decision, not investigated |
| window_background_opacity | n/a | n/a | out-of-scope | known gap | user decision, not investigated |
| text_background_opacity | n/a | n/a | out-of-scope | known gap | user decision, not investigated |
| Background blur / HSB tint | n/a | n/a | out-of-scope | known gap | user decision, not investigated |

### Window buttons regeneration command

Reproduces the Window buttons row above. Copy this fenced block, not the
table cell — a Markdown table cell cannot hold a literal `|`, and the
review-round-1 Critical finding (C1) was exactly that a backslash-escaped
pipe in the table cell reached the shell intact, made `gen-config.sh` emit
Lua wezterm rejects wholesale, and silently produced a false, self-comparing
`AE=0`/`PAE=0` that matched this row's real numbers for the wrong reason.
`gen-config.sh` now refuses a `WINDOW_DECORATIONS` value containing a
backslash outright, so that specific failure can no longer happen silently
either way — but the command below is the one to actually run:

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
FANCY=true SPAWN_TABS=true WINDOW_DECORATIONS="INTEGRATED_BUTTONS|RESIZE" FUZZ=1 \
    ./compare-case.sh chrome-buttons "$PWD/corpus/chrome.sh"
```

### Cursor-blink DECSCUSR probe regeneration commands

Reproduces the Cursor blink row's app-driven-path sub-finding ("`degraded`,
not parity" — gray(159) floor vs. GL's continuous sweep toward background).
`gen-config.sh` correctly refuses `cursor_blink_rate` set with a
non-`Blinking*` `default_cursor_style` (its symmetric blink-rate/cursor-style
guard), which is exactly the combination this probe needs — the cursor shape
must come from the DECSCUSR escape sequence alone, with nothing in the config
resolving it — so the config is hand-written here rather than generated:

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
source ./lib.sh

cat > /tmp/decscusr-corpus.sh <<'CORPUS'
#!/usr/bin/env bash
set -euo pipefail
printf 'DECSCUSR CASE\n'
printf '\033[1 q'
printf 'cursor rests at the end of this line: '
exec sleep 600
CORPUS
chmod +x /tmp/decscusr-corpus.sh

for fe in OpenGL PureCpu; do
cat > "$PARITY_OUT/decscusr-$fe.lua" <<LUA
local wezterm = require 'wezterm'
return {
  front_end = '$fe',
  font_size = 12.0,
  initial_cols = 100,
  initial_rows = 30,
  cursor_blink_rate = 600,
  text_blink_rate = 0,
  animation_fps = 30,
  default_cursor_style = 'SteadyBlock',
  enable_tab_bar = true,
  use_fancy_tab_bar = true,
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
LUA
done

for side in cpu gl; do
  fe=PureCpu; [ "$side" = "gl" ] && fe=OpenGL
  kill_class "par-decscusr-$side"
  launch "par-decscusr-$side" "$PARITY_OUT/decscusr-$fe.lua" "/tmp/decscusr-corpus.sh"
  W=$(find_window "par-decscusr-$side")
  check_no_config_error "par-decscusr-$side"
  pause 2
  printf '%s: ' "$side"
  sample_region "$W" "1x1+382+58" 30 0.1
  echo
  kill_class "par-decscusr-$side"
done
```

Sample crop `1x1+382+58` is the cursor cell for this two-line corpus (row 1,
not row 2 — `DECSCUSR CASE` is the corpus's only line before the cursor line,
one fewer than `corpus/cursor.sh`'s three). Backends are launched and
sampled sequentially, one window at a time, matching every other command in
this document.

## Settled by reading: the single-texture question

The hypothesis was that `call_draw_purecpu` binds one texture and therefore
cannot draw content that the GPU path sources from some other texture.

**The hypothesis is false, and for a good reason: the GPU path has only one
texture too.** `call_draw_glium` binds exactly one `SrgbTexture2d` — the glyph
cache atlas (`render/draw.rs:157`) — and exposes it twice, as
`atlas_nearest_sampler` and `atlas_linear_sampler` (`render/draw.rs:215-223`),
two samplers over the same texture. Every `Sprite` in the GUI is allocated from
the single `GlyphCache::atlas` (`glyphcache.rs:562`): glyphs
(`glyphcache.rs:873`), custom block/poly glyphs (`glyphcache.rs:1340`), solid
colour swatches (`glyphcache.rs:1132`), inline images and animation frames
(`glyphcache.rs:922`, `978`, `1071`), and the util sprites
(`utilsprites.rs:159-162`). `GlyphCache::new_in_memory` exists but is only used
by the `ls-fonts` CLI and unit tests, never by a window.

So no content class is invisible under PureCpu because of texture binding. The
real structural gap was one level down, in sampling: the GPU interpolates
texture coordinates across each quad and therefore rescales whenever source and
destination sizes differ, while `call_draw_purecpu` blitted 1:1 and clipped
(`purecpu.rs:344-360`). Content whose quads relied on that rescale — inline
images, double-width/height lines, scaled fallback glyphs — was drawn from the
right texture but at the wrong size.

**Both structural gaps are closed as of Task 14, and the rows above are the
evidence.** `773b845` gave the rasteriser a nearest-neighbour source step, which
is what moved the two inline-image rows and DECDWL/DECDHL to `parity`; the one
quad class deliberately left on the old crop path is `IS_BG_IMAGE`, which is out
of scope. The second gap was `do_paint_purecpu`'s idle skip
(`termwindow/mod.rs:1436-1447`), where time-driven animation that changed no line
seqno and marked no dirty rect never reached the paint pass; `230117c` gives
blink, bell and animated images their own dirty-rect producers, which moved
blinking text, the visual bell and the animated GIF to `parity`, and `868159c`
resolves the cursor shape before testing it for blink.

What is left is smaller and differently shaped: the cursor blink's waveform is
quantised rather than eased, and the two backends cap their atlases at different
sizes. Neither is a missing capability.

## Grading basis for chrome pixels (Task 5)

> **Post-fix status (Task 14).** The open defect this section was built around —
> the fancy tab bar's bit-exact 1 px glyph shift, plus the larger non-integer
> divergence beside it — is **closed**, fixed by `773b845`. The tab strip now
> measures `PAE = 257` and clears at 1% fuzz, i.e. it behaves like the body. The
> method below is kept, unchanged and still binding, for two reasons: it is how
> the post-fix numbers were graded (the shift test is what proves the shift is
> gone rather than merely smaller — `dx = 0` now wins where `dx = 1` used to),
> and the tab strip still has no independently established noise floor of its
> own, so a future chrome regression must still be graded on explicit per-region
> pixel evidence rather than on a borrowed threshold.

Task 3's gate (`FUZZ = 1` plus a body `PAE <= 257` assertion) was calibrated
against plain terminal-body text and explicitly does **not** extend to the tab
strip (`y < 32`), which Task 3 left as STOP with no established clean noise
floor. This review does not invent a chrome threshold to fill that gap —
doing so risks dissolving the confirmed 1px shift into a passing number, which
is exactly what the plan's Global Constraints forbid.

Instead, chrome pixels here are graded on explicit, per-region pixel evidence:

- **Window buttons, rounded corners, split dividers**: graded on raw `AE`/`PAE`
  at fuzz 0 over a crop tight to that element alone. Every crop below came
  back bit-identical (`AE = 0`, `PAE = 0`); at that resolution there is no
  threshold question to answer — parity holds because nothing differs.
- **Fancy tab bar title/icon glyphs**: graded by testing whether GPU pixels at
  column `x` exactly equal PureCpu pixels at column `x-1` (a one-pixel
  integer shift), using bit-exact `AE = 0` on the shifted alignment as proof
  of a genuine displacement rather than antialiasing noise. This reuses Task
  3's own method (`noise-floor.md`, Finding 2) rather than introducing a new
  one. Glyph runs that are *not* shifted are confirmed by the same test
  returning `AE = 0` at the *unshifted* (`dx = 0`) alignment.
- **Retro tab bar**: graded against Task 3's plain-text noise floor
  (`FUZZ = 1`), because retro renders tab titles as ordinary monospace cells —
  the same code path Task 3's gate was calibrated against — not through
  `box_model`. The measured `PAE = 257` (exactly 1 LSB) at fuzz 0 sits right
  at Task 3's own boundary value, and clears completely at fuzz 1%.

**Corpus hermeticity (review round 1, I4 — fixed, not just documented).**
`gui-startup` completely replaces wezterm's normal startup, so a handler that
spawns windows/tabs/panes without forwarding the CLI's command discards
`corpus/chrome.sh` entirely; every tab and pane then ran the operator's own
login shell and its `~/.bashrc` prompt. The first round of this task only
recorded that leak (tab titles reading `1: bash`, two lines of prompt text
with real nerd-font colour icons in the body, `PAE = 514` on the default body
crop). It is fixed now: `gen-config.sh`'s `gui-startup` handler passes
`args = cmd.args` to every `spawn_window`/`spawn_tab`/`split` call, so every
capture in this row's evidence runs the corpus's own deterministic text (tab
titles read `1: sleep`/`2: sleep`/`3: sleep`, matching Task 3's naming for the
same corpus pattern). With that fix, the default body crop reports
`PAE = 257` — exactly Task 3's gate boundary, confined entirely to the
corpus's own two printed lines, with the rest of the body bit-identical —
so it no longer needs to be explained away as out-of-scope contamination.

One side effect of the fix worth recording at the point of use:
`compare-case.sh`'s Gray-colourspace warning fires for every capture in this
row, because the corpus's own banner text has no colour content, unlike the
shell prompt it replaced. That is the expected, correct colourspace for this
specific corpus (confirmed by inspecting the captures directly — both show
real, non-blank content), not the "backend hadn't drawn anything yet" case
the warning exists to catch; `compare-case.sh`'s own comment anticipates
this ("If a future case legitimately produces a Gray capture, say so where
that case is defined") and this is that case.

## Grading basis for animation rows (Task 6)

Cursor blink, blink-attribute text and the visual bell cannot be diffed
frame-for-frame across backends the way every static row above is: each
backend's blink/bell phase is driven by its own wall-clock start time, so a
GL capture and a PureCpu capture taken "at the same moment" are, in general,
at *different* points in an uncoordinated cycle. A cross-backend pixel diff
of two such captures would measure phase offset, not a rendering defect —
meaningless in either direction (a lucky phase match would report a false
`parity`; an unlucky one would report a false defect).

So these three rows are graded differently from every other row in this
matrix: **sampled within a backend, not diffed across backends.** Each case
uses `sample-case.sh <case> <corpus> <geom> <count> <interval>` (built on
`lib.sh`'s `sample_region`, reused unchanged from Task 4) to sample one pixel
repeatedly, several seconds apart, once per backend, and prints the raw
colour sequence for a human (or this document) to read directly — no
settling, no threshold, no fuzz. The question each row answers is: **does
the sampled value change over time in this backend, and how?** A backend
whose sequence is flat when the other backend's is not has not merely
degraded the effect, it has not run the code path that produces it — see
each row's Evidence for why, citing the specific dirty-rect/scheduling gap
in `do_paint_purecpu`.

The one exception is the static-cursor row, which is graded the ordinary
cross-backend way: with `cursor_blink_rate = 0` (the harness default used
everywhere except these three rows) the cursor is not time-driven in either
backend, so there is no phase-synchronisation problem to avoid.

A corpus must be able to trigger the defect its row claims to test. All four
rows here share `corpus/cursor.sh` (a static cursor position, an SGR 5
blink-attribute word, and a bell rung once per second in a background loop
so it stays ringing across the whole sampling window); which behaviour is
exercised is selected by the config the driver generates
(`CURSOR_BLINK_RATE`/`DEFAULT_CURSOR_STYLE`, `TEXT_BLINK_RATE`,
`VISUAL_BELL`), all consumed through `gen-config.sh`'s existing guard against
the blink-rate/cursor-style pairing trap (see its own comments) — except the
cursor-blink row's DECSCUSR follow-up probe, which deliberately writes its
own config to bypass that guard (the guard correctly refuses
`cursor_blink_rate` set with a non-`Blinking*` `default_cursor_style`, which is
exactly the combination a raw-DECSCUSR probe needs — the shape must come from
the escape sequence alone, not from config). **It is cited as evidence, for
one specific claim only** (fix round 2, N1): that the app-driven cursor-blink
path is itself `degraded`, not the config-driven path's `missing` verdict,
which does not depend on it. Its regeneration commands are fenced below (same
pattern as the Window buttons row), since a hand-written config cannot be a
one-line command in a table cell.

**Two sequences from a continuously varying signal are never expected to
reproduce digit-for-digit** (fix round 2, N2): the cursor-blink and
blink-attribute-text rows' `gl` sequences are samples of an eased fade whose
phase is not synchronised with the sampling interval, so a re-run lands on
different points of the same sweep. The invariant a re-run must reproduce is
the *range and variance* of the sequence (e.g. "sweeps continuously across
17-222"), never the specific digits. The one exception is the visual-bell
row's `gl` sequence, which *does* reproduce byte-for-byte: a 1Hz ring sampled
every 0.4s aliases onto exactly three fixed phase offsets every cycle, so
repeated runs keep landing on the same three levels rather than sweeping
through a continuum.

**The sampling interval is part of the instrument, and it can silently disarm
this whole method** (Task 14). These rows are graded by "does the sampled value
change over time in this backend" — which presupposes that the *reference* arm
visibly changes. It does not always. Re-grading cursor blink post-fix at
`cursor_blink_rate = 600`, the OpenGL arm read essentially **flat** at 0.4 s,
0.1 s and 0.2 s sampling intervals, and flat again at 0.2 s with the rate
widened to 2000 ms: the interval aliases against the eased cycle, and each
`import` costs a non-trivial and variable fraction of the interval, so the
samples keep landing on the same phase. At 0.15 s both arms resolve cleanly.
**A flat subject arm against a flat reference arm is not evidence of anything**,
and it looks exactly like the pre-fix defect — so before reading a flat PureCpu
sequence as a finding, check that the GL arm in the same run is not also flat.
Where a row's verdict depends on this, grade it with the pre-fix binary as a
control in the same session, which is how the cursor-blink and blinking-text
rows above were settled.

**Declared unmeasured within these rows** (Task 6's implementer disclosed
these; Task 6's reviewer judged all three legitimate scope boundaries that
undermine no verdict; recorded here in Task 8 so the disclosure lives in the
published document and not only in the task reports). Three things inside the
scope of these rows were **not** exercised: `VisualBellTarget::CursorColor`
(the visual-bell row samples an empty-background pixel, which exercises the
default `BackgroundColor` target only); rapid blink (SGR 6 /
`text_blink_rate_rapid`, as distinct from the SGR 5 / `text_blink_rate` path
the blinking-text row measures); and any interaction between the three
animations when more than one is active at once. Each row's verdict covers
what its Evidence cell actually sampled, and no more.

**Grading basis: a row is graded against its own named feature, not the
generic mechanism behind it** (fix round 2, addition per controller ruling,
now also a plan Global Constraint). Animated GIF and Cursor blink are both
`termwindow/mod.rs:1436-1447`'s idle-skip early exit, with the same observable
shape (PureCpu freezes, GL keeps animating) — yet one is graded `degraded` and
the other `missing`, and that is not an inconsistency: an animated GIF is an
image, and PureCpu draws it correctly, just as a single still frame — the
*image* is delivered, only its motion is lost, so it is partly there
(`degraded`). Cursor blink's entire named feature *is* the motion; when it
doesn't run, none of what the row claims to measure happens at all
(`missing`). A row's verdict answers "how much of *this row's feature*
survived", not "which mechanism broke".

## `Software` versus `PureCpu`

`FrontEndSelection::Software` is consumed in exactly one place:
`window/src/configuration.rs:11`, which makes `prefer_swrast()` true. That flag
only steers EGL/WGL configuration selection towards a software rasteriser
(`window/src/egl.rs:434-478`, `window/src/os/windows/wgl.rs:95`). Everything
above that — glium context, shaders, quad generation, `call_draw_glium` — is the
`OpenGL` front end unchanged. `Software` is therefore "OpenGL, but insist on
llvmpipe"; on this GPU-less machine it is behaviourally identical to `OpenGL`.

`PureCpu` skips OpenGL entirely: no GL context is created
(`termwindow/mod.rs:853-856`), the atlas is a plain `ImageTexture` in system
memory (`renderstate.rs:113-116`), and quads are rasterised by hand into a
`Vec<u8>` framebuffer that is pushed to the window with
`present_software_frame_region`. What PureCpu adds over `Software` is the
removal of the GL/Mesa stack and the ability to repaint only dirty regions; what
it gives up is the shader, the samplers, and per-frame animation repaints.
