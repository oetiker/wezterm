# PureCpu fix pass — design

Repairs the defects found by the PureCpu parity review
(`docs/purecpu-review/`), which produced documents only. This document scopes
the fixes, the code structure they land in, and how each one is verified.

Predecessor: `docs/superpowers/specs/2026-08-10-purecpu-parity-review-design.md`
(the review that found these defects) and its deliverables under
`docs/purecpu-review/`. Every fix below cites a finding id from
`docs/purecpu-review/findings.md` or a row in `parity-matrix.md`.

## 1. Mission

Fix everything the review found worth fixing, in the user's fork, so that
`front_end = "PureCpu"` on a GPU-less Linux/X11 machine loses as little as
possible against `front_end = "OpenGL"`.

Unlike the review, **this work writes Rust.** The read-only-source-tree
constraint that governed the review is lifted and replaced by the constraints in
§8.

## 2. Scope

**In scope — all 18 findings** (C1, I1–I5, M1–M7, L1–L5), plus two items the
review left outside its own findings list at the user's direction:

- **Subpixel-antialiased text.** The review graded this `degraded` by reading
  and rated it the mildest gap. It is in scope because the data needed to fix it
  already exists in the atlas (§4.3).
- **Scaled fallback / bitmap glyphs.** The matrix row rests on code reading
  alone because no installed font was known to yield `glyph.scale != 1`. The
  resampler (§4.2) should fix it by construction; confirming that requires
  installing a font that triggers the path (§6.4).

**Out of scope:** the five `known gap` rows — `window_background_image`,
`window_background_opacity`, `text_background_opacity`, background blur / HSB
tint, and the background-image sampling filter. Four of these were never
examined at all; nobody yet knows whether PureCpu draws them wrong or not at
all, so they are investigation work, not defect repair.

Platform scope is inherited from the review: **Linux/X11 only.** No macOS,
Windows or Wayland. No upstreamability concerns — `PureCpu` is fork-introduced
and has no upstream counterpart.

## 3. Structural approach

Three options were considered:

- **A — point fixes only.** Fifteen independent diffs at the cited lines.
  Smallest per-fix change, no churn. Rejected because the resampler and the
  subpixel mask would land as further branches inside an already 376-line,
  five-deep function, and the two dirty-rect bugs would be fixed at their call
  sites with nothing preventing a third site from reintroducing them.
- **B — point fixes plus two extracted units.** Chosen. See §4.
- **C — shader-equivalence rewrite** of the quad loop mirroring
  `glyph-frag.glsl` branch for branch, pinned by golden-image tests. Rejected
  for now, for three reasons, none of which is blast radius (PureCpu is
  fork-local and endangers no upstream or GL code):
  1. It restructures the site of only 5 of 18 findings. The other 13 are point
     fixes under any option, so C is a cleaner fifth of the plan, not a cleaner
     plan.
  2. It spends its risk where the code is already correct. Six matrix rows are
     `parity` today and four are bit-identical — static cursors in all three
     shapes, window buttons, rounded corners, split dividers, plus native-size
     inline images (`AE = 0, PAE = 0`) and body text at 1 LSB. All are produced
     by the loop C would rewrite, and they are right *by measurement*, not by
     construction — particularly the linear/sRGB and HSV handling at
     `purecpu.rs:432-460`.
  3. `call_draw_purecpu` has **no** unit-test coverage today (the 11 tests in
     the file all attach to surrounding helpers), so a rewrite would pass a
     green suite while rendering garbage.

**B is a strict prefix of C, not a compromise against it.** Extracting the
sampler is the first move of shader-equivalence, and the unit tests in §6.2
attach to the extracted units. If after this work the loop still wants the rest
of the restructuring, C becomes its own step with a suite that can prove it did
not break the four bit-identical rows.

## 4. Architecture

### 4.1 Unit 1 — `dirty`: pane-aware dirty-rect geometry

`purecpu_state.dirty_pixel_rects` is built at `wezterm-gui/src/termwindow/mod.rs:1300-1375`
from a single `pane_info` Option — the **active** pane — and every rect is
computed as `content_top + row * cell_h` with `x: 0, width: fb_width`. That is
three defects, not two:

- no `pos.top` (I2, vertical axis),
- no `pos.left` (I2, horizontal axis),
- full-window-width bands, which are wrong for any pane that is not full width.

I1 is the *caller*: one pane instead of all panes in the tab.

The unit takes an explicit pane placement — origin, cell extent, viewport,
dirty rows, cursor position — and returns rects already in window coordinates.
Building a rect without stating where the pane is becomes structurally
impossible, because the origin is an argument rather than an ambient assumption.
The caller changes from "get the active pane" to "iterate the tab's panes".

