# PureCpu / OpenGL noise floor

> **Status after the fix pass (Task 14).** This document is a *calibration
> record*, so its numbers are left as they were measured, against the pre-fix
> binary. Two things have changed since, and both are marked at the point of
> use below:
>
> 1. **Finding 2 is closed.** The fancy tab bar's 1 px glyph shift was fixed by
>    `773b845`; the tab strip now behaves like the body. See "Finding 2 —
>    closed" below for the re-run numbers.
> 2. **The noise floor itself is unchanged**, which is the more important of
>    the two: body `AE = 2434`, body `PAE = 257`, body `AE = 0` from 0.5% fuzz
>    upward, empty background `AE = 0`. The gate below therefore stands exactly
>    as written and needs no recalibration.
>
> Re-run against `wezterm-gui-6311e97` with the same two commands the
> "Reproducing" section prints.

**Task 3 (gate) — outcome: PROCEED for the terminal body with `FUZZ = 1` plus a
body `PAE <= 257` assertion, STOP for the tab-bar strip.**

Two problems surfaced during calibration, and neither may be absorbed into a
threshold. One is a defect in the *harness* that silently faked a whole-cell
difference; the other is a genuine *rendering* difference in the tab bar. Both
are written up below as findings. Once the harness defect is corrected and the
tab-bar finding is excluded from thresholding, the remaining difference in the
terminal body is a textbook noise floor and the rest of the review can proceed.

- Instrument: `tools/purecpu-parity/calibrate.sh`
- Corpus: `tools/purecpu-parity/corpus/plain.sh` (static plain text, cursor blink
  and text blink disabled)
- Backends: `OpenGL` on Mesa llvmpipe (no GPU on this machine) vs `PureCpu`
- Frame: 1000x693 = 693000 px; 32px fancy tab strip, 661px terminal body

---

## Finding 1 (harness defect) — window focus state was not controlled

Task 2 launched both windows at the same time and captured both. Only one X11
window can hold focus, so one capture was of a focused window and the other of
an unfocused one. When the window is unfocused **wezterm draws the cursor as a
hollow outline instead of a solid block** (`render/mod.rs:709-718` maps a block
cursor to `CursorShape::Default` only when `focused_and_active`, and
`customglyph.rs:5080-5100` fills the cell for `Default` but strokes an outline
for `SteadyBlock`). The Task 2 pair (`out/plain-gl.png`, `out/plain-cpu.png`) is
focus-mismatched — the PureCpu window held focus, the OpenGL window did not.

The cursor is the *only* thing that changes here. wezterm does have a separate
focus-dependent path for the fancy tab bar's titlebar colours, but under the
harness config `active_titlebar_bg` and `inactive_titlebar_bg` are both the
default `#333333`, so it produces no visible change: the measured tab-strip
contribution is **AE = 0** for both backends between their focused and unfocused
captures. The full 160-pixel delta is the cursor cell, and it lies in the
terminal body, not the tab strip.

The effect is not subtle: it contributes an **exactly 8x20 solid rectangle**, one
full character cell, at the cursor position (x 1-9, y 166-186). That is precisely
the "whole cells" pattern the gate is supposed to stop on, and it is entirely an
artefact of how the captures were taken.

### Evidence

Regenerate everything in this section with
`tools/purecpu-parity/focus-probe.sh`. It makes two runs, differing only in
launch order (under `marco` the last-mapped window takes focus;
`_NET_ACTIVE_WINDOW` confirmed the winner each time):

| run | launch order | focused window |
|-----|--------------|----------------|
| A   | gl, then cpu | cpu |
| B   | cpu, then gl | gl  |

Cross-comparing the four captures separates focus from backend cleanly:

