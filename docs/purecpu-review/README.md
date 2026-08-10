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
  verdict. **14 carry the command and the numbers behind them; 2 carry a code
  citation and no measurement; 5 are out of scope and carry a reason instead.**
- **[findings.md](findings.md)** — 18 defects found by reading the fork's own
  patch, the most severe of them then confirmed by runtime probes against the
  built binary, ranked, with evidence class stated per finding.
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
rasteriser cannot reproduce what the GPU's fixed function does for free — plus
one place where it never gets asked to draw at all. **Four root causes account
for every confirmed defect in this review:**

| # | Root cause | Where | What it costs you |
|---|---|---|---|
| 1 | **The blit is 1:1 and crops; there is no resampler at all** | `purecpu.rs:346` | Anything whose quad is drawn at a size different from its atlas sprite: non-native-size inline images, double-width/height lines, scaled fallback and bitmap glyphs |
| 2 | **The idle skip returns before the paint pass when no line is dirty** | `termwindow/mod.rs:1436-1447` | Everything time-driven: GIF frames, blinking text, the visual bell, and cursor blink's *app-driven* path (quantised to ~2 paints/cycle) |
| 3 | **Cursor-blink detection tests the raw, unresolved cursor shape** | `termwindow/mod.rs:1383` vs `render/mod.rs:604-611` | `default_cursor_style = "Blinking*"` — the documented way to turn blinking on — is inert. This is the *config-driven* blink path; root cause 2 governs the app-driven one |
| 4 | **Dirty tracking is scoped to the active pane, and dirty-rect geometry omits the pane's origin** | `findings.md` I1, I2 | **Panes in a split stop repainting.** Output in any non-active pane never appears; and even the active pane freezes when it is not at the window's top-left |

**Root cause 4 is not in the parity matrix, and if you use splits it is
probably the one that will bite you first.** No matrix row could have caught it:
every harness case ran a single full-width pane, where both offsets are zero and
the active pane is the only one with output. It was found by reading the fork's
patch and then confirmed with purpose-built probes. Root causes 1-3 are what the
pixel comparisons measure; 4 is why the pixel comparisons were not the whole job.

The single most severe finding is separate from all four, and it is not a
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

## Start here: the most severe finding

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

**And read I1/I2 next, before the matrix.** C1 is the most severe finding, but
it needs a hostile or freak escape sequence to fire. The split-pane repaint bugs
(root cause 4 above; `findings.md` I1 and I2) fire during ordinary everyday use
of a feature you probably use, and they are `Important` for the same reason C1
is `Critical` — content is silently not drawn. They are written up under "the
rest of findings.md" below only because that section follows findings.md's own
severity ladder, not because they are minor.

---

## Headline parity result

21 features, graded against **their own named feature** — an animated GIF that
draws correctly but never advances is `degraded` (the image is delivered, only
its motion is lost); cursor blink is `missing` because the blink *is* the
feature.

| Verdict | Count | |
|---|---|---|
| `parity` | 6 | all measured; 4 bit-identical on the element measured, 2 within 1 LSB |
| `degraded` | 7 | 5 measured, 2 concluded by reading |
| `missing` | 3 | all measured |
| `known gap` | 5 | out of scope, not measured |

**What works — in a single pane.** Ordinary terminal text is *rendered* soundly:
monochrome text is within one 8-bit step everywhere, with *zero* difference on
background pixels (body `PAE = 257`, body `AE = 0` from 0.5% fuzz up). Read that
as a statement about the rasteriser, not about daily use: **every row in this
table was measured on a single full-width pane, and in a split the same text can
simply stop updating** (root cause 4 / I1 / I2). Correct pixels and delivered
pixels are different claims, and the matrix only certifies the first. Static cursors in all three shapes (block, bar, underline), window buttons,
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
- **Double-width and double-height lines (DECDWL/DECDHL)** — measured in a
  Task 8 addendum, and the result is stark. OpenGL stretches each glyph to the
  doubled cell; PureCpu draws it at **base size, spaced at the doubled pitch**,
  so the line reads as small letters with gaps. Body `PAE = 45232` (176 LSB,
  failing the gate by two orders of magnitude) against **`PAE = 257` for
  ordinary single-width lines in the very same capture**. Worse, the DECDHL
  **bottom half is not drawn at all** — the top line's quad is supposed to cover
  both rows, and the 1:1 blit clips it away (ink ratio 16.9, zero PureCpu ink on
  every scan row of the band). `degraded` rather than `missing` because the text
  is still delivered and legible; the bottom-half row alone is a `missing`
  sub-case.
- **Scaled fallback and bitmap glyphs** (including colour emoji) — same
  `purecpu.rs:346-347` mechanism, now demonstrated by the row above, but
  deliberately not measured: it needs an installed font that actually yields
  `glyph.scale != 1`, and a capture that used no such font would return a null
  result indistinguishable from parity.
