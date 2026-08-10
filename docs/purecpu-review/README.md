# PureCpu vs. the GPU backend: where the software renderer falls short

This is the summary of a review of your fork's two large unreviewed changes on
top of upstream `e723cf5` — the pure-Rust font stack and the `PureCpu` software
renderer. The question it answers is narrow and practical: **if you run wezterm
on a GPU-less machine with `front_end = "PureCpu"`, what do you lose compared to
`front_end = "OpenGL"`?**

Scope, by your own decision: this fork only, Linux/X11 only, no
macOS/Windows/Wayland, no upstreamability concerns, background image and
transparency out of scope. **This review produces documents, not fixes.**
Nothing in the tree was changed except these documents and the test harness that
produced them; Rust source was read-only throughout.

Two detail documents sit behind this page, and every claim here is traceable to
a row or a finding in one of them:

- **[parity-matrix.md](parity-matrix.md)** — 21 feature rows, each with a
  verdict. The 16 that were investigated carry the command and the numbers
  behind them; the 5 out-of-scope rows carry a reason instead.
- **[findings.md](findings.md)** — 18 defects found by reading the fork's own
  patch, ranked, with evidence class stated per finding.
- [noise-floor.md](noise-floor.md) — the calibration that makes the pixel
  comparisons trustworthy. Worth reading only if you want to check the method.

---

## The one thing to understand first

**PureCpu is not a separate rendering pipeline.** It consumes the same layer and
quad stream, from the same glyph atlas, as the GPU path — the hypothesis that it
draws from a different texture and therefore cannot see some content classes was
tested and is false (the GPU path has only one texture too; see
`parity-matrix.md`, "Settled by reading: the single-texture question").

So the gaps are not missing features. They are places where the hand-written
rasteriser cannot reproduce what the GPU's fixed function does for free. Nearly
every confirmed rendering defect traces back to **three** root causes:

| # | Root cause | Where | What it costs you |
|---|---|---|---|
| 1 | **The blit is 1:1 and crops; there is no resampler at all** | `purecpu.rs:346` | Anything whose quad is drawn at a size different from its atlas sprite: non-native-size inline images, double-width/height lines, scaled fallback and bitmap glyphs |
| 2 | **The idle skip returns before the paint pass when no line is dirty** | `termwindow/mod.rs:1436-1447` | Everything time-driven: GIF frames, blinking text, the visual bell, cursor blink |
| 3 | **Cursor-blink detection tests the raw, unresolved cursor shape** | `termwindow/mod.rs:1383` vs `render/mod.rs:604-611` | `default_cursor_style = "Blinking*"` — the documented way to turn blinking on — is inert |

The single most severe finding is separate from all three, and it is not a
rendering issue at all. See below.

**A related question the matrix settles in passing: `Software` is not a third
option.** `FrontEndSelection::Software` is consumed in exactly one place — it
makes `prefer_swrast()` true, which only steers EGL/WGL configuration selection
towards a software rasteriser. Everything above that (glium context, shaders,
quad generation, `call_draw_glium`) is the `OpenGL` front end unchanged, so on a
GPU-less machine `Software` is behaviourally identical to `OpenGL`. What
`PureCpu` adds over it is the removal of the GL/Mesa stack and the ability to
repaint only dirty regions; what it gives up is the shader, the samplers, and
per-frame animation repaints — which is precisely the list above.

---

## Start here: the one finding that is not cosmetic

**C1 — the atlas has no size ceiling. A ~1.5 KB sixel escape sequence makes
PureCpu allocate 4 GiB.** ([findings.md](findings.md), C1)

`RenderContext::allocate_texture_atlas`'s `Glium` arm checks the requested side
against `caps.max_texture_size` and bails, which is what sends the GPU path into
its `AllowImage::Scale(2)` downscale-and-retry fallback. **The `PureCpu` arm has
no cap and no fallible path**, so it just allocates. The atlas side is chosen
from the decoded pixel width of an inline image — i.e. from content, which on a
terminal means from anything that reaches the tty.