| comparison | focus states | AE | AE @ fuzz 12% |
|---|---|---|---|
| run A: gl vs cpu | mismatched | 2782 | 277 |
| run B: gl vs cpu | mismatched | 2782 | 277 |
| gl(B) vs cpu(A)  | **both focused** | 2622 | 117 |
| gl(A) vs cpu(B)  | **both unfocused** | 2622 | 117 |
| gl(B) vs gl(A)   | same backend, focus differs | 160 | 160 |
| cpu(A) vs cpu(B) | same backend, focus differs | 160 | 160 |

The arithmetic closes exactly: 2622 + 160 = 2782. The 160-pixel cursor cell is
contributed by focus alone, and each backend on its own shows the *same*
160-pixel change when focus is taken away.

Split by region, that delta is entirely in the terminal body — the tab strip does
not move at all, confirming the titlebar-colour path contributes nothing here:

| same backend, focus differs | tab strip AE | body AE |
|---|---|---|
| OpenGL focused vs unfocused | 0 | 160 |
| PureCpu focused vs unfocused | 0 | 160 |

![cursor vs focus](../../tools/purecpu-parity/out/noise-floor-cursor-focus.png)

Left to right: OpenGL focused, OpenGL unfocused, PureCpu focused, PureCpu
unfocused. Both backends draw a solid block when focused and a hollow outline
when not. **PureCpu's cursor rendering is correct**; only the comparison was wrong.

### Required harness fix (Task 2 territory, not applied here)

Drivers must launch and capture **one window at a time**
(`launch` -> `find_window` -> `capture_settled` -> `kill_class` -> next backend),
so that every capture is taken while its window holds focus. Capturing two
concurrently-open windows is unsound for any comparison involving the cursor or
the fancy tab bar. `lib.sh` was not modified as part of this task; this is
recorded for the team lead to route.

Both backends reproduce their own output byte-identically across runs
(tab strip AE = 0 for gl(A) vs gl(B) and for cpu(A) vs cpu(B)), so once focus is
matched the captures are fully deterministic.

---

## Finding 2 (real rendering difference) — tab-bar title glyphs are one pixel left in PureCpu

With focus matched, 2622 pixels still differ. 2434 of them are in the terminal
body and vanish completely at 0.5% fuzz. The other **188 are in the 32px tab
strip, and no fuzz value removes them** — 43 pixels still differ at 50% fuzz.
They sit in a 30x12 box at (29,12), which is the tab title text "1: sleep".

This is not antialiasing noise. It is a **one-pixel horizontal displacement of
some glyph runs**. Taking the PureCpu image one pixel to the *left* of the
OpenGL image reproduces it exactly:

| region (y 12-23) | gl[x] vs cpu[x] | gl[x] vs cpu[x-1] |
|---|---|---|
| x 29-40 | AE = 100 | **AE = 0** |
| x 49-58 | AE = 64  | AE = 11 |

An AE of exactly 0 over a 12x12 block means those pixels are bit-identical once
shifted — a pure integer offset, not a rounding difference.

The displacement is **per-glyph-run, not global**. Sliding the whole title by a
global `dx` — comparing OpenGL at crop `62x12+8+12` against PureCpu at crop
`62x12+(8-dx)+12`, i.e. the full title band x 8-69, y 12-23 — is best at `dx=0`:

| dx | -2 | -1 | 0 | 1 | 2 |
|---|---|---|---|---|---|
| AE | 436 | 387 | **164** | 196 | 409 |

So most of the title is aligned and only some glyph runs land one pixel early.
That is the signature of a horizontal glyph-positioning/rounding difference in
the fancy tab bar text path, not of a shifted tab bar.

![tab bar zoom](../../tools/purecpu-parity/out/noise-floor-tabbar-zoom.png)

6x zoom of the tab title; OpenGL on top, PureCpu below. The `s`, `l` and `e`
stems sit one pixel to the left in the PureCpu rendering.

The difference is deterministic, reproducible, and independent of focus.

**Disposition:** this is a genuine finding and belongs to Task 5 (tab bar and
window chrome parity). It must **not** be dissolved into `FUZZ` — no fuzz value
absorbs it anyway. Task 5 should treat the tab strip as a region with a known
open defect rather than as a clean baseline.