Roughly 60 lines move out of `mod.rs`. The arithmetic gains unit tests with a
non-zero origin — the case that had neither test nor harness coverage, and the
reason both defects were invisible to a 21-row matrix.

### 4.2 Unit 2 — `sampler`: the atlas sampler PureCpu never had

`wezterm-gui/src/termwindow/render/purecpu.rs:346` blits 1:1 and crops; the
rasteriser contains no resampler at all (I5).

The unit maps a destination pixel to a source texel given a source rect and a
destination rect, matching the GPU's sampler exactly:
`MagnifySamplerFilter::Nearest` + `MinifySamplerFilter::Nearest` +
`SamplerWrapFunction::Clamp` (`wezterm-gui/src/termwindow/render/draw.rs:215-218`),
including GL's half-texel centre convention.

**Nearest is not an approximation of what the GPU does — it is what the GPU
does.** `atlas_linear_sampler` is used in exactly one shader branch,
`o_has_color == 2.0`, the window background attachment
(`wezterm-gui/src/glyph-frag.glsl:121`), which is out of scope. Glyphs, colour
emoji and images all sample `atlas_nearest_sampler`. Bit-exact parity on scaled
quads is therefore reachable, and the half-texel convention is what decides
whether we land bit-exact or one row off.

Every textured quad goes through the unit. **When source and destination extents
are equal it must reduce to today's 1:1 path exactly** — directly testable, and
the property that protects the four bit-identical rows.

This single unit fixes I5, the DECDWL/DECDHL sizing (glyphs drawn at base size
at doubled pitch), the DECDHL blank bottom band (the top line's quad is supposed
to cover both rows and the 1:1 blit clips it away), and scaled bitmap/emoji
glyphs. M1's truncation-vs-rounding belongs here too, being the same coordinate
mapping.

### 4.3 Subpixel antialiasing — a point fix, not a unit

The LCD path already stores per-channel sRGB coverage in R/G/B with the
max-of-channels alpha in A (`wezterm-font/src/rasterizer/skrifa_rasterizer.rs:695-701`).
PureCpu's `IS_GLYPH` branch reads only `tex_a` and discards R/G/B
(`purecpu.rs:412-419`), then composites through a scalar-alpha `blend_over`
(`purecpu.rs:529`). GL instead emits a per-channel `colorMask` and selects
`dual_source_blending` when `subpixel_aa` is set (`draw.rs:262-266`).

The fix is to carry three coverage channels out of the `IS_GLYPH` branch and add
a per-channel variant of `blend_over`. No new rasterisation and no font work —
the data is already in the atlas. This stays inside the loop rather than
becoming a seam.

### 4.4 Invariant: the OpenGL path must come out bit-identical

Two fixes touch code shared with the GL path — C1 in `renderstate.rs` and the
pane iteration in `mod.rs`. Everything else is behind `purecpu_state` or inside
`purecpu.rs`. The idle skip (`mod.rs:1436-1447`) and cursor-blink detection
(`mod.rs:1383`) were confirmed to sit inside a PureCpu-gated function, so fixes
I3/I4 cannot reach GL.

This is verified, not assumed: §6.3.

## 5. Fix inventory and order

Work proceeds straight through. The order is not arbitrary — four items change
what a later item measures (§5.1).

| # | Fix | Where | Findings |
|---|-----|-------|----------|
| 1 | Atlas ceiling + `bail!` so the PureCpu arm reaches `AllowImage::Scale` | `renderstate.rs:78-118` | C1 |
| 2 | Unit 1; iterate all panes; origin as an argument | `mod.rs:1300-1375` → new `dirty` unit | I1, I2 |
| 3 | Clamp instead of skip; present bands that pass the bottom edge | `purecpu.rs` | M7, M6 |
| 4 | Unit 2; round rather than truncate destination coordinates | `purecpu.rs:346` → new `sampler` unit | I5, M1 |
| 5 | Per-channel coverage out of `IS_GLYPH`; per-channel blend | `purecpu.rs:412-419, 529` | (subpixel row) |
| 6 | Resolved cursor shape; per-feature dirty rects for bell, SGR 5 text blink and GIF frame advance; integer-division blink interval | `mod.rs` | I4, I3, L3 |
| 7 | Double-composite — **re-measure first** | `purecpu.rs` | M2 |
| 8 | COLR empty-path panic; zero-palette CPAL; `unitsPerEm = 0` | `wezterm-font` | M3, M4, M5 |
| 9 | Band copy and X GC churn; per-glyph `HintingInstance`; `CURRENT_TIME`; silent backend substitution | mixed | L1, L2, L4, L5 |

### 5.1 Ordering consequences