Measured, not extrapolated: a 17000x64 gradient sixel is **1527 bytes** on the
wire and drives the atlas to 32768x32768, which is **4096 MiB — touched, not
lazily reserved** (`Atlas::new` builds a second full-size image and writes the
whole rect). Peak RSS came back at 4170 MiB, stable across runs and reproduced
independently. Amplification from input to allocation is about 2.8e6. The next
doubling selects 65536, i.e. **16 GiB**; that case was deliberately *not* run,
because the machine has 25 GiB and other people on it.

**This arm is fork-introduced, not inherited.** `git show
e723cf5:wezterm-gui/src/renderstate.rs` has only `Glium` and `WebGpu` arms — there
is no upstream software arm the missing ceiling could have come from.

Fix direction: give the `PureCpu` arm a ceiling and a `bail!`, so it reaches the
same `AllowImage::Scale` fallback the `Glium` arm already reaches.

---

## Headline parity result

21 features, graded against **their own named feature** — an animated GIF that
draws correctly but never advances is `degraded` (the image is delivered, only
its motion is lost); cursor blink is `missing` because the blink *is* the
feature.

| Verdict | Count | |
|---|---|---|
| `parity` | 6 | all measured; 4 bit-identical on the element measured, 2 within 1 LSB |
| `degraded` | 7 | 4 measured, 3 concluded by reading |
| `missing` | 3 | all measured |
| `known gap` | 5 | out of scope, not measured |

**What works.** Ordinary terminal text — the thing you actually look at all day
— is sound. Monochrome text is within one 8-bit step everywhere, with *zero*
difference on background pixels (body `PAE = 257`, body `AE = 0` from 0.5% fuzz
up). Static cursors in all three shapes (block, bar, underline), window buttons,
rounded corners and split dividers are **bit-identical** on a crop tight to the
element (`AE = 0`, `PAE = 0`) — no threshold question to answer, nothing differs.
Inline images at their **native** size are bit-identical too. The retro tab bar
is `parity` within 1 LSB rather than bit-identical, and it is the one row graded
on a threshold borrowed from the body noise floor; the matrix says so in its
Evidence cell rather than letting it read as equivalent to the others.

**What is degraded.**

- **Inline images**, both protocols, but the two rows were measured on different
  content and the evidence does not transfer between them:
  - **iTerm2 OSC 1337 at a non-native size** is the severe one, and it is
    measured directly. Requested at 200x132 px, and again at 20x4 cells, PureCpu
    draws a *dotted grid of tiny cropped fragments* — one top-left tile per
    covered cell, background showing through the rest of every cell — where
    OpenGL draws a solid stretched block. Over a crop covering both non-native
    blocks: `AE = 39360`, `PAE = 61423` (0.937, near-maximal). Reachable from an
    ordinary escape sequence.
  - **Sixel** is `degraded` from **two independent mechanisms**, neither of which
    is the dotted grid above — no sixel was ever captured at a non-native
    requested size, so that failure mode is *predicted* for sixel via the shared
    `populate_image_quad` path, not shown. What *was* measured: (a) at 300x300,
    still requested at its native size, source-stepping rounding displaces the
    gradient band boundary by one scan row — `AE = 3300`, `PAE = 1542` (6 LSB),
    which fails the gate; and (b) at 17000x64, once the atlas must grow past
    `GL_MAX_TEXTURE_SIZE`, the GPU path downscales via `AllowImage::Scale(2)` and
    PureCpu does not, so **every pixel in the visible strip differs** (`AE =
    64000` of 64000) by a uniform ~2-3/255.
  - Both protocols are bit-identical at native size (`AE = 0`, `PAE = 0`).
- **Animated GIFs** never advance a frame on an otherwise-idle screen. Over 12
  samples across 6 s, GL cycled both frames; PureCpu reported the same frame all
  12 times.
- **The fancy tab bar** — two distinct classes, neither absorbed into a
  threshold: 37 columns are a bit-exact 1 px left shift, and 99 columns are a
  larger (~77/255) non-integer positioning divergence that shifting does not fix.
  The retro tab bar reproduces neither, which is what pins it to the `box_model`
  glyph-quad path rather than to text rendering generally.