### Finding 2 — CLOSED (Task 14), fixed in `773b845`

The cause was `findings.md` M1: destination coordinates were truncated with
`as i32` instead of rounded, so a quad whose destination edge is not integral
started up to one pixel early. The fancy tab bar computes element positions in
floats, which is why it showed the defect and the retro bar (ordinary monospace
cells on an integral grid) never did. The fix rounds.

Re-running `./focus-probe.sh` unmodified against `wezterm-gui-6311e97` — the
same script, the same corpus, the same two comparisons — now reports the
**opposite** result on the test that originally established the finding:

| region (y 12-23) | gl[x] vs cpu[x] | gl[x] vs cpu[x-1] |
|---|---|---|
| x 29-40 | **AE = 0** | AE = 88 |
| x 49-58 | **AE = 0** | AE = 69 |

The `AE = 0` has moved from the shifted alignment to the unshifted one. That is
the sharpest form the result can take: those glyph runs are now bit-identical
to OpenGL's, and it is the *shifted* comparison that is wrong. The global-`dx`
sweep agrees — `dx = 0` gives `AE = 0`, against 349 at ±1 and 432 at ±2, where
before the best value at `dx = 0` was 164 and nothing reached zero.

**This also removes the one thing that kept the tab strip out of the gate.**
The strip's fuzz-0 `AE` falls **188 → 24**, and it now reaches **0 from 0.5%
fuzz upward**, exactly like the body — where before, residuals there survived
every fuzz value up to 50%. Post-fix sweep, focus-matched, whole
frame / tab strip / body:

| fuzz | whole frame | tab strip | body |
|---|---|---|---|
| 0%    | 2458 | 24 | 2434 |
| 0.25% | 2458 | 24 | 2434 |
| 0.5% and above (to 50%) | **0** | **0** | **0** |

The focus arithmetic still closes exactly, which is the check that the harness
itself has not drifted: focus-mismatched 2618, focus-matched 2458, focus delta
160, and 2458 + 160 = 2618.

