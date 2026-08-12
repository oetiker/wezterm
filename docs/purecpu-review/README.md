# PureCpu vs. the GPU backend: what the software renderer costs you now

*(This page was originally titled "where the software renderer falls short".
After the fix pass that title would be misleading — the four root causes it was
built around are repaired, and on a GPU-less box PureCpu now costs one to two
orders of magnitude less CPU than the renderer it is matched against. The
remaining shortfalls are stated below and they are small.)*

This is the summary of a review of your fork's two large unreviewed changes on
top of upstream `e723cf5` — the pure-Rust font stack and the `PureCpu` software
renderer. The question it answers is narrow and practical: **if you run wezterm
on a GPU-less machine with `front_end = "PureCpu"`, what do you lose compared to
`front_end = "OpenGL"`?**

Scope, by your own decision: this fork only, Linux/X11 only, no
macOS/Windows/Wayland, no upstreamability concerns, background image and
transparency out of scope.

> **These documents now describe a repaired binary.** The review was written
> first and produced documents only; a fix pass then followed, and this page has
> been rewritten to describe what PureCpu does **now** rather than what it did
> when the review was written. Everything below was re-measured against
> `wezterm-gui-6311e97` (built from commit `6311e97`), with the pre-fix binary
> `wezterm-gui-prefix` kept and re-run as the control arm wherever a before/after
> claim is made. Where a verdict changed, the detail documents state the old
> number, the new number and the commit.
>
> **The four root causes the original version of this page was built around are
> repaired.** The table below is the post-fix version; the residue is stated
> under it, plainly, and it is small and differently shaped from what the review
> found.

Two detail documents sit behind this page, and every claim here is traceable to
a row or a finding in one of them:

- **[parity-matrix.md](parity-matrix.md)** — 22 feature rows, each with a
  verdict. **16 carry the command and the numbers behind them; 1 carries a code
  citation and no measurement; 5 are out of scope and carry a reason instead.**