- **Double-width/height lines, scaled fallback and bitmap glyphs (including
  colour emoji), and subpixel-antialiased text** (`freetype_render_target =
  "HorizontalLcd"` — the rasteriser has one scalar alpha and no per-channel
  mask). These three are **by reading, not measured** — see the confidence note
  below.

**What is missing.**

- **Cursor blink** — but read the scope, because it is two different answers.
  Config-driven blink (`default_cursor_style = "BlinkingBlock"`, the documented
  way) **never starts at all**: sampled 16 times over 6.4 s, PureCpu returned
  `gray(224)` every single time while GL swept the full 17-222 range. That is
  root cause 3. Separately, an application that drives the shape itself with
  DECSCUSR *does* blink — but that path is `degraded`, not fine: quantised to
  ~2 paints/cycle by design, dwelling on two plateaux with a floor of
  `gray(159)`, where GL sweeps continuously to within a few LSB of background.
- **Blinking text (SGR 5)** — frozen, and frozen *invisible*, deterministically:
  the eased intensity starts at 0 (`fg = bg`) on the one paint that happens when
  the window settles, and the idle skip suppresses every paint after it, forever.
  The word is simply blank space. There is no rescheduling mechanism for text
  blink at all, so this is a structural absence rather than a resolution gap.
- **The visual bell** never visibly rings. The `Alert::Bell` handler invalidates
  the window but pushes no dirty rect, so the idle skip still takes the early
  exit and the background-mix computation never runs.

---

## How much to trust each verdict

The review distinguishes these deliberately, and this page does not flatten them:

- **Measured** — a command was run against the built binary and the numbers are
  in the Evidence cell. **13 of 21 rows**, including every `parity` and every
  `missing`. Five of them state "reproduced from a clean shell" in the matrix
  (the four cursor/animation rows plus text glyphs); several others were
  re-run independently by a second reviewer during the review rounds.
- **By reading** — the code path is unambiguous but no runtime trigger was
  demonstrated. 3 rows. Two of them — double-width/height lines and scaled
  fallback/bitmap glyphs — are consequences of root cause 1, whose mechanism
  *was* measured on images. The third, subpixel-antialiased text, is a
  **separate mechanism** that was never measured at all: dual-source blending
  with a per-channel `colorMask` on the GPU side versus a single scalar alpha in
  `blend_over` on PureCpu's. Note also that Task 1 predicted `degraded` for
  images by reading, and measurement then showed images are bit-identical at
  native size. **Reading correctly identified the mechanism and got the
  conditions wrong.** Treat all three as well-founded predictions, not results —
  and the subpixel row as the weakest of them, since not even its mechanism has
  been exercised.
- In `findings.md`, the same distinction is enforced by a written demotion rule:
  a defect whose trigger was not reproduced is recorded one class below its
  defect class. That is why M3/M4/M5 sit at Medium.

Two verdicts changed during the review as a direct result of measuring, which is
the best evidence that the instrument was not merely confirming its own
predictions: inline images went from a predicted `degraded` to measured `parity`
at native size (and then to `degraded` again on a corpus that could actually
trigger the defect), and cursor blink went from a by-reading `degraded` to a
measured `missing`.

---

## The rest of findings.md, in severity order

`findings.md` presents findings in id order within each class, which is *not*
severity order inside Medium. The order below is the severity ladder the
document itself defines, applied consistently.

**Important — content silently not drawn in an ordinary configuration. Neither
of these was reachable by the parity matrix at all**, for a reason worth
internalising: every case in the harness put the content under test in a
**single full-width pane**, where `pos.top` and `pos.left` are both zero and the
active pane is the only one with live output. The corpus was blind along an axis
nobody thought to vary — not along an axis the tool could not see:

- **I1** — dirty tracking consults only the **active** pane, so output in any
  other pane of a split is never repainted until something forces a full repaint.
  Measured.
- **I2** — dirty-rect geometry omits the pane's `pos.top`/`pos.left`, so even the
  *active* pane freezes when it is not at the window's top-left. Measured on both
  axes.

