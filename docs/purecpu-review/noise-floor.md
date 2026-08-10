# PureCpu / OpenGL noise floor

**Task 3 (gate) — outcome: PROCEED for the terminal body with `FUZZ = 3`, STOP for the tab-bar strip.**

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
an unfocused one. wezterm renders two things differently when unfocused: **the
cursor becomes a hollow outline instead of a solid block**, and the fancy tab bar
switches to its inactive titlebar colours. The Task 2 pair
(`out/plain-gl.png`, `out/plain-cpu.png`) is focus-mismatched — the PureCpu
window held focus, the OpenGL window did not.

The effect is not subtle: it contributes an **exactly 8x20 solid rectangle**, one
full character cell, at the cursor position (x 1-9, y 166-186). That is precisely
the "whole cells" pattern the gate is supposed to stop on, and it is entirely an
artefact of how the captures were taken.

### Evidence

Two runs were made, differing only in launch order (under `marco` the
last-mapped window takes focus; `_NET_ACTIVE_WINDOW` confirmed the winner each
time):

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

The displacement is **per-glyph-run, not global**: rolling the whole tab strip by
one pixel makes the match worse (dx=0 gives AE 164, dx=-1 gives 387), so most of
the title is aligned and only some glyph runs land one pixel early. That is the
signature of a horizontal glyph-positioning/rounding difference in the fancy tab
bar text path, not of a shifted tab bar.

![tab bar zoom](../../tools/purecpu-parity/out/noise-floor-tabbar-zoom.png)

6x zoom of the tab title; OpenGL on top, PureCpu below. The `s`, `l` and `e`
stems sit one pixel to the left in the PureCpu rendering.

The difference is deterministic, reproducible, and independent of focus.

**Disposition:** this is a genuine finding and belongs to Task 5 (tab bar and
window chrome parity). It must **not** be dissolved into `FUZZ` — no fuzz value
absorbs it anyway. Task 5 should treat the tab strip as a region with a known
open defect rather than as a clean baseline.

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

| channel | AE | PAE |
|---|---|---|
| Red   | 2535 | 49344 |
| Green | 2529 | 49344 |
| Blue  | 2536 | 49344 |

Spread of 7 counts across 2535 (0.3%) — no channel bias. The signed means are
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

**`FUZZ = 3`** (percent), for the terminal body region only.

Reasoning, following the gate rule:

- Differences in the body concentrate on glyph edges — confirmed both by metrics
  (`PAE` = 1 LSB; zero differences in the empty background) and by looking at the
  heat map.
- The count collapses sharply: 2434 -> 0 between 0.25% and 0.5% fuzz. The
  smallest percentage at which it reaches zero is 0.5%; rounding that up to the
  nearest whole percent gives 1%, plus the specified 2 points of margin gives
  **3%**. There is generous headroom — the body count stays at 0 all the way to
  50%, so 3% is nowhere near a tuned-to-taste threshold.
- No channel bias, no uniform background shift, no whole-cell difference once
  the focus confound is removed.

**Scope limits that later tasks must respect:**

1. `FUZZ = 3` applies to the **terminal body (y >= 32) only**. The tab strip has
   an open defect (Finding 2) that no fuzz value hides; do not raise `FUZZ` to
   make the tab strip pass.
2. `FUZZ = 3` is only valid for **focus-matched captures**. Applied to a
   focus-mismatched pair it would let a whole-cell cursor difference through (the
   160-px cursor cluster survives 12% fuzz). The harness fix in Finding 1 is a
   precondition for every subsequent comparison task.
3. The floor was measured on static plain text. Tasks introducing inline images
   or animation should re-check that their content does not have a different
   floor before trusting 3%.

## Reproducing

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
export PARITY_XAUTH=<path to the :20 Xauthority>
./calibrate.sh                                    # Task 2 pair (focus-mismatched)
./calibrate.sh out/focus-B-gl.png out/focus-A-cpu.png   # focus-matched pair
```

`out/` is gitignored, so the PNGs referenced above are run artifacts rather than
committed files; the focus-matched captures were produced by launching the two
backends in each order and capturing both windows.