- **C1 fixes a parity row, not only the OOM.** The 17000×64 sixel case is
  `degraded` because GL downscales via `AllowImage::Scale(2)` and PureCpu does
  not. Giving the PureCpu arm the same ceiling and fallible path makes it take
  the same fallback, so that sub-case is repaired by fix 1 and the resampler
  never sees it.
- **M2 is deferred behind a re-measurement** (fix 7, after fix 2). Its
  attribution to a specific rect overlap was tested and eliminated twice; the
  surviving evidence is a coverage model that predicts all 87 differing pixels
  within 1.91 LSB. The dirty-rect geometry is the suspected source and fix 2
  rewrites it, so M2 may not exist afterwards — and if it does, it will be a
  different overlap than the one that was hunted. Fixing it before re-measuring
  would be fixing a phantom.
- **The scaled-glyph font must be installed before fix 4 is verified** (§6.4).
- **Fix 6's regression is invisible by construction.** Its purpose is repaints
  on an idle screen; the failure mode is repainting *too much*, which looks
  perfect and costs CPU. It needs an explicit idle-CPU measurement (§6.3), not
  "the bell rings now".

### 5.2 Per-feature dirty rects (fix 6)

The idle skip is kept. Each animated thing marks its own region dirty instead:
the `Alert::Bell` handler pushes a window rect (today it invalidates the window
but pushes no rect, so the early exit still fires and the background-mix
computation never runs); blink scheduling dirties the cursor cell and the
blinking-text cells; GIF frame advance dirties the image quad.

This preserves the idle-efficiency property exactly — an idle screen with
nothing animating still does nothing — which is the reason PureCpu exists. The
rejected alternative was to skip the early exit whenever any animation is live:
one condition instead of four sites, but a blinking cursor is present in almost
every session, so it would repaint the whole window at the blink rate forever
and give up the dirty-region win in normal use.

`purecpu_force_full_repaint` is retained. It is the control experiment that
proved the diagnosis, and §6.3 uses it as the too-much baseline.

## 6. Verification

Three layers, because no single one is trustworthy here.

### 6.1 Layer 1 — harness rows, re-run with their own printed commands

Every fix with a matrix row is done when that row's command re-runs and the
number crosses its gate. Commands are copied **verbatim out of the rendered
document, including leading env vars** — the review recorded five separate
occasions where a printed command nobody re-ran was wrong, twice in the
reassuring direction. Both implementer and reviewer run them.

Load-bearing details inherited from the review: `PARITY_SETTLE_WARMUP=20` on the
commands that carry it; the `WINDOW_DECORATIONS` case lives in a fenced block
rather than a table cell because it needs a literal `|`; `FUZZ` is never raised
to make a case pass.

### 6.2 Layer 2 — Rust unit tests on the extracted units

These do not exist today in any form.

- **`sampler`**: 1:1 identity reduces to today's blit exactly; 2×
  magnification against a hand-computed nearest-neighbour expectation; the
  half-texel centre convention at a boundary; clamp behaviour at edges; a
  degenerate zero-extent rect.
- **`dirty`**: rects at a non-zero `pos.top`; at a non-zero `pos.left`; both
  together; a pane narrower than the window; a band that would run past the
  framebuffer bottom (M6); rows outside the viewport.
- **Per-channel `blend_over`** against the dual-source blend arithmetic.
- **Atlas ceiling**: a request above the cap returns `Err` rather than
  allocating.

### 6.3 Layer 3 — regression invariants, run once at the end

- **GL bit-identical.** One case, GL-vs-GL, pre-fix binary against post-fix
  binary: `AE = 0` required. This is the check behind §4.4.
- **The six `parity` rows must not move.** Four are bit-identical today; fix 4
  rewrites the code that produces them. Any drift is a regression, not a
  rounding difference.
- **Idle CPU.** Measured with a blinking cursor on screen, against two
  baselines: the pre-fix binary (the floor) and `purecpu_force_full_repaint =
  true` (the ceiling we must stay well below).

### 6.4 The scaled-glyph precondition

The scaled fallback / bitmap glyph row cannot be verified against a font that
never yields `glyph.scale != 1`: a clean re-run would return a null result
indistinguishable from parity — the exact mistake that produced this review's
false `parity` on inline images. Installing or locating such a font is a
**prerequisite task for fix 4's verification**, not a step within it. If no font
can be made to trigger the path, the row stays `by reading` and says so.

### 6.5 The standing anti-false-verdict rule

Every verification must answer: **"could this run have failed, and what would
that have looked like?"** A fix verified by a clean number carries the same
exposure the defect measurements did — a blank capture, a rejected config
falling back to the same backend on both sides, a flat sample sequence from a
window that never rendered. The review found nine distinct instances.