If you use splits, I1 and I2 are probably what you would notice first in daily
use — ahead of anything in the parity matrix.

**I3, I4, I5** are root causes 2, 3 and 1 stated as findings, with the
matrix rows they explain cross-referenced.

**Medium, but read these three first within their class** — they are
defect-class Critical (process aborts reachable from font data), recorded Medium
only because no crafted font was built to fire them (`fontTools` was not
available on the machine). They rank below I1/I2 under the document's own
demotion rule precisely because their triggers were not reproduced, which is a
statement about the evidence, not about how bad they would be:

- **M3** — unconditional panic in the empty-path fallback of the COLR glyph
  rasteriser. The fallback is unfinishable and panics whenever reached; tiny-skia
  semantics were verified by running a standalone program. What is *not* shown is
  the font that reaches it.
- **M4** — `cpal.color_record_indices()[0]` panics on a CPAL table with zero
  palettes.
- **M5** — `unitsPerEm = 0` divides by zero in both the shaper and the rasteriser.

**M1** explains the tab bar's bit-exact 1 px shift: destination coordinates are
truncated (`as i32`) instead of rounded. Fix direction is one call:
`(tl.position[0] + half_w).round() as i32`. **M2** documents that overlapping
dirty rects composite the same pixel twice; the model is strongly supported
(across all 87 label-row pixels differing by more than 1 LSB, the double-composite
prediction lands within 1.91 LSB, mean residual -0.05) but its attribution to a
specific rect overlap was **tested twice and eliminated** — the document says so
rather than dressing it up.

**M6, M7, L1-L5** are latent edge cases, performance and hygiene. `findings.md`
also carries a **"Considered and rejected"** section of 7 non-findings, recorded
so nobody re-derives them — including the triple-buffer misreading and several
apparent panic sites that are in fact guarded.

---

## What this review does *not* cover

Stated plainly, because a clean-looking matrix is misleading without it.

**Out of scope by your decision, never measured** — 5 `known gap` rows:
`window_background_image`, `window_background_opacity`,
`text_background_opacity`, background blur / HSB tint, and the window background
image sampling filter. The last of these *was* read (the GPU samples background
image quads bilinearly, PureCpu does the same nearest 1:1 crop as every other
quad, so the same crop-instead-of-scale defect is predicted) but no capture was
taken, so treat it as an untested prediction. The other four were not looked at
at all.

**Platforms.** Linux/X11 only. macOS, Windows and Wayland were not examined in
any way.