**Scope limit 1 in "Decision" below is therefore obsolete as a statement about
the current binary** — the tab strip no longer has an open defect that no fuzz
value hides. It is left in place because the rule it states ("do not raise
`FUZZ` to make the tab strip pass") is still the right rule, and because the
tab strip still has no independently calibrated noise floor of its own: chrome
pixels are graded on explicit per-region evidence, not on a threshold. See
`parity-matrix.md`, "Grading basis for chrome pixels".

---

## The noise floor itself (terminal body, focus matched)

Excluding the 32px tab strip, the picture is exactly what a well-behaved
software rasterizer should look like against llvmpipe.

| metric | value |
|---|---|
| AE   | 2434 px (0.368% of the 661000-px body) |
| RMSE | 15.3086 (0.000234) |
| PAE  | **257 (0.00392)** — one 8-bit step |
| MAE  | 0.911877 (0.0000139) |

`PAE = 257/65535` is one unit at 8-bit depth. **Every differing pixel in the
terminal body is off by exactly ±1 LSB.** There is no pixel anywhere in the body
where the two backends disagree by more than the smallest representable amount.

### Fuzz sweep (focus-matched pair)

| fuzz | whole frame | tab strip | body | body % |
|------|-------------|-----------|------|--------|
| 0%    | 2622 | 188 | 2434 | 0.3682% |
| 0.25% | 2622 | 188 | 2434 | 0.3682% |
| 0.5%  | 162  | 162 | **0** | 0.0000% |
| 0.75% | 162  | 162 | 0 | 0.0000% |
| 1%    | 157  | 157 | 0 | 0.0000% |
| 2%    | 149  | 149 | 0 | 0.0000% |
| 3%    | 147  | 147 | 0 | 0.0000% |
| 5%    | 136  | 136 | 0 | 0.0000% |
| 8%    | 127  | 127 | 0 | 0.0000% |
| 12%   | 117  | 117 | 0 | 0.0000% |
| 20%   | 90   | 90  | 0 | 0.0000% |
| 30%   | 75   | 75  | 0 | 0.0000% |
| 50%   | 43   | 43  | 0 | 0.0000% |

The body count does not merely "approach zero", it **reaches exactly zero at
0.5% fuzz and stays there**. Every residual in the whole-frame column from 0.5%
onward is Finding 2 in the tab strip.

### Spatial distribution

| region | AE |
|---|---|
| text rows (y 32-192) | 2434 |
| empty background below the text (y 192-693, 501000 px) | **0** |

Not one pixel differs in the empty background. The differences are confined to
rows containing text, and within those rows to antialiased glyph edges.

### Channel balance

Body region only, consistent with the 1-LSB headline above:

| channel | AE | PAE |
|---|---|---|
| Red   | 2347 | 257 |
| Green | 2341 | 257 |
| Blue  | 2348 | 257 |

(The whole-frame figures are AE 2535/2529/2536 with PAE 49344 in every channel;
that PAE is Finding 2 in the tab strip, not a body deviation.)

Spread of 7 counts across 2347 (0.3%) — no channel bias. The signed means are
symmetric as well (mean of `gl-cpu` = 4.14e-05, mean of `cpu-gl` = 4.44e-05), so
neither backend is systematically brighter; PureCpu's sRGB/linear round trip
rounds in both directions roughly equally.

### Heat maps

![all differences](../../tools/purecpu-parity/out/noise-floor-diff-matched.png)

Every differing pixel, focus matched. Red traces the outline of each glyph —
letters, digits, punctuation, ligatures, box-drawing and block characters, the
underline of `UNDER`, and the edges of the coloured background cells. It is edge
tracing throughout: no glyph is filled in solid, no cell is uniformly shifted,
and the entire lower two thirds of the frame is untouched.

![differences above the noise floor](../../tools/purecpu-parity/out/noise-floor-diff-matched-fuzz1.png)

The same comparison at 1% fuzz. The terminal body renders as plain unmarked
text — every body difference is gone. The only red left in the frame is on the
tab bar title, which is Finding 2.

For contrast, the uncorrected Task 2 pair is at
`tools/purecpu-parity/out/noise-floor-diff-mismatched.png`; it carries the
additional solid cursor block from Finding 1.

---

## Decision

Later tasks apply **three** checks to the terminal body, not one:

1. **`FUZZ = 1`** (percent) — body AE at 1% fuzz must be 0.
2. **body `PAE <= 257` at fuzz 0** — the sharp check.
3. **report raw fuzz-0 AE alongside** the fuzzed count, so sub-threshold drift
   stays visible instead of being silently zeroed.

All three are mechanised in `calibrate.sh`, which exits non-zero if (1) or (2)
fails. On the focus-matched pair it reports `PASS body PAE 257 <= 257` and
`PASS body AE at fuzz 1% is 0`; on the uncorrected Task 2 pair it fails both.

### Why the pixel-count threshold is 1 and not 3

The obvious reading of the gate rule — smallest fuzz where the count reaches
zero (0.5%), rounded up (1%), plus 2 points of margin — gives 3%. **That margin
is far too expensive here.** Fuzz is a distance threshold, so raising it does not
merely tolerate more antialiasing noise; it makes the comparison blind to *any*
deviation below the threshold, including a perfectly uniform one across the
entire body.

Measured by injecting a uniform brightness offset into the PureCpu body
(`convert body-cpu.png -evaluate Add $((n*257))`, Q16, so `n*257` is exactly
n/255 at 8-bit) and re-running the comparison:

| injected offset | AE @ fuzz 1% | AE @ fuzz 3% | body PAE @ fuzz 0 |
|---|---|---|---|
| +1/255 | 0 | 0 | 514 |
| +2/255 | 1492 | 0 | 771 |
| +3/255 | **660134** | 0 | 1028 |
| +4/255 | 661000 | 0 | 1285 |
| +5/255 | 661000 | 0 | 1542 |
| +6/255 | 661000 | **0** | 1799 |
| +7/255 | 661000 | 1492 | 2056 |
| +8/255 | 661000 | 660134 | 2313 |

At `FUZZ = 3` a uniform body-wide error of up to +6/255 — roughly 2.4% — reports
**zero differing pixels out of 661000**. That is precisely the defect class most
plausible in a new software rasteriser (an sRGB/linear round-trip or blend-weight
error), and precisely what Task 4 most needs to detect. `FUZZ = 1` catches such
an error from +3/255 upward and partially at +2/255, while still reporting 0 on
the real floor — it remains about 2.5x the measured 1-LSB floor, so it is margin
against noise, not a tuned-to-taste number.

### Why the PAE assertion matters more than either

The calibrated fact is not "few body pixels differ" but **"no body pixel differs
by more than one 8-bit step"** (`PAE = 257` at Q16). Asserting that directly is
strictly stronger than any fuzz threshold: the rightmost column above shows PAE
rising at *every* injected level, including the uniform +1/255 that both `FUZZ =
1` and `FUZZ = 3` miss entirely. A comparison whose body PAE exceeds 257 is a
finding regardless of how few pixels are involved.

The supporting evidence for the floor itself is unchanged: differences
concentrate on glyph edges (confirmed by metrics and by looking at the heat map),
the empty background is untouched, there is no channel bias, and no whole-cell
difference survives once the focus confound is removed.

**Scope limits that later tasks must respect:**

1. These thresholds apply to the **terminal body (y >= 32) only**. The tab strip
   has an open defect (Finding 2) that no fuzz value hides; do not raise `FUZZ`
   to make the tab strip pass.
2. They are only valid for **focus-matched captures**. On a focus-mismatched pair
   the whole-cell cursor difference survives 12% fuzz and drives body PAE to
   53456. The harness fix in Finding 1 is a precondition for every subsequent
   comparison task.
3. The floor was measured on static plain text. Tasks introducing inline images
   or animation should re-check that their content does not have a different
   floor before trusting these numbers.
4. **Never raise `FUZZ` to make a case pass.** If a comparison fails, the
   thresholds are doing their job; the table above is what a relaxed threshold
   costs.

## Reproducing

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
export PARITY_XAUTH=<path to the :20 Xauthority>

# Regenerate the four focus-{A,B}-{gl,cpu}.png captures, all Finding 1 and
# Finding 2 evidence tables, and every figure embedded in this document.
./focus-probe.sh

# Metrics, fuzz sweep and assertions.
./calibrate.sh out/focus-B-gl.png out/focus-A-cpu.png   # focus-matched -> exit 0
./calibrate.sh                                          # Task 2 pair   -> exit 1
```

`out/` is gitignored, so every PNG referenced above is a run artifact rather than
a committed file — hence `focus-probe.sh`, which regenerates all of them from
nothing. It is the one script that deliberately launches both backends at the
same time, because that is how the focus mismatch is produced on purpose in order
to measure it; production comparison drivers must capture one window at a time.

The injected-defect table is reproduced with:

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity/out
convert focus-B-gl.png  -crop 1000x661+0+32 +repage /tmp/bg.png
convert focus-A-cpu.png -crop 1000x661+0+32 +repage /tmp/bc.png
for n in 1 2 3 4 5 6 7 8; do
  convert /tmp/bc.png -evaluate Add $((n*257)) -depth 8 /tmp/inj.png
  echo "+$n/255 fuzz1=$(compare -metric AE -fuzz 1% /tmp/bg.png /tmp/inj.png null: 2>&1)" \
       "fuzz3=$(compare -metric AE -fuzz 3% /tmp/bg.png /tmp/inj.png null: 2>&1)" \
       "PAE=$(compare -metric PAE /tmp/bg.png /tmp/inj.png null: 2>&1)"
done
```
