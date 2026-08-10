# PureCpu Parity Review — Design

Date: 2026-08-10
Branch: `update-optimization-rebased` (fork base: upstream `e723cf5`)

## Problem

The fork carries two large changes against upstream wezterm:

1. A pure-Rust font stack (`skrifa`, `harfrust`, `fontdb`) replacing FreeType/HarfBuzz/C COLR.
2. A `PureCpu` software renderer, for running wezterm without a GPU.

Both are unreviewed. The renderer's correctness is asserted only by daily use on one
machine, and it is unknown which GPU-backend capabilities it silently fails to draw.

## Goals

- A findings list of defects in the patch, independent of parity.
- A parity matrix for the GPU backend versus PureCpu, limited to the feature areas
  below, with each verdict backed by evidence rather than by reading code alone.

## Non-goals

Fixes are not part of this work. They are selected by the user after reading both
documents. Also excluded: macOS, Windows, Wayland, documentation, changelog entries,
and upstreamability. This is a personal fork run on Linux/X11.

## Parity scope

In scope:

- **Inline images** — sixel, iTerm2 protocol, animated GIFs.
- **Tab bar and window chrome** — fancy tab bar, window buttons, rounded corners,
  split dividers, borders.
- **Cursor and text animation** — cursor blink and easing, blinking text attributes,
  visual bell, text fade-in.

Known gaps, recorded but not investigated: `window_background_image`,
`window_background_opacity`, `text_background_opacity`, and background layer
blur/HSB tint.

## Primary risk hypothesis

`call_draw_purecpu` (`wezterm-gui/src/termwindow/render/purecpu.rs:154`) obtains
exactly one texture — the glyph cache atlas — and rasterizes every textured quad from
it. Any content the GPU path draws from a different texture will not render at all,
and no screenshot of working content would reveal it. Inline images are the prime
suspect. Phase 1 exists to settle this question, because it is the class of gap that
runtime comparison cannot find.

## Method

### Phase 1 — Audit (static)

Enumerate what the GPU path can emit: every quad producer, every texture source,
every blend mode. Then determine what PureCpu consumes. Output: the parity matrix
skeleton, with each row marked as confirmed-by-reading or needs-measurement.

### Phase 2 — Differential measurement

For the three in-scope areas only. Identical content is rendered under
`front_end = "OpenGL"` and `front_end = "PureCpu"`, each window captured and compared.

Verified feasible before writing this spec: on display `:20`, wezterm renders,
`import -window` captures correctly, and two consecutive captures of a settled frame
are pixel-identical (`compare -metric AE` = 0).

### Phase 3 — Fixes

Gated on user selection. Not specified here.

## Environment

A dedicated X server, isolated from the user's `:10` desktop, so test windows cannot
be disturbed by or disturb real work:

- `Xvnc :20`, 1280x1024, depth 24, `-localhost`, port 5920, dedicated auth file.
- `marco` as window manager, so chrome and decorations behave as on a real desktop.
- OpenGL is Mesa **llvmpipe** 4.5 — software rasterized. This is what makes a GPU
  reference available on a machine with no GPU.

Display `:20` and port 5920 were confirmed free; `:10`–`:14` belong to other sessions
on this shared machine and must not be touched.

## Comparison methodology

**The two backends will not produce bit-identical output, and asserting pixel equality
would produce noise, not findings.** llvmpipe blends in normalized float with its own
rounding; PureCpu's `blend_over` does its own sRGB/linear round trip. Antialiased
glyph edges will differ by small amounts everywhere.

Therefore:

1. **Calibrate a noise floor first.** Capture a plain-text frame under both backends
   and measure the difference. This establishes the expected per-pixel deviation and
   its spatial distribution.
2. **If the noise floor is not stable and small, stop.** A harness that cannot
   distinguish rendering differences from blending noise on plain text cannot do so on
   images either. That outcome is reported as a limitation, not worked around.
3. **Assert on structural difference** — content present in one backend and absent in
   the other, gross geometry errors, channel swaps — using a fuzz threshold derived
   from step 1.

### Determinism

Static captures pin: window geometry, font and size, colour scheme,
`cursor_blink_rate = 0`, and blinking text disabled. Animation parity is assessed by
comparing specific sampled states, never by diffing live frames.

### Mechanics

- Both instances launch with `--always-new-process` and distinct `--class` values.
- `WEZTERM_CONFIG_FILE` points each at a generated config pinning `front_end`.
- Window IDs come from `xwininfo -root -tree` (`xdotool` is not installed).
- Corpus content is emitted by a script: text sample, sixel via `convert logo: sixel:-`,
  and a hand-emitted iTerm2 `OSC 1337 File=` sequence. `wezterm imgcat` is unavailable
  because the mux CLI binary is not built.

## Verdict taxonomy

Each parity matrix row is one of:

- `parity` — matches within the calibrated noise floor.
- `degraded` — renders, but measurably wrong (offset, colour, missing antialiasing).
- `missing` — does not render.
- `known gap` — out of scope by decision, not investigated.

Every non-`known gap` verdict cites its evidence: a capture pair, or the code path
that makes the behaviour impossible.

## Artifacts

- `tools/purecpu-parity/` — harness scripts and corpus.
- `docs/purecpu-review/findings.md` — defect list.
- `docs/purecpu-review/parity-matrix.md` — parity matrix.

## Risks

- **llvmpipe is slow.** A full wezterm frame may take long enough that captures race
  the paint. Mitigated by settling the frame and verifying via double capture; proven
  to work in the smoke test.
- **The user's real config differs from the pinned test config,** so parity is
  established for the test configuration, not for every possible setting.
- **Animated content cannot be compared frame-for-frame** between backends. Sampling
  specific states is a weaker check than continuous comparison, and is stated as such.