**Deliberately not run.** The 65536 atlas case (16 GiB on a shared 25 GiB
machine — the arithmetic does not need a demonstration that could take out other
people's work), and crafted malformed fonts for M3/M4/M5.

**Declared unmeasured inside rows that were otherwise measured** (all four now
recorded in the matrix itself, under "Grading basis for animation rows"):
`VisualBellTarget::CursorColor` — the bell row samples an empty-background
pixel, so only the default `BackgroundColor` target was exercised; rapid blink
(SGR 6 / `text_blink_rate_rapid`), as distinct from the SGR 5 path the blinking
-text row measures; any interaction between the three animations running at
once; and the DECSCUSR cursor-blink sub-finding, whose numbers come from a
hand-written config because `gen-config.sh` correctly refuses the combination
that probe requires (its regeneration block is fenced in the matrix and does
run). Each row's verdict covers what its Evidence cell actually sampled.

**Method limits.** Three worth knowing:

- **Every harness case ran the content under test in a single full-width pane.**
  That is what made I1 and I2 invisible to the matrix — not that captures were
  settled, since Task 4 added a non-settling sampler that the animation rows and
  the GIF row all use. With one pane, `pos.top`/`pos.left` are zero and the
  active pane is the only one producing output, so both defects are identically
  silent. Any defect that needs a split would have been missed the same way.
- **Time-driven rows cannot be diffed across backends at all**: each backend's
  blink/bell/animation phase runs off its own wall clock, so a cross-backend
  pixel diff of two captures "at the same moment" would measure phase offset,
  not a defect. The three animation rows (cursor blink, blinking text, visual
  bell) and the animated-GIF row are therefore graded by sampling a region
  repeatedly *within* each backend and asking whether the value changes over
  time. A flat sequence against a varying one is the finding.
- The tab strip has **no established noise floor** (Task 3 stopped there
  deliberately rather than inventing one), so chrome pixels are graded on
  explicit per-region pixel evidence, never on a threshold. `FUZZ` was never
  raised to make a case pass.

**The existing test suite does not help here.** `cargo test -p wezterm-gui -p
wezterm-font` passes (29 + 1), but `purecpu.rs`'s own unit tests do not touch the
rasterising path at all — so a green suite says nothing about any of the above.

---

## Re-running the harness

Everything lives in [`tools/purecpu-parity/`](../../tools/purecpu-parity/). The
binary under test is `/scratch/oetiker/wezterm-builds/wezterm-gui-rebased`
(override with `WEZTERM_BIN`). `out/` is gitignored — every PNG referenced in
the detail documents is a run artifact, regenerated by the commands printed
beside the numbers they produced.

**Prerequisite.** `lib.sh` requires `PARITY_XAUTH`, and `PARITY_DISPLAY` if you
are not on `:20`. It fails closed via `${VAR:?}`, so a missing value can only
produce a loud error, never a quietly wrong number.

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
export PARITY_XAUTH=<path to the :20 Xauthority>   # PARITY_DISPLAY=:20 by default
```

If display `:20` is gone, recreate it (`Xvnc :20 -depth 24 -geometry 1280x1024`
plus a window manager — the exact invocation is in the plan's "Environment
setup" section) and confirm with
`DISPLAY=:20 XAUTHORITY=$PARITY_XAUTH xdpyinfo | grep dimensions`.

```bash
# Calibration: re-establish the noise floor and the focus-mismatch evidence.
./focus-probe.sh
./calibrate.sh out/focus-B-gl.png out/focus-A-cpu.png   # focus-matched -> exit 0

# Static comparison: capture both backends on one corpus and diff them.
FUZZ=1 ./compare-case.sh <case-name> "$PWD/corpus/<corpus>.sh"

# Time-driven content, which cannot be diffed across backends: sample one
# region repeatedly, per backend, and read the colour sequence.
./sample-case.sh <case-name> "$PWD/corpus/<corpus>.sh" <crop-geometry> <count> <interval>
```

**The two comparison drivers launch one backend at a time, and that is not
optional.** Capturing two concurrently-open windows leaves one unfocused, and
wezterm draws a hollow cursor when unfocused versus a solid block when focused —
injecting a full character cell of difference that survives 12% fuzz and looks
exactly like the whole-cell defect class the comparison exists to detect. This
actually happened, and it is why `calibrate.sh` exists. `focus-probe.sh` is the
one deliberate exception: it launches both at once *on purpose*, because
producing the focus mismatch is how you measure it.

`gen-config.sh` builds the wezterm config for each case and exposes the knobs
the animation rows need (`CURSOR_BLINK_RATE`, `DEFAULT_CURSOR_STYLE`,
`TEXT_BLINK_RATE`, `ANIMATION_FPS`, `VISUAL_BELL`, `FANCY`, `SPAWN_TABS`,
`WINDOW_DECORATIONS`). It refuses combinations that silently produce a
non-blinking cursor, and `compare-case.sh`/`sample-case.sh` both call
`check_no_config_error`, which fails loudly if wezterm rejected the config —
a rejected config falls back to defaults, which means *both* windows use the
same backend and report a triumphant `AE = 0`.

**Copy regeneration commands verbatim out of the rendered document, including
any leading env vars.** Several of them carry a `PARITY_SETTLE_WARMUP=20` that
is load-bearing rather than decorative; without it, slow-to-render content lets
the settle detector declare victory on two identical *blank* captures and
reproduce the noise floor instead of the real result. `compare-case.sh` now
echoes the warmup in effect and warns on a blank-looking capture, so a run
missing it announces itself. One command (the `WINDOW_DECORATIONS` case) needs a
literal `|` and therefore lives in a fenced block below the matrix table rather
than in a table cell — use that block, not a cell.