- **[findings.md](findings.md)** — 19 defects (18 from the review, plus one new
  cost finding the fix pass's measurement round turned up), ranked, with
  evidence class and a disposition — the fixing commit, or an explicit statement
  that it was not fixed — stated per finding.
- [noise-floor.md](noise-floor.md) — the calibration that makes the pixel
  comparisons trustworthy. Worth reading only if you want to check the method.

---

## The one thing to understand first

**PureCpu is not a separate rendering pipeline.** It consumes the same layer and
quad stream, from the same glyph atlas, as the GPU path — the hypothesis that it
draws from a different texture and therefore cannot see some content classes was
tested and is false (the GPU path has only one texture too; see
`parity-matrix.md`, "Settled by reading: the single-texture question").

So the gaps were not missing features. They were places where the hand-written
rasteriser could not reproduce what the GPU's fixed function does for free — plus
one place where it never got asked to draw at all. **Four root causes accounted
for every confirmed defect in the review. All four are now repaired:**

| # | Root cause | What it cost you | Status now |
|---|---|---|---|
| 1 | **The blit was 1:1 and cropped; there was no resampler at all** (`purecpu.rs:346`) | Anything drawn at a size different from its atlas sprite: non-native-size inline images, double-width/height lines, scaled fallback and bitmap glyphs | **Repaired** (`773b845`) — a nearest-neighbour source step, matching the GPU's nearest sampler. Non-native iTerm2 images went from `PAE = 61423` to **bit-identical**; DECDWL/DECDHL from body `PAE = 45232` to **257**, including the double-height bottom band that was previously **not drawn at all** |
| 2 | **The idle skip returned before the paint pass when no line was dirty** (`termwindow/mod.rs:1436-1447`) | Everything time-driven: GIF frames, blinking text, the visual bell | **Repaired** (`230117c`) — blink, bell and animated images get their own dirty-rect producers. All three moved to `parity`, each against the pre-fix binary as a control in the same session |
| 3 | **Cursor-blink detection tested the raw, unresolved cursor shape** (`termwindow/mod.rs:1383`) | `default_cursor_style = "Blinking*"` — the documented way to turn blinking on — was inert | **Repaired** (`868159c`) — the shape is resolved through `effective_shape` first. Blink now runs; it is the one row still `degraded`, because PureCpu's blink is a two-level square wave where OpenGL's is an eased sweep |
| 4 | **Dirty tracking was scoped to the active pane, and dirty-rect geometry omitted the pane's origin** (`findings.md` I1, I2) | **Panes in a split stopped repainting** — the worst defect in the review, and the one you would have hit first | **Repaired** (`bc05562`) — dirty rects are computed for every pane at its own origin |

**Root cause 4 was never in the parity matrix, and that is the methodological
lesson worth keeping.** No matrix row could have caught it: every harness case
ran a single full-width pane, where both pane offsets are zero and the active
pane is the only one with output. It was found by reading the patch and then
confirmed with purpose-built probes. The corpus was blind along an axis nobody
thought to vary — not along an axis the tool could not see.

**What is left.** Two *matrix rows*, neither of them a missing capability — plus
two findings that are not matrix rows at all and are listed with the full
residue further down (`findings.md` N2's open half, and the cost finding N1):

- **Cursor blink is quantised, not eased.** PureCpu alternates between two
  levels where OpenGL sweeps continuously. Deliberate in the source (~2 paints
  per cycle instead of `animation_fps` paints per cycle), and visible if you
  look for it.
- **The two backends cap their atlases at different sizes** — PureCpu at 8192,
  this box's llvmpipe at 16384 — so an image large enough to need
  `AllowImage::Scale` ends up at *different resolutions* on the two paths
  (PureCpu at Scale(4), GL at Scale(2)). Every pixel of a 17000x64 sixel strip
  differs by a uniform few LSB. Both backends are doing the documented thing
  with different constants; closing it is a memory-budget decision, not a bug
  fix. The 8192 cap exists because of C1 below.

Plus one **cost** finding, new and unfixed: a static inline image costs
**0.57 ms of CPU every frame** for as long as anything holds the animation timer
open — a blinking cursor does, by default. See "The cost result" below.

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

## The cost result, which reframes everything below

**On a GPU-less box, PureCpu is not a degraded fallback on cost. It is one to
two orders of magnitude cheaper than the renderer it is being matched against.**

The fix pass's measurement round ran four arms per case — the pre-fix binary
(floor), the fixed binary (subject), the fixed binary forced to repaint every
frame (ceiling), and the fixed binary on **OpenGL** (the control) — sequentially,
one window at a time, 30 s samples at `animation_fps = 60`:

| case | floor | subject | ceiling | **OpenGL (llvmpipe)** | subject's position floor→ceiling |
|---|---|---|---|---|---|
| idle cursor | 1.03% | **1.43%** | 36.20% | **205.30%** | 1.1% |
| SGR 5 blinking text | 0.00% | **6.00%** | 37.60% | **175.80%** | 16.0% |
| static image | 0.90% | **4.83%** | 72.07% | **248.90%** | 5.5% |

llvmpipe costs **176–249% CPU — 30 to 120× the PureCpu subject — in every
measured case.** (Above 100% because llvmpipe is multi-threaded.) That is the
strongest argument this front end exists, and it is worth stating before any
parity gap, because it is the trade the parity gaps are being paid for.

Two more results from the same round, one reassuring and one a correction:

- **The idle skip works.** The idle-cursor subject sits **1.1% of the way from
  floor to ceiling** — at the floor, which is where it should be.
- **A prediction was directionally right and quantitatively wrong by about 6×,
  and only this round could tell the two apart.** A review had predicted the
  SGR 5 blink case would sit *near the ceiling*, because one blinking cell makes
  the dirty set non-empty every animation frame and so defeats the idle skip.
  The mechanism is confirmed — a paint pass does run every frame — but the
  magnitude is not: the subject is **16% of the way to the ceiling**, 6.3×
  cheaper than a full repaint, because the dirty-rect machinery still confines
  the blit to the blinking cells' rows. Note the floor is **0.00%** here: the
  pre-fix binary spent literally no CPU on blinking text, because it never
  repainted it. That is the defect, and 6% is what correctness costs.

**Absolute percentages drift with load on this shared machine; the
floor→subject→ceiling *ratios* are the result, not the percentages.** Re-run a
whole round rather than comparing a new number against a stored one.

---

## Start here: the most severe finding

**C1 — the atlas had no size ceiling. A ~1.5 KB sixel escape sequence made
PureCpu allocate 4 GiB.** ([findings.md](findings.md), C1) **Fixed in
`c5cc8ab`**: the `PureCpu` arm now has a ceiling
(`PURECPU_MAX_TEXTURE_SIZE = 8192`) and a `bail!`, so it reaches the same
`AllowImage::Scale` downscale-and-retry the GL arm reaches — confirmed from the
renderer's own log, which now carries the fallback line where before it carried
none. The description below is the defect as found; it is kept because the
mechanism explains both the fix and the atlas-cap asymmetry the fix left behind.

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

Fix direction, as recorded at the time and as taken: give the `PureCpu` arm a
ceiling and a `bail!`, so it reaches the same `AllowImage::Scale` fallback the
`Glium` arm already reaches.

**The one thing the fix left behind, stated because it is easy to miss:** the
ceiling chosen was 8192 (256 MiB at RGBA), and this box's llvmpipe reports
16384. So the two backends now fall back at *different* points and settle on
different scale factors for the same oversized image. That asymmetry is what the
17000x64 sixel strip's residual difference actually is — it is not a rasteriser
defect, and it is not the double-composite finding M2 it was once conflated
with. It has its own row in the matrix.

**I1/I2 were the ones to act on, and they are fixed.** C1 is the most severe
finding, but it needs a hostile or freak escape sequence to fire. The split-pane
repaint bugs (root cause 4 above; `findings.md` I1 and I2) fired during ordinary
everyday use — output in a background pane never appearing, and the *active*
pane freezing when it was not at the window's top-left. Both are fixed in
`bc05562`.

---

## Headline parity result

22 features, graded against **their own named feature** — an animated GIF that
drew correctly but never advanced was `degraded` (the image was delivered, only
its motion was lost); cursor blink was `missing`, because the blink *is* the
feature.

| Verdict | Before the fix pass | **Now** | |
|---|---|---|---|
| `parity` | 6 | **15** | 16 measured, 1 by reading |
| `degraded` | 7 | **2** | cursor blink (quantised) and the atlas-cap asymmetry |
| `missing` | 3 | **0** | — |
| `known gap` | 5 | 5 | out of scope, never measured |

(21 rows before, 22 now: the atlas-cap asymmetry was added as a row of its own,
because it is what a residual previously attributed to two other causes actually
is.)

**Nine rows changed verdict**: sixel, iTerm2, animated GIF, the fancy tab bar,
DECDWL/DECDHL, blinking text, the visual bell and subpixel-antialiased text all
reached `parity`; cursor blink went `missing` → `degraded`.

**The OpenGL path is bit-identical across the whole pass.** Captured from the
pre-fix and post-fix binaries sequentially — never concurrently, because focus
state is a confound in any two-window comparison — on the same corpus with the
same generated config: `AE = 0, PAE = 0` on plain text, window chrome and
inline images alike, with matching non-zero ink on both sides so it is not a
null comparison. No PureCpu fix leaked into the GPU path. That was the single
most important invariant of the fix pass, because several fixes touched code the
two backends share.

**What works.** Ordinary terminal text is rendered soundly: monochrome text is
within one 8-bit step everywhere, with *zero* difference on background pixels
(body `PAE = 257`, body `AE = 0` from 0.5% fuzz up). **The caveat that used to
follow this sentence is gone**: it read "in a single pane — in a split the same
text can simply stop updating", and that was root cause 4, now fixed. Correct
pixels and delivered pixels are still different claims, but they no longer come
apart in a split.

Static cursors in all three shapes (block, bar, underline), window buttons,
rounded corners and split dividers are **bit-identical** on a crop tight to the
element (`AE = 0`, `PAE = 0`) — no threshold question to answer, nothing differs.
Inline images are bit-identical at their native size **and now at non-native
sizes too**. The retro tab bar is `parity` within 1 LSB rather than
bit-identical, and it is graded on a threshold borrowed from the body noise
floor; the matrix says so in its Evidence cell rather than letting it read as
equivalent to the others. **The fancy tab bar has joined it**: its 1 px glyph
shift is fixed, and the strip now measures `PAE = 257` and clears at 1% fuzz.

**What changed, and what is left.**

Every entry that used to sit under "degraded" or "missing" here has moved, with
one exception. Rather than list nine repairs, here is the shape of the change
and then the residue.

- **Inline images** — both protocols, at every size measured. The severe case
  was iTerm2 at a non-native size: PureCpu drew a *dotted grid of tiny cropped
  fragments*, one top-left tile per covered cell, where OpenGL drew a solid
  stretched block (`AE = 39360`, `PAE = 61423`, near-maximal). It is now
  **bit-identical**, and so is the 300x300 sixel whose gradient band boundary
  used to land one scan row off. Checked against a null result, not assumed:
  both backends' crops develop as sRGB with matching non-zero standard
  deviation, so both really drew the image.
- **Animated GIFs** now advance frames on an otherwise-idle screen; both frames
  appear in a sampling window where PureCpu previously reported one frame for
  all 12 samples.
- **The fancy tab bar** — both classes are gone: the 37 columns of bit-exact
  1 px shift, and the 99 columns of larger (~77/255) non-integer divergence
  beside them. The shift test now returns `AE = 0` at the *unshifted*
  alignment, which is the sharpest available form of "the shift is gone".
- **Double-width and double-height lines** — the starkest repair. OpenGL
  stretches each glyph to the doubled cell; PureCpu used to draw it at base size
  spaced at the doubled pitch, so a double line read as small letters with gaps
  (body `PAE = 45232` against `257` for ordinary lines in the same capture), and
  the DECDHL **bottom half was not drawn at all** — 1824 OpenGL ink pixels
  against exactly **0**. Now: body `PAE = 257`, and the bottom band lays down
  **1820** ink pixels against OpenGL's 1824. Ink-count ratios went 2.01, 2.10
  and 16.9 → **1.00** across the three bands.
- **Blinking text (SGR 5)** was frozen *invisible* — deterministically, at the
  eased fade's zero-intensity end, so the word was simply blank space. It now
  varies across the same band as the reference in the same run.
- **The visual bell** never visibly rang; it now flashes and fades through the
  same levels as OpenGL.
- **Subpixel-antialiased text** (`freetype_render_target = "HorizontalLcd"`)
  went further than a repair: it was the review's better-founded *by-reading*
  row, and it is now **measured**, because the same commit that implemented
  per-channel subpixel antialiasing also taught the harness the config knob it
  needed. Pre-fix body `PAE = 29041` (113 LSB, the grayscale fallback) against
  post-fix `257`.

**The residue, in full:**

- **Cursor blink is `degraded`.** Config-driven blink used to be inert — the
  documented `default_cursor_style = "BlinkingBlock"` never started at all — and
  it now runs. But PureCpu blinks as a **two-level square wave** where OpenGL
  eases continuously, which is deliberate in the source (~2 paints per cycle
  rather than `animation_fps` paints per cycle). Present, visibly coarser.
  **Which evidence carries which half of that:** the blink's *presence* is
  measured against a pre-fix control and is not in doubt; the *shape* rests on
  samples at a single interval plus the source comment, and it is the source
  comment that makes the two-level reading more than an aliasing hypothesis —
  a sampler that aliases could flatten an eased waveform into two apparent
  levels. The `degraded`-rather-than-`parity` verdict turns on the shape.
- **The atlas-cap asymmetry is `degraded`**, and it is not really a rendering
  defect: PureCpu caps its atlas at 8192 and this box's llvmpipe at 16384, so an
  image big enough to need `AllowImage::Scale` is stored at a quarter resolution
  on one path and a half on the other. Every pixel of the 17000x64 test strip
  differs, by a uniform few LSB. Confirmed from both backends' own logs in the
  same run rather than inferred.
- **Scaled fallback and bitmap glyphs** (including colour emoji) is the one
  `parity` verdict resting on reading alone. Its old `degraded` rested on the
  1:1 crop, which no longer exists — the DECDWL row demonstrates the same code
  path with numbers — but the precondition was never established: some installed
  font must actually yield `glyph.scale != 1`, and a capture using no such font
  would return a null result indistinguishable from parity. Read it as an
  untested prediction that now points at parity, not as a result.
- **Animated-image ink that escapes its cell is still not repainted**
  (`findings.md` N2, the open half). Glyph ink escaping a dirtied cell *was* a
  defect — a descender left frozen below its cell while the glyph above it
  blinked — and it is fixed; it is also **the one defect in this pass that was
  reproduced in pixels rather than reasoned about**. But the fix grows dirty
  rects to overlapping quads on vertex buffer 1, where glyphs live, and image
  quads live on buffer 0 and are shifted by the window padding. So four of the
  five dirty-rect producers are covered and animated images are not. Admitting
  buffer 0 wholesale would grow every incremental frame to the pane-sized
  background quad, i.e. to a full repaint.
- **One new, unfixed cost finding**: a static inline image costs 0.57 ms of CPU
  per frame (`findings.md` N1).

---

## How much to trust each verdict

The review distinguishes these deliberately, and this page does not flatten them:

- **Measured** — a command was run against the built binary and the numbers are
  in the Evidence cell. **16 of 22 rows**, including every `parity` bar one and
  both `degraded`s. Every row was re-run against the post-fix binary in the
  verification pass, and every row whose verdict *changed* was graded with the
  **pre-fix binary as a control in the same session** — which is what makes a
  negative result mean anything. In several cases the control reproduced the
  documented pre-fix defect exactly (blinking text flat at `gray(16)`, cursor
  blink flat at `gray(224)`, subpixel AA at body `PAE = 29041`), which proves
  the instrument was working on the day the subject came back clean.
- **By reading** — the code path is unambiguous but no runtime trigger was
  demonstrated. **1 row**: *scaled fallback / bitmap glyphs*. Its precondition —
  that some installed font yields `glyph.scale != 1` — is untested and
  font-dependent, and it was left unmeasured on purpose, because a null result
  there would be indistinguishable from parity. (*Subpixel-antialiased text* was
  the other by-reading row; it is now measured, because the fix pass added the
  config knob the harness had been missing.)
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

> **All of this section's findings are dispositioned in `findings.md`**, each
> with the commit that addressed it. The severity ordering below is kept because
> it is how you should read the document, and the defect descriptions are kept
> because they explain what the fixes had to do.

**Important — content silently not drawn in an ordinary configuration. This was
root cause 4 from the summary table, and it was the part of this review to act
on first. Both are fixed in `bc05562`. Neither finding was reachable by the
parity matrix at all**, for a reason worth internalising: every case in the
harness put the content under test in a **single full-width pane**, where
`pos.top` and `pos.left` are both zero and the active pane is the only one with
live output. The corpus was blind along an axis nobody thought to vary — not
along an axis the tool could not see:

- **I1** — dirty tracking consulted only the **active** pane, so output in any
  other pane of a split was never repainted until something forced a full
  repaint. Measured.
- **I2** — dirty-rect geometry omitted the pane's `pos.top`/`pos.left`, so even
  the *active* pane froze when it was not at the window's top-left. Measured on
  both axes.

**I3, I4, I5** are root causes 2, 3 and 1 stated as findings, with the matrix
rows they explain cross-referenced. All three are fixed (`230117c`, `868159c`,
`773b845`); I4's row is `degraded` rather than `parity` because the blink is now
present but quantised.

**Medium, but read these three first within their class** — they are
defect-class Critical (process aborts reachable from font data), recorded Medium
only because no crafted font was built to fire them (`fontTools` was not
available on the machine). **Their division sites were guarded in `b10c3a5`, and
that is deliberately not called "fixed":** no font reaching any of them was ever
built, so nothing demonstrates a trigger now passing that used to abort. The
guard is what changed; the evidence class did not.

- **M3** — unconditional panic in the empty-path fallback of the COLR glyph
  rasteriser. The fallback is unfinishable and panics whenever reached; tiny-skia
  semantics were verified by running a standalone program. What is *not* shown is
  the font that reaches it.
- **M4** — `cpal.color_record_indices()[0]` panics on a CPAL table with zero
  palettes.
- **M5** — `unitsPerEm = 0` divides by zero in both the shaper and the rasteriser.

**M1** explained the tab bar's bit-exact 1 px shift: destination coordinates were
truncated (`as i32`) instead of rounded. Fixed in `773b845` by exactly the
one-call fix direction it recorded — and the shift test now returns `AE = 0` at
the *unshifted* alignment. **M2** documented that overlapping dirty rects
composite the same pixel twice; it is **closed by re-measurement with no code
change**, and the way it was closed is the better lesson: in one experiment the
pre-fix control reproduced M2's signature to the digit (`n = 87`,
`max|resid| = 1.91`, `mean = -0.05`) while the subject returned `n = 0`. The
control is what makes the negative mean anything. Which commit removed it was
**not** determined and was not guessed.

**M6, M7, L1-L5** are latent edge cases, performance and hygiene, and three of
them are worth knowing about:

- **L1** (per-band GC create/destroy) had its central claim **corrected**: those
  are no-reply X requests queued with the `PutImage` they bracket, not round
  trips, so there was no latency to save. The optimisation was built anyway,
  **measured null on both the client and the X-server side**, and reverted.
- **L2** (per-glyph `HintingInstance`) is fixed by a size-keyed cache
  (`bce58ea`), with a test asserting the cache is invisible in the output.
- **L5** is **closed as a non-defect**: it claimed removed font backends are
  "silently substituted", and all three sites in fact warn, naming both what was
  asked for and what was used.

`findings.md` also carries a **"Considered and rejected"** section of 7
non-findings, recorded so nobody re-derives them — including the triple-buffer
misreading and several apparent panic sites that are in fact guarded.

---

## What this review does *not* cover

Stated plainly, because a clean-looking matrix is misleading without it.

**Out of scope by your decision, never measured** — 5 `known gap` rows:
`window_background_image`, `window_background_opacity`,
`text_background_opacity`, background blur / HSB tint, and the window background
image sampling filter. The last of these *was* read, and the reading has been
updated rather than left stale: the GPU samples background-image quads
*bilinearly*, and while the rasteriser did gain a resampler in the fix pass,
`IS_BG_IMAGE` was **deliberately left on the old crop path** because background
images are out of scope. So the predicted divergence is now two deep — PureCpu
crops where the GPU rescales, and the GPU's rescale is bilinear where PureCpu's
new sampler is nearest — and it is a deliberate scope boundary rather than an
oversight. No capture was ever taken; treat it as an untested prediction. The
other four were not looked at at all.

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
Its regeneration block is fenced in the matrix and does run. **It was not
re-run after the fixes**, so the app-driven blink path's quantisation
sub-finding still rests on its original measurement; the config-driven path,
which is the common case and the one that was `missing`, was re-measured with a
control.

**Method limits.** Three worth knowing:

- **Every harness case ran the content under test in a single full-width pane.**
  That is what made I1 and I2 invisible to the matrix — not that captures were
  settled, since Task 4 added a non-settling sampler that the animation rows and
  the GIF row all use. With one pane, `pos.top`/`pos.left` are zero and the
  active pane is the only one producing output, so both defects are identically
  silent. Any defect that needs a split would have been missed the same way.
- **The sampling interval is part of the instrument, and it can silently disarm
  the animation method.** Those rows ask "does the sampled value change over
  time", which presupposes the *reference* arm visibly changes — and it does not
  at every interval. Re-grading cursor blink post-fix, the OpenGL arm read
  essentially flat at 0.4 s, 0.1 s and 0.2 s, because each `import` costs a
  variable fraction of the interval and the samples keep landing on the same
  phase; 0.15 s resolved both arms cleanly. A flat subject against a flat
  reference proves nothing and looks exactly like the defect. Check the
  reference arm before reading a flat sequence as a finding.
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

**The test suite now says something, where it used to say nothing.** It passes
at **127** (`wezterm-gui`) **+ 18** (`wezterm-font`), against 29 + 1 at review
time. The number is not the point — the original complaint was the sharper one
that a green suite was *uninformative*, because a `panic!` planted inside the
blit loop left it green. That is closed: the loop now has a framebuffer harness,
and mutation-testing a statement inside it **kills 7 tests, all at the mutated
statement**, with a discriminating negative that correctly does not die (a test
that returns before reaching the mutated loop). Breaking the loop turns the
suite red.

What the suite still does **not** cover, stated so the number is not read as
more than it is: nothing in `call_draw_purecpu` itself; and inside the blit
loop, the whole `subpixel_aa` arm, `apply_hsv`, per-vertex HSV, `mix_value != 0`
and the degenerate-quad `continue`. Nor does any test execute the animation
plumbing in `mod.rs` — `AnimatedCellScan` and `image_next_frame_due` are
exercised only by the sampled animation rows, which are graded on a colour
sequence rather than on an assertion.

---

## Re-running the harness

Everything lives in [`tools/purecpu-parity/`](../../tools/purecpu-parity/).
`lib.sh`'s default `WEZTERM_BIN` is the pre-fix
`/scratch/oetiker/wezterm-builds/wezterm-gui-rebased`, so **every command below
needs `WEZTERM_BIN` pointed at the binary you actually mean** — the post-fix
numbers in these documents come from
`/scratch/oetiker/wezterm-builds/wezterm-gui-6311e97`. Keeping the pre-fix
binaries is deliberate: `wezterm-gui-prefix` is the control arm of every
before/after comparison here and should not be deleted or overwritten. `out/` is
gitignored — every PNG referenced in the detail documents is a run artifact,
regenerated by the commands printed beside the numbers they produced.

```bash
export WEZTERM_BIN=/scratch/oetiker/wezterm-builds/wezterm-gui-6311e97
```

**The CPU-cost round** is a separate driver, because it measures time rather
than pixels:

```bash
SUBJECT_BIN=$WEZTERM_BIN ./measure-round.sh    # FLOOR_BIN defaults to wezterm-gui-prefix
```

It runs four arms (floor / subject / ceiling / OpenGL) across three cases,
sequentially, one window at a time. Read the ratios, not the absolute
percentages — they drift with load on this shared machine.

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
