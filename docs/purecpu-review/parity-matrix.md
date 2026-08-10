# PureCpu Parity Matrix

Backend under test: `front_end = "PureCpu"`
Reference: `front_end = "OpenGL"` (Mesa llvmpipe 4.5)

Verdict vocabulary: `parity`, `degraded`, `missing`, `known gap`.
Method: `by-reading`, `needs-measurement`, `out-of-scope`.

Rows marked `needs-measurement` have their behaviour columns pre-filled from
static reading where that reading is suggestive but not conclusive; later tasks
overwrite Verdict/Evidence with measured results.

| Feature | GPU behaviour | PureCpu behaviour | Method | Verdict | Evidence |
|---|---|---|---|---|---|
| Text glyphs (monochrome) | Coverage mask sampled from the one atlas with a nearest sampler, tinted by `fg_color` mixed with `alt_color` by `mix_value`, `foreground_text_hsb` applied, output converted to sRGB (`glyph-frag.glsl:145-160`) | Same algorithm reimplemented per-pixel; `has_color==0.0` branch tints `fg` by atlas alpha and applies `foreground_text_hsb`, then linear→sRGB (`purecpu.rs:412-429`, `455-460`) | needs-measurement | | |
| Inline image: sixel | Image is decoded into the same glyph atlas; each covered cell gets a quad whose texture rect is the cell's fraction of the sprite and whose destination is exactly one cell — the GPU rescales source to destination (`render/mod.rs:468-518`) | Textured quads are blitted 1:1: `blit_w = tex_w.min(dest_w)`, `blit_h = tex_h.min(dest_h)`, source stepped in lockstep with destination. No scaling exists in the rasteriser, so whenever the image's per-cell source region is not exactly `cell_width x cell_height` the image is cropped (or leaves gaps), never resampled (`purecpu.rs:344-360`) | needs-measurement | degraded | Regenerate native-size sub-case: `FUZZ=1 ./compare-case.sh images-native "$PWD/corpus/images-native.sh"` — `out/images-native-gl.png` vs `out/images-native-cpu.png`, region crop `64x64+0+55`: AE=0, PAE=0 (bit-identical parity at native size, reproduced in fix round 2). Two larger sizes, both measured through `compare-case.sh` (not a side probe), both fail the plan's `PAE<=257` gate: **(1)** 300x300 sixel, still requested at native size (`FUZZ=1 ./compare-case.sh images "$PWD/corpus/images.sh"`, region crop `300x300+0+341`): AE=3300, **PAE=1542 (6 LSB, fails gate)** — not a crop, a one-row displacement of the gradient band boundary (row 135: `gl srgb(140,0,114)` vs `cpu srgb(142,0,112)`, i.e. GL steps to the next band one scan row earlier than PureCpu; absent at 64x64, so it is source-stepping rounding that only shows up once the image spans enough rows for a rounding remainder to accumulate). **(2)** `FUZZ=1 PARITY_SETTLE_WARMUP=20 ./compare-case.sh wide-sixel "$PWD/corpus/wide-sixel.sh"` (17000x64 gradient — `PARITY_SETTLE_WARMUP` is required, not optional prose: this content takes long enough to reach the framebuffer that `capture_settled` can declare victory on two identical *blank* captures before either backend has drawn anything, silently reproducing the noise floor instead of the real result — `compare-case.sh` fix round 3 now echoes the warmup value used and warns if a capture comes back `Gray` colorspace, so a run missing this var announces itself instead of looking like `parity`): whole-frame AE=64932, body PAE=13107 (51 LSB, fails gate — **but this figure is multi-causal: it is dominated by an unexplained text-antialiasing difference in the "WIDE SIXEL:"/"END" label rows, not the image itself; see Task 4 fix-round-2/3 reports for the breakdown. The strip numbers below carry the verdict, not this one**); the visible 1000x64 strip alone (region crop `1000x64+0+55`) is **AE=64000 of 64000 pixels — every pixel in the strip differs** — by a uniform ~2-3/255 colour offset (`gl srgb(122,0,132)` vs `cpu srgb(124,0,130)` sampled across the whole strip width), consistent with GL sampling a `paint.rs:72-76` `AllowImage::Scale(2)`-downscaled sprite (confirmed via GL's own log: `Not enough texture space (... max 16384); will retry render with Scale(2)`; PureCpu's log has no such line, since `renderstate.rs:78-118`'s `Self::PureCpu` arm has no texture-size cap and never enters this fallback) while PureCpu blits the full-resolution source. So sixel is degraded from at least two independent, unrelated mechanisms once an image departs from a small native size: row-stepping rounding (visible already at 300x300) and the `AllowImage::Scale` asymmetry (visible once the atlas must grow past `GL_MAX_TEXTURE_SIZE`). See Task 4 fix-round-2/3 reports |
| Inline image: iTerm2 OSC 1337 | Same path as sixel (`populate_image_quad`, `render/mod.rs:441-521`); `AllowImage::Scale(n)` may store a downscaled sprite in the atlas, widening the source/destination ratio | Same 1:1 crop as sixel; a downscaled sprite makes the mismatch larger | needs-measurement | degraded | Regenerate native-size sub-case: `FUZZ=1 ./compare-case.sh images-native "$PWD/corpus/images-native.sh"` — `out/images-native-gl.png` vs `out/images-native-cpu.png`, region crop `64x64+0+143`: AE=0, PAE=0 (bit-identical) — parity only holds when the requested display size equals the sprite's native size. Requested at non-native size (200x132px, and again at 20x4 cells): `FUZZ=1 ./compare-case.sh images "$PWD/corpus/images.sh"`, region crop `200x200+0+77` (merged view of both non-native blocks after terminal scroll): AE=39360, PAE=61423 (0.937 — near-maximal). Visual: OpenGL draws a solid stretched gradient block; PureCpu draws a dotted grid of tiny cropped fragments, one small top-left tile per covered cell, background showing through the rest of every cell — exactly the `purecpu.rs:346-347` crop-instead-of-scale defect at full severity, reachable from an ordinary escape sequence. (The "cpu ink stddev is ~59% of gl's" claim in fix-round-1 did not survive splitting the merged region into its two sub-blocks and should not be cited as supporting evidence — the AE/PAE numbers and the visual pattern carry the verdict.) See Task 4 fix-round-1 and fix-round-2 reports |
| Inline image: animated GIF | Frame advance happens inside `cached_image`, i.e. as a side effect of running the paint pass; the GPU path repaints on the scheduled animation timer so frames advance (`glyphcache.rs:929-970`, `render/mod.rs:468-473`) | `do_paint_purecpu` returns before `paint_impl` whenever no terminal row is dirty and no cursor-blink transition occurred, so the paint pass — and therefore frame advance — does not run for a still screen (`termwindow/mod.rs:1436-1447`). Additionally the same 1:1 crop applies to each frame | needs-measurement | degraded | The corpus GIF loops forever (`identify -verbose` reports `Iterations: 0`; `glyphcache.rs:955-960` wraps frame index unconditionally, so loop metadata doesn't matter — it animates for as long as paints happen). Measured with `sample-case.sh`, which samples a live region repeatedly instead of waiting to settle: `./sample-case.sh gifanim "$PWD/corpus/gif-loop.sh" "4x4+14+69" 12 0.5`. Over 12 samples at 0.5s intervals (6s): `gl` cycles `srgb(255,255,0)`/`srgb(0,128,0)` (yellow/green, both GIF frames observed); `cpu` reports `srgb(0,128,0)` for all 12 samples — frozen on one frame for the entire window. Confirms the `termwindow/mod.rs:1436-1447` idle-skip prediction directly: PureCpu does not advance GIF frames on an otherwise-idle screen. See Task 4 fix-round-1 report |
| Fancy tab bar | Elements rendered through `box_model`; glyph quads are sized `texture.coords.size * glyph.scale`, so a fallback/bitmap glyph with `scale != 1` is rescaled by the GPU (`box_model.rs:889-914`) | Solid-colour and 1:1 glyph quads are correct; any `glyph.scale != 1` quad is cropped rather than scaled (`purecpu.rs:346-347`) | needs-measurement | degraded | Task 3's finding reproduces and is characterised further here, at case scale (3 tabs + 1 active, produced by `SPAWN_TABS=true`). **Corpus is hermetic** (review round 1, I4 — `gen-config.sh`'s `gui-startup` handler now forwards `args = cmd.args` to every spawn/split, so every tab runs `corpus/chrome.sh`'s own deterministic text; tab titles read `1: sleep`/`2: sleep`/`3: sleep`, not a login shell). Regenerate: `FANCY=true SPAWN_TABS=true FUZZ=1 ./compare-case.sh chrome "$PWD/corpus/chrome.sh"` (`tools/purecpu-parity/`) — `out/chrome-gl.png` vs `out/chrome-cpu.png`. Whole-frame AE=2070 (fuzz 0) / 674 (fuzz 1%); body PAE (crop `1000x661+0+32`) = 257, exactly the Task 3 gate boundary, confined entirely to the corpus's own two printed text lines (crop `1000x64+0+32`: AE=1269, PAE=257; the remaining 597 body rows, crop `1000x597+0+96`: AE=0) — ordinary text-glyph noise, not a chrome finding. (`compare-case.sh` prints a `Gray colorspace` warning for both captures; this corpus's own text has no colour content by design, so Gray is the correct, expected colourspace here, not a sign of a blank capture — confirmed by inspecting both PNGs directly, which show the corpus banner and prompt-free panes on both backends.) Tab-strip crop `1000x32+0+0`: AE=801, no fuzz value clears it (matches Task 3: this region has no established noise floor and is graded by explicit shift/no-shift pixel evidence below, not by a fuzz threshold — see "Grading basis" note above the table). **The strip has three behaviour classes, not two** (column-by-column classification of `chrome-{gl,cpu}.png` columns x=0-299, y=0-31, each column tested against the same column and the left-neighbour column): **164 columns identical** (`dx=0` gives `AE=0`), **37 columns are a clean 1px-left shift** (`dx=0` nonzero, the left-shifted `dx=1` comparison gives `AE=0`), and **99 columns are neither** — concentrated at x≈94-283 (tab 2 and 3's titles, separators, close icons), where shifting barely helps (crop `196x32+94+0`: AE=637 at `dx=0` vs 561 at `dx=1`) and the peak per-channel error is 77/255 either way (`compare -metric PAE` on the same crop: 19789, i.e. 19789/257 ≈ 77 LSB) — too large to be antialiasing noise, but not a clean integer shift either. **Confirmed shift, active tab title**: single-row crops `50x1+20+12` and `50x1+20+14` (the "sleep" letters) are `AE=0` when compared against `out/chrome-cpu.png` shifted one column left (`-crop 50x1+19+12`/`+19+14`) — bit-exact. **Unshifted, active tab**: digit `1` + `:` (crop `20x16+0+8`) and the active tab's close `x` icon (crop `20x16+72+8`) are both `AE=0` at `dx=0`, and both rounded corners (`10x10+0+0`, `10x10+79+0`) are `AE=0`/`PAE=0`. **Neither, inactive tab 2's close icon** (crop `16x16+158+8`): `AE=78` at `dx=0`, `AE=16` at `dx=1` — not bit-exact at either alignment. Row-by-row (`16x1+158+y`) shows why the block crop is ambiguous: rows y=8,10,12 are identical (`dx=0` `AE=0`), row y=14 is a clean 1px shift (`dx=1` `AE=0`), and rows y=18,20,22 are neither (small residuals, `AE` 2-10, at both alignments) — a mix of unshifted, cleanly-shifted, and non-integer-divergent rows within one glyph, not a single displacement. So the strip's difference is genuinely two things: a confirmed, bit-exact 1px-left shift on some `box_model` glyph runs (title-text letters on the active tab, some rows of some inactive-tab icons), and a separate, larger (~77/255), non-integer positioning divergence concentrated on inactive-tab glyph runs that a 1px shift does not resolve. Both are `degraded`; neither is folded into a threshold. Retro tab bar (below) reproduces neither, confirming both are specific to the fancy tab bar's `box_model` glyph-quad path, not to text rendering in general. |
| Retro tab bar | Rendered as ordinary cells plus `util_sprites.white_space` / `filled_box`, both allocated in the same atlas (`render/tab_bar.rs:49-50`, `utilsprites.rs:159-162`) | Same quads; solid-colour quads take the `IS_SOLID_COLOR` fill path, glyphs the 1:1 path (`purecpu.rs:300-341`) | needs-measurement | parity | **Caveat: this is the one row graded on a threshold borrowed from the body noise floor (with stated justification below), not on the same threshold-free footing as Window buttons/Rounded corners/Split dividers, which are bit-identical (`AE=0`/`PAE=0`) and need no threshold at all — parity here is "within 1 LSB", not "no difference".** `FANCY=false SPAWN_TABS=true FUZZ=1 ./compare-case.sh chrome-retro "$PWD/corpus/chrome.sh"` (`tools/purecpu-parity/`) — `out/chrome-retro-gl.png` vs `out/chrome-retro-cpu.png` (hermetic corpus, as above). Whole-frame AE=1427 (fuzz 0) / **0 (fuzz 1%)** — the fancy-tab-bar shift/divergence defects do not reproduce here (a genuine defect is never absorbed by fuzz; this is a different, unaffected render path, confirmed separately below). Tab-strip crop `1000x32+0+0`: AE=281 raw, PAE=257 (exactly 1 LSB, the Task 3 body-gate boundary value); narrowed to the retro bar's actual 21px height (crop `1000x21+0+0`): AE=158, PAE=257 — ordinary antialiasing noise, same order of magnitude as the plain-text noise floor Task 3 established for the terminal body, and it clears at fuzz 1% (AE=0). The retro tab bar draws tab titles as ordinary monospace terminal cells rather than `box_model` glyph runs with float-computed positions, which is consistent with it not sharing the fancy bar's defects. |
| Window buttons | Built as `box_model` Elements (polys + glyphs) drawn from the same atlas (`render/window_buttons.rs`) | Same; poly sprites are generated at their final pixel size so the 1:1 blit is exact | needs-measurement | parity | Window buttons only render in `box_model`'s fancy-tab-bar path when `window_decorations` includes `INTEGRATED_BUTTONS` (`fancy_tab_bar.rs:314,372`); the harness's `gen-config.sh` did not previously expose this, so it was extended with a `WINDOW_DECORATIONS` env var (default unset, so every existing case reproduces byte-for-byte unchanged — verified with `diff` against the pre-Task-5 generator's output for every front-end/FANCY combination). **The regeneration command needs a literal `\|`, which a Markdown table cell cannot hold — see the fenced command directly below the table (review round 1, C1): a table-cell copy of this command previously required a backslash-escaped pipe that reached the shell intact and made `gen-config.sh` emit invalid Lua, silently dropping the whole config (including `front_end`) to defaults and reporting a false, self-comparing `AE=0`/`PAE=0`.** `out/chrome-buttons-gl.png` vs `out/chrome-buttons-cpu.png` (hermetic corpus content in both panes) show minimise/maximise/close icons rendered top-right. Whole-frame AE=2070/674/257 (fuzz 0 / fuzz 1% / body PAE), identical to the Fancy tab bar row's numbers since it's the same corpus and window layout plus buttons. Region crop `140x32+860+0` (the button cluster): AE=0, PAE=0 — bit-identical. This case could have shown the same class of defect as the tab-title glyphs (the buttons are also `box_model` polys/glyphs) but did not, so the parity verdict is not a structural inability to detect a defect. |
| Rounded corners | `poly_quad` rasterises the corner poly into the atlas at exactly the requested corner size and marks it `IS_GRAY_SCALE` (`render/mod.rs:290-328`, `box_model.rs:1038-1092`) | `has_color==4.0` branch: `fg` tinted by atlas alpha; source and destination sizes are equal by construction, so the 1:1 blit is exact (`purecpu.rs:406-411`) | needs-measurement | parity | Exercised in every fancy-tab-bar capture above (every fancy tab, active and inactive, has `border_corners` set — `fancy_tab_bar.rs:183-195,229-241`); only the active tab's two corners were actually measured, since inactive tabs share the bar background and their corners are not visually separable from it. Top-left and top-right corners of the active tab, `out/chrome-gl.png` vs `out/chrome-cpu.png`, crops `10x10+0+0` and `10x10+79+0` (re-located after the hermetic-corpus fix changed tab-title text from "bash" to "sleep", which widened the active tab and moved its right edge): AE=0, PAE=0 — bit-identical, unaffected by the adjacent title-text shift. |
| Split dividers | `filled_rectangle` → `IS_SOLID_COLOR` quad, texture ignored by the shader (`render/split.rs:32,55`, `render/mod.rs:266-287`, `glyph-frag.glsl:115-118`) | `IS_SOLID_COLOR` fill path, colour-only, size-independent (`purecpu.rs:300-341`) | needs-measurement | parity | `SPAWN_TABS=true`'s `gui-startup` handler (`tools/purecpu-parity/gen-config.sh`) splits the first tab right at 50%; that handler unconditionally activates the last-spawned tab (tab 3) once done, which put the split off-screen for every capture until fixed by adding an explicit `tab:activate()` at the end of the handler to reselect the split tab (see `gen-config.sh` diff — this is what "gui-startup does not exercise what the row claims" means in practice: the event fired, but the config it produced could not show a divider in any capture without this fix). With the fix (and the later hermetic-corpus fix, review round 1 I4, which does not move the divider — it is independent of tab-title text), `out/chrome-gl.png` vs `out/chrome-cpu.png` show the vertical divider at x=495; column crop `1x661+495+32` (full body height): AE=0, PAE=0 — bit-identical. |
| Cursor (static) | Block/bar/underline drawn as a poly or solid quad at exact pixel size | Same quads; 1:1 blit is exact | needs-measurement | | |
| Cursor blink easing | `ColorEase::intensity_continuous()` is evaluated on the CPU every animation frame and delivered as `fg_color_mix` / `cursor_border_mix`, giving a continuous eased fade (`render/mod.rs:680-696`) | `do_paint_purecpu` quantises the eased intensity to a boolean (`i < 0.5`) and only repaints on a phase change or cycle end; between transitions the paint pass is skipped entirely (`termwindow/mod.rs:1380-1412`, `1436-1447`). The rasteriser itself handles `mix_value` correctly (`purecpu.rs:252-255`), so the loss is in the repaint scheduling, not the blend | by-reading | degraded | `termwindow/mod.rs:1387-1394` (quantisation to on/off), `termwindow/mod.rs:1440-1447` (paint skipped between transitions) |
| Blinking text attribute | `blink_state` / `rapid_blink_state` intensity is applied CPU-side to the cell foreground each paint, and `update_next_frame_time` schedules the next frame (`render/screen_line.rs:797-825`) | The colour computation is identical when a paint happens, but the dirty-rect pass only ever marks changed terminal rows and the cursor cell; nothing marks cells carrying the blink attribute, so the early exit at `termwindow/mod.rs:1440-1447` suppresses the repaint on an otherwise idle screen | needs-measurement | | |
| Visual bell | `get_intensity_if_bell_target_ringing` returns a continuously varying mix that is applied to the cell/cursor background each paint (`render/mod.rs:233-258`, `535-565`) | Same colour computation, but a ringing bell changes no line seqno and sets no dirty rect, so the same early exit applies (`termwindow/mod.rs:1436-1447`) | needs-measurement | | |
| Double-width / double-height lines (DECDWL/DECDHL) | Glyph destination is scaled by `width_scale` / `height_scale` while the atlas source stays at base size; the GPU rescales (`render/screen_line.rs:50-63`, `632-647`) | 1:1 blit clips to the smaller of source and destination, so the glyph is drawn at base size in the top-left of the enlarged cell (`purecpu.rs:346-347`) | by-reading | degraded | `render/screen_line.rs:636-647` vs `purecpu.rs:346-347` |
| Scaled fallback / bitmap glyphs (`glyph.scale != 1`) | Destination is `sprite size * glyph.scale`; the GPU rescales (`render/screen_line.rs:380-393`, `box_model.rs:902-913`). `scale` is set below 1 for oversized or unscaled bitmap glyphs such as colour-emoji fonts (`glyphcache.rs:749-798`) | 1:1 blit crops the sprite to the destination rectangle instead of downscaling | by-reading | degraded | `glyphcache.rs:749-798`, `box_model.rs:902-913` vs `purecpu.rs:346-347` |
| Subpixel antialiased text (`freetype_render_target = "HorizontalLcd"`) | Layer 1 is drawn with dual-source blending using the per-channel `colorMask` output (`render/draw.rs:172-193`, `244`, `260-266`; `glyph-frag.glsl:14-17`, `147-152`) | The rasteriser has a single scalar alpha and no per-channel mask; `blend_over` blends one alpha for all channels (`purecpu.rs:529-543`) | by-reading | degraded | `render/draw.rs:181-193` vs `purecpu.rs:529-543` |
| Window background image sampling filter | `IS_BG_IMAGE` quads are sampled through `atlas_linear_sampler` (bilinear) rather than the nearest sampler (`glyph-frag.glsl:119-127`, `render/draw.rs:220-223`) | `has_color==2.0` branch does the same 1:1 nearest crop as every other textured quad (`purecpu.rs:394-399`) | out-of-scope | known gap | user decision, not investigated |
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
real structural gap is one level down, in sampling: the GPU interpolates texture
coordinates across each quad and therefore rescales whenever source and
destination sizes differ, while `call_draw_purecpu` blits 1:1 and clips
(`purecpu.rs:344-360`). Content whose quads rely on that rescale — inline
images, double-width/height lines, scaled fallback glyphs — is drawn from the
right texture but at the wrong size.

The second structural gap is not in the rasteriser at all but in
`do_paint_purecpu`'s idle skip (`termwindow/mod.rs:1436-1447`): time-driven
animation that changes no line seqno and marks no dirty rect never reaches the
paint pass.

## Grading basis for chrome pixels (Task 5)

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
memory (`renderstate.rs:110-115`), and quads are rasterised by hand into a
`Vec<u8>` framebuffer that is pushed to the window with
`present_software_frame_region`. What PureCpu adds over `Software` is the
removal of the GL/Mesa stack and the ability to repaint only dirty regions; what
it gives up is the shader, the samplers, and per-frame animation repaints.