Where it is cheap, **put the control inside the capture**, as the DECDWL corpus
did — print the fixed case and a reference case in the same frame, so a null
result cannot pass as success because the frame itself proves the instrument was
working.

## 7. Environment, build and execution

**Environment.** Display `:20`, alive at the time of writing, with the existing
Xauthority at
`/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20`.
`PARITY_XAUTH` must be exported for every harness run; `lib.sh` fails closed via
`${VAR:?}` if it is not. If `:20` is gone, recreate it per the review plan's
"Environment setup" section.

**Build.** `cargo build --release -p wezterm-gui` with `-j4`, into the shared
`CARGO_TARGET_DIR`. The fixed binary goes to a **new** path,
`/scratch/oetiker/wezterm-builds/wezterm-gui-fixed`;
`/scratch/oetiker/wezterm-builds/wezterm-gui-rebased` stays untouched as the
pre-fix reference, because §6.3's GL invariant needs old against new. The
harness selects the binary via `WEZTERM_BIN`.

**Execution.** One subagent implements a fix, an independent second reviews it;
the same implementer is resumed for fix rounds and the same reviewer for scoped
re-reviews. Every review in the predecessor plan found at least one Critical or
Important defect. The user is in the loop **once, at the end** — not per fix.
Subagents must write their full report to a file and return only a short inline
summary; inline replies are frequently lost when a subagent terminates on idle.

## 8. Constraints

- **Never touch X displays `:10`–`:14`** — other users on this shared machine.
  All testing is on `:20`.
- **The audible bell stays disabled in every run.** `gen-config.sh` already
  emits `audible_bell = 'Disabled'` unconditionally (line 182, outside the
  `VISUAL_BELL` branch), but wezterm's default is `AudibleBell::SystemBeep`, so
  a hand-written config that omits the line beeps — repeatedly, on a shared
  machine, for the duration of a sampling run. Two rules follow:
  - Every config in this work comes from `gen-config.sh`. If a combination it
    refuses is genuinely needed, **extend `gen-config.sh`** rather than
    hand-writing around it. (The review already hit this once: the DECSCUSR
    probe needed a hand-written config because `gen-config.sh` correctly refuses
    the blink-rate/cursor-style combination it requires.)
  - `lib.sh` gains a `check_bell_disabled` that greps the effective config and
    fails closed, alongside the existing `check_no_config_error`.
- **Never more than 4 cores**, including the cargo build (`-j4`).
- **Never run the 65536-atlas case** — 16 GiB on a shared ~25 GiB box. The
  32768 case (4 GiB) is already measured; C1's fix must make both unreachable,
  and that is verified by the unit test in §6.2, not by allocating.
- **Shared harness files** (`lib.sh`, `gen-config.sh`, `compare-case.sh`,
  `sample-case.sh`) are load-bearing for the review's committed results: any
  change must keep default output byte-identical, verified.
- **Never raise `FUZZ`**, and never grade a case against another case's number.
  Grade against a stated basis.
- **Sequential capture is mandatory.** Two concurrently-open windows leave one
  unfocused, and wezterm draws a hollow cursor when unfocused — injecting a full
  character cell of difference that survives 12% fuzz. `focus-probe.sh` is the
  one deliberate exception.
- Do not commit `mise.toml` (untracked, intentional); `out/` and
  `.superpowers/` stay gitignored.
- The deliverables under `docs/purecpu-review/` are a record of the review as it
  stood. Fixes update them only where a row's verdict changes, and such an
  update states the binary it was measured against.

## 9. Risks

- **The half-texel convention in Unit 2.** Getting it wrong lands everything one
  row or column off — a defect that looks like a fix and passes a gate tuned for
  1 LSB only if nobody checks position. Mitigated by the identity test and by
  the six no-drift rows.
- **Fix 6 repainting too much.** Invisible to any pixel comparison; caught only
  by the idle-CPU measurement in §6.3.
- **Rewriting the code behind four bit-identical rows** (fix 4). Mitigated by
  making the identity case a unit-tested property of the sampler and by
  re-running all six `parity` rows.
- **M3/M4/M5 fixed blind.** Their triggers were never reproduced (`fontTools`
  was unavailable). The fixes are guards on unambiguous panic sites, but "fixed"
  here means "the panic site is guarded", not "the crash was reproduced and no
  longer occurs". This must be stated when the work is reported, not blurred.
- **The scaled-glyph null result** (§6.4) — the one place this plan could
  manufacture a false success in the same way the review did once.

## 10. Open questions

- Whether a font yielding `glyph.scale != 1` can be installed on this machine
  (§6.4). If not, that row stays unverified and says so.
- Whether M2 survives fix 2 at all (§5.1).
- Whether option C becomes worthwhile after this pass, with the unit tests from
  §6.2 in place to make it safe (§3).