- **Subpixel-antialiased text** (`freetype_render_target = "HorizontalLcd"`) —
  the mildest entry here, and the README previously made it sound like the
  worst. **PureCpu falls back to ordinary grayscale antialiasing**: the LCD path
  already stores a max-of-channels alpha alongside the per-channel coverage
  (`skrifa_rasterizer.rs:687,701`), which PureCpu's glyph branch uses
  (`purecpu.rs:412-419`). The glyph is entirely present, correctly positioned
  and legible. What you lose is the subpixel horizontal resolution and the
  colour fringing — not the text.

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
  in the Evidence cell. **14 of 21 rows**, including every `parity`, every
  `missing`, and (as of the Task 8 addendum) double-width/height lines. Five
  state "reproduced from a clean shell" in the matrix — the four
  cursor/animation rows plus text glyphs — and the DECDWL addendum was run twice
  from clean shells with identical numbers; several others were re-run
  independently by a second reviewer during the review rounds.
- **By reading** — the code path is unambiguous but no runtime trigger was
  demonstrated. **2 rows**, and they are not equally exposed:
  - *Subpixel-antialiased text* has **no runtime precondition left to fail**.
    The chain is traceable end to end (`HorizontalLcd` → `use_lcd_subpixel` →
    `subpixel_mask_to_rgba` → `has_color = false` → PureCpu's glyph branch). If
    you set the option, the divergence follows. This is the better-founded of
    the two, and its consequence is mild — grayscale AA, see above.
  - *Scaled fallback / bitmap glyphs* shares its mechanism with the DECDWL row,
    which is now measured, but its precondition — that some installed font
    yields `glyph.scale != 1` — is untested and font-dependent. It was left
    unmeasured on purpose, because a null result would be indistinguishable
    from parity.
- In `findings.md`, the same distinction is enforced by a written demotion rule:
  a defect whose trigger was not reproduced is recorded one class below its
  defect class. That is why M3/M4/M5 sit at Medium.

**Measurement has overturned a by-reading verdict twice in this review, and
confirmed one — and the pattern in the failures is worth knowing.** Inline
images went from a predicted `degraded` to measured `parity` at native size (and
back to `degraded` only on a corpus that could actually trigger the defect), and
cursor blink went from a by-reading `degraded` to a measured `missing`. In
neither case was the *mechanism* wrong; both times the **precondition** was —
images needed a non-native requested size, blink needed the shape to be resolved.
That is why the last untested precondition in the document was worth spending a
measurement on: nobody had shown `printf '\033#6'` actually reaches the
double-width path in this fork. It does, and the reading was right. Two
overturns and one confirmation is the honest record; it is also the best
evidence available that the instrument was not merely confirming its own
predictions.

---

## The rest of findings.md, in severity order

`findings.md` presents findings in id order within each class, which is *not*
severity order inside Medium. The order below is the severity ladder the
document itself defines, applied consistently.

**Important — content silently not drawn in an ordinary configuration. This is
root cause 4 from the summary table, and if you use splits it is the part of
this review to act on first. Neither finding was reachable by the parity matrix
at all**, for a reason worth internalising: every case in the harness put the
content under test in a
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

**Declared unmeasured inside rows that were otherwise measured** — **three**
things, all recorded in the matrix under "Grading basis for animation rows":
`VisualBellTarget::CursorColor` (the bell row samples an empty-background pixel,
so only the default `BackgroundColor` target was exercised); rapid blink (SGR 6
/ `text_blink_rate_rapid`), as distinct from the SGR 5 path the blinking-text
row measures; and any interaction between the three animations running at once.
Each row's verdict covers what its Evidence cell actually sampled.

A fourth item belongs beside these but is a different thing and should not be
counted with them: the DECSCUSR cursor-blink sub-finding **was** measured (30
samples at 0.1 s), just with a hand-written config, because `gen-config.sh`
correctly refuses the blink-rate/cursor-style combination that probe requires.
Its regeneration block is fenced in the matrix and does run.

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
same backend and report a triumphant `AE = 0`. **One limit worth knowing before
you trust an `AE = 0`:** the guard greps `launch`'s log
(`lib.sh:87-90`, `grep -q "Configuration Error" "$log" 2>/dev/null`), so if that
log were missing or unreadable it would pass silently rather than fail closed.
Every capture cited in these documents was produced with logs present and
checked, but a future run that loses the log loses the guard with it.

**Copy regeneration commands verbatim out of the rendered document, including
any leading env vars.** Several of them carry a `PARITY_SETTLE_WARMUP=20` that
is load-bearing rather than decorative; without it, slow-to-render content lets
the settle detector declare victory on two identical *blank* captures and
reproduce the noise floor instead of the real result. `compare-case.sh` now
echoes the warmup in effect and warns on a blank-looking capture, so a run
missing it announces itself. One command (the `WINDOW_DECORATIONS` case) needs a
literal `|` and therefore lives in a fenced block below the matrix table rather
than in a table cell — use that block, not a cell.
