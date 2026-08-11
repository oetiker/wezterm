# PureCpu Fix Pass Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair every defect the PureCpu parity review found, so that `front_end = "PureCpu"` on a GPU-less Linux/X11 machine loses as little as possible against `front_end = "OpenGL"`.

**Architecture:** Point fixes at each cited site, plus two extracted units that turn three *absences* into named, tested code: a pane-aware dirty-rect geometry unit (`termwindow/purecpu_dirty.rs`) and the nearest-neighbour atlas sampler PureCpu never had (`termwindow/render/purecpu_sampler.rs`). Everything else — the atlas ceiling, the subpixel mask, the animation dirty rects, the font-data guards — stays a point fix.

**Tech Stack:** Rust 2021, `cargo`, the `tools/purecpu-parity` shell harness (ImageMagick `compare`, `xwd`, Xvnc on display `:20`).

**Spec:** `docs/superpowers/specs/2026-08-11-purecpu-fixes-design.md`
**Review that found these defects:** `docs/purecpu-review/{README,findings,parity-matrix,noise-floor}.md`

## Global Constraints

Every task's requirements implicitly include this section. Read it before every task.

- **Never touch X displays `:10`–`:14`.** Other users share this machine. All testing is on `:20`.
- **The audible bell stays disabled in every run.** wezterm's default is `AudibleBell::SystemBeep`. `gen-config.sh` already emits `audible_bell = 'Disabled'` unconditionally (line 182, outside the `VISUAL_BELL` branch). **Every config in this work comes from `gen-config.sh`.** If a combination it refuses is needed, extend `gen-config.sh` — never hand-write around it.
- **Nothing you run may emit raw bytes to the terminal.** This is the same shared-machine rule as the bell, one layer lower, and it is the one that actually got broken: a subagent's stdout lands in the *user's own terminal*, so a single `0x07` byte anywhere in it beeps at a person who is not looking at your work. Three mechanical rules, no judgement required:
  - **Never `cat`, `head`, `tail` or otherwise print a binary file.** That includes every `.png` and `.xwd` under `tools/purecpu-parity/out/`. Inspect them with `file`, `identify`, `xxd | head`, or ImageMagick — never by dumping bytes.
  - **Never execute a corpus script directly.** `corpus/cursor.sh` rings the bell by design; it is meant to run *inside* a wezterm launched by the harness, never in your own shell. Read corpus scripts with the Read tool; run them only via `compare-case.sh` / `sample-case.sh`.
  - **Redirect every harness, build and wezterm command to a log file and Read it** — not just long or unfamiliar ones. `cmd > run.log 2>&1` then Read `run.log`. This is the rule that actually holds: it does not depend on any config being correct at the moment the command runs, which is precisely the assumption that failed. It also keeps output out of context.

  **Order matters:** the `check_bell_disabled` guard and this redirection discipline must be in place *before* the first command that launches wezterm. There is otherwise a window in which nothing enforces either.
- **Never more than 4 cores.** `cargo build -j4`, `cargo test -j4`.
- **Never run the 65536-atlas case.** 16 GiB on a shared ~25 GiB box. The 32768 case (4 GiB) is already measured; C1's fix is verified by unit test, not by allocating.
- **Shared harness files** (`lib.sh`, `gen-config.sh`, `compare-case.sh`, `sample-case.sh`) are load-bearing for the review's committed results. Any change must keep default output byte-identical, verified.
- **Never raise `FUZZ`** to make a case pass, and never grade a case against another case's number. Grade against a stated basis.
- **Sequential capture is mandatory.** Two concurrently-open windows leave one unfocused, and wezterm draws a hollow cursor when unfocused — a full character cell of difference that survives 12% fuzz. `focus-probe.sh` is the one deliberate exception.
- **`wezterm-gui` is a binary-only crate** (no `src/lib.rs`). The test command is `cargo test -p wezterm-gui --bin wezterm-gui`. `--lib` fails with "no library targets found" — this wasted a review round once.
- **Copy harness commands verbatim out of the rendered document, including leading env vars.** Several carry `PARITY_SETTLE_WARMUP=20`, which is load-bearing: without it the settle detector declares victory on two identical *blank* captures. A printed command nobody re-ran has been wrong five times in this project, twice in the reassuring direction.
- **Every verification answers: "could this run have failed, and what would that have looked like?"** A clean number from a broken instrument is this project's dominant failure mode — it has appeared nine distinct ways. Where cheap, put a control case in the same capture as the subject.
- **Subagents:** write the full report to a file; return only a short inline summary. Inline replies are frequently lost when a subagent terminates on idle.
- Do not commit `mise.toml` (untracked, intentional). `out/` and `.superpowers/` stay gitignored.
- `timeout N cat </dev/null` does **not** wait (cat hits EOF); use `timeout N tail -f /dev/null` — `pause` in `lib.sh` does this.
- `local a="$1" b="...$a..."` fails under `set -u`; split multi-variable `local`.

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `wezterm-gui/src/renderstate.rs` | Modify: atlas ceiling on the `PureCpu` arm | 2 |
| `wezterm-gui/src/termwindow/purecpu_dirty.rs` | **Create:** pane-aware dirty-rect geometry, pure functions + tests | 3 |
| `wezterm-gui/src/termwindow/mod.rs` | Modify: use the dirty unit, iterate all panes, resolved cursor shape, animation dirty rects | 4, 8, 9 |
| `wezterm-gui/src/termwindow/render/purecpu_sampler.rs` | **Create:** nearest-neighbour atlas sampler + pixel-coverage rules, pure functions + tests | 6 |
| `wezterm-gui/src/termwindow/render/purecpu.rs` | Modify: clamp fixes, use the sampler, per-channel subpixel blend | 5, 7, 10 |
| `wezterm-font/src/rasterizer/skrifa_rasterizer.rs` | Modify: COLR panic, CPAL panic, `unitsPerEm = 0` | 11 |
| `tools/purecpu-parity/lib.sh` | Modify: `check_bell_disabled` guard | 1 |
| `docs/purecpu-review/*.md` | Modify: verdict updates, stating the binary measured | 13 |

---

### Task 1: Prerequisites — reference binary, bell guard, scaled-glyph font

Nothing here changes renderer behaviour. It establishes the three things later verification depends on.

**Files:**
- Modify: `tools/purecpu-parity/lib.sh`
- Create: `tools/purecpu-parity/corpus/scaled-glyph.sh`

**Interfaces:**
- Produces: `check_bell_disabled <config-path>` in `lib.sh`; `/scratch/oetiker/wezterm-builds/wezterm-gui-prefix` (the pre-fix reference binary); a documented answer to whether a `glyph.scale != 1` font exists on this machine.

- [ ] **Step 1: Confirm the environment is alive**

```bash
export PARITY_XAUTH=/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20
DISPLAY=:20 XAUTHORITY=$PARITY_XAUTH xdpyinfo | grep dimensions
```

Expected: a dimensions line (`1280x1024` or similar). If this fails, `:20` is gone — recreate it per the review plan's "Environment setup" section (`docs/superpowers/plans/2026-08-10-purecpu-parity-review.md`) before continuing. Do **not** substitute another display; `:10`–`:14` belong to other people.

- [ ] **Step 2: Preserve the pre-fix reference binary**

The GL-invariant check in Task 13 needs a binary built from the tree *before* any fix. `wezterm-gui-rebased` is that binary and matches this branch, but later tasks must not overwrite it.

```bash
cp -n /scratch/oetiker/wezterm-builds/wezterm-gui-rebased \
      /scratch/oetiker/wezterm-builds/wezterm-gui-prefix
ls -la /scratch/oetiker/wezterm-builds/
```

Expected: `wezterm-gui-prefix` exists. `-n` means an existing copy is never clobbered.

- [ ] **Step 3: Write the failing test for the bell guard**

Add to `tools/purecpu-parity/lib.sh` a self-test at the bottom of the file's test block, or if none exists, verify by hand in Step 5. First write the guard's *call site expectation* by adding this to `compare-case.sh` and `sample-case.sh` immediately after their existing `check_no_config_error` call:

```bash
check_bell_disabled "$CONFIG"
```

- [ ] **Step 4: Run the harness to verify it fails**

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
FUZZ=1 ./compare-case.sh belltest "$PWD/corpus/text.sh"
```

Expected: FAIL with `check_bell_disabled: command not found`.

- [ ] **Step 5: Implement the guard in `lib.sh`**

```bash
# Fail closed if a config could let wezterm ring the system bell.  This box is
# shared and :20 is not ours alone; a sampling run with the default
# AudibleBell::SystemBeep beeps for the whole six seconds.  gen-config.sh always
# emits the Disabled line, so this only ever fires on a hand-written config —
# which is exactly the case that has no other guard.
check_bell_disabled() {
  local cfg="$1"
  if [ ! -r "$cfg" ]; then
    echo "check_bell_disabled: config not readable: $cfg" >&2
    return 1
  fi
  if ! grep -q "audible_bell *= *'Disabled'" "$cfg"; then
    echo "check_bell_disabled: $cfg does not disable the audible bell." >&2
    echo "  Every config in this work must come from gen-config.sh." >&2
    return 1
  fi
}
```

- [ ] **Step 6: Verify the guard passes on a generated config and fails on a hand-written one**

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
FUZZ=1 ./compare-case.sh belltest "$PWD/corpus/text.sh"     # expect: normal run, exit 0
printf 'return {}\n' > /tmp/claude-1003/-scratch-oetiker-wezterm/bad.lua
source ./lib.sh; check_bell_disabled /tmp/claude-1003/-scratch-oetiker-wezterm/bad.lua
```

Expected: the first passes; the second prints the error and returns 1. **Could this have failed silently?** Yes — if `grep` matched a commented-out line. Confirm the generated config's line is live by reading it: `grep -n audible_bell out/belltest-*.lua`.

- [ ] **Step 7: Verify shared-harness output is byte-identical**

The Global Constraints require `compare-case.sh`'s default output to be unchanged. Compare against the committed reference numbers:

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
git stash && FUZZ=1 ./compare-case.sh preguard "$PWD/corpus/text.sh" 2>&1 | tee /tmp/claude-1003/-scratch-oetiker-wezterm/pre.txt
git stash pop && FUZZ=1 ./compare-case.sh postguard "$PWD/corpus/text.sh" 2>&1 | tee /tmp/claude-1003/-scratch-oetiker-wezterm/post.txt
diff <(sed 's/preguard/CASE/g' /tmp/claude-1003/-scratch-oetiker-wezterm/pre.txt) \
     <(sed 's/postguard/CASE/g' /tmp/claude-1003/-scratch-oetiker-wezterm/post.txt)
```

Expected: no differences beyond the case name.

- [ ] **Step 8: Determine whether a `glyph.scale != 1` font exists on this machine**

The scaled fallback / bitmap glyph matrix row cannot be verified against a font that never triggers the path — a clean re-run would return a null result indistinguishable from parity, which is the exact mistake that produced this review's false `parity` on inline images once already.

```bash
fc-list | grep -iE "noto color emoji|emoji|unifont|terminus|misc-fixed" | head -20
fc-list :scalable=false family | sort -u | head -20   # bitmap-only fonts are the strongest candidates
```

Then confirm the path is actually reached, rather than assuming it from the font's name. Build with a temporary `log::info!` at the site that sets `glyph.scale`, run wezterm against a corpus that renders the candidate font, and grep the log for a `scale != 1` line.

- [ ] **Step 9: Record the answer in the corpus file**

Create `tools/purecpu-parity/corpus/scaled-glyph.sh` printing text in whichever font was confirmed. If **no** font on this machine triggers `glyph.scale != 1`, write the file anyway with a header comment stating that, and record it as the answer to spec §10's first open question. In that case the matrix row stays `by reading` and Task 13 says so explicitly — it does **not** get quietly marked fixed.

- [ ] **Step 10: Commit**

```bash
git add tools/purecpu-parity/lib.sh tools/purecpu-parity/compare-case.sh \
        tools/purecpu-parity/sample-case.sh tools/purecpu-parity/corpus/scaled-glyph.sh
git commit -m "harness: fail closed on a config that leaves the audible bell enabled"
```

---

### Task 2: C1 — atlas size ceiling on the PureCpu arm

`RenderContext::allocate_texture_atlas`'s `Glium` arm checks `caps.max_texture_size` and bails, which is what sends the GPU path into its `AllowImage::Scale(2)` downscale-and-retry fallback. The `PureCpu` arm has no cap and no fallible path, so a 1527-byte sixel drives the atlas to 32768×32768 = **4096 MiB, touched not reserved** (`Atlas::new` writes the whole rect). Measured peak RSS: 4170 MiB.

This fix also repairs a parity row: the 17000×64 sixel case is `degraded` only because GL downscales and PureCpu does not. With the ceiling in place PureCpu takes the same fallback.

**Files:**
- Modify: `wezterm-gui/src/renderstate.rs:113-117`

**Interfaces:**
- Produces: `PURECPU_MAX_TEXTURE_SIZE: usize` in `renderstate.rs`; `allocate_texture_atlas` returns `Err` for `size > PURECPU_MAX_TEXTURE_SIZE` on the `PureCpu` arm.

- [ ] **Step 1: Write the failing test**

Add to the bottom of `wezterm-gui/src/renderstate.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purecpu_atlas_rejects_oversize() {
        let ctx = RenderContext::PureCpu;
        // 32768 is the size a 17000x64 sixel drives the atlas to; it costs
        // 4 GiB of touched memory, so this must be refused rather than
        // allocated.  Do NOT change this test to allocate the buffer.
        let err = ctx
            .allocate_texture_atlas(32768)
            .expect_err("32768 must be refused");
        assert!(
            err.to_string().contains("larger than the max"),
            "unexpected error text: {err}"
        );
    }

    #[test]
    fn purecpu_atlas_accepts_ordinary_sizes() {
        // Proves the ceiling refuses rather than blanket-rejecting.  Uses a
        // small size deliberately: allocating PURECPU_MAX_TEXTURE_SIZE here
        // would touch 256 MiB on every test run for no extra coverage, and
        // this box is shared.
        let ctx = RenderContext::PureCpu;
        assert!(ctx.allocate_texture_atlas(1024).is_ok());
        assert!(PURECPU_MAX_TEXTURE_SIZE >= 4096, "ceiling too low for ordinary use");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-gui --bin wezterm-gui purecpu_atlas -- --nocapture
```

Expected: FAIL — `purecpu_atlas_rejects_oversize` panics on `expect_err` (the call succeeds today), and `PURECPU_MAX_TEXTURE_SIZE` does not resolve.

Note: `purecpu_atlas_accepts_ceiling` allocates 8192×8192×4 = 256 MiB. That is deliberate and within budget; it is what proves the ceiling is usable rather than merely restrictive.

- [ ] **Step 3: Implement the ceiling**

At the top of `wezterm-gui/src/renderstate.rs`, near the other constants:

```rust
/// Maximum atlas side length for the PureCpu renderer.
///
/// The Glium arm bails when the requested size exceeds the GPU's
/// max_texture_size, and that bail is what drives the caller into its
/// AllowImage::Scale(2) downscale-and-retry fallback.  The PureCpu arm had no
/// equivalent, so the atlas side was chosen from the decoded pixel width of an
/// inline image — i.e. from anything that reaches the tty.  A 1527-byte sixel
/// reached 32768, which is 4 GiB of *touched* memory (Atlas::new writes the
/// whole rect), and the next doubling is 16 GiB.
///
/// 8192 costs 256 MiB at RGBA and matches the smallest max_texture_size we are
/// likely to meet on the GL side, so PureCpu and OpenGL fall back at
/// comparable points.
pub const PURECPU_MAX_TEXTURE_SIZE: usize = 8192;
```

Then replace the `PureCpu` arm at `renderstate.rs:113-117`:

```rust
            Self::PureCpu => {
                use ::window::bitmaps::ImageTexture;
                if size > PURECPU_MAX_TEXTURE_SIZE {
                    anyhow::bail!(
                        "Cannot use a texture of size {} as it is larger \
                         than the max {} supported by the PureCpu renderer",
                        size,
                        PURECPU_MAX_TEXTURE_SIZE
                    );
                }
                let texture: Rc<dyn Texture2d> = Rc::new(ImageTexture::new(size, size));
                Ok(texture)
            }
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-gui --bin wezterm-gui purecpu_atlas -- --nocapture
```

Expected: 2 passed.

- [ ] **Step 5: Verify the caller reaches the scale fallback rather than dying**

A `bail!` is only correct if the caller handles it. Read the call site and confirm it maps the error into `AllowImage::Scale`, the same way the Glium bail is handled:

```bash
cd /scratch/oetiker/wezterm && grep -rn "allocate_texture_atlas" wezterm-gui/src/ | grep -v renderstate.rs
```

Read each hit. If any propagates the error to a fatal path instead of retrying at a smaller size, that is a defect **introduced by this fix** and must be repaired here, not deferred — a hard failure on a big sixel is not an improvement on a slow one.

- [ ] **Step 6: Commit**

```bash
git add wezterm-gui/src/renderstate.rs
git commit -m "fix(purecpu): cap the atlas size so a sixel cannot allocate 4 GiB (C1)"
```

---

### Task 3: The `dirty` unit — pane-aware rect geometry

Pure functions with tests. No caller changes yet; Task 4 wires them in. Splitting the two means a reviewer can reject the geometry without rejecting the integration.

Today's code at `mod.rs:1300-1375` computes every rect as `content_top + row * cell_h` with `x: 0, width: fb_width`. That is three defects: no `pos.top` (I2 vertical), no `pos.left` (I2 horizontal), and full-window-width bands that are wrong for any pane narrower than the window.

**Files:**
- Create: `wezterm-gui/src/termwindow/purecpu_dirty.rs`
- Modify: `wezterm-gui/src/termwindow/mod.rs` (add `mod purecpu_dirty;` alongside the existing submodule declarations)

**Interfaces:**
- Consumes: `crate::termwindow::render::purecpu::DirtyRect` only (fields `x, y, width, height: i32`). Deliberately **no** `mux` dependency — see `pane_placement` below. For Task 4's benefit: `mux::tab::PositionedPane` has `left, top, width, height: usize` in **cells**, plus `pixel_width, pixel_height: usize`, `is_active: bool`, `pane: Arc<dyn Pane>`.
- Produces:
  - `pub struct PanePlacement { pub origin_x: i32, pub origin_y: i32, pub cols: i32, pub rows: i32, pub cell_w: i32, pub cell_h: i32 }`
  - `pub fn pane_placement(left_cells: i32, top_cells: i32, cols: i32, rows: i32, content_left: i32, content_top: i32, cell_w: i32, cell_h: i32) -> PanePlacement` — plain values, **not** a `PositionedPane`, so the unit has no `mux` dependency and is testable without constructing an `Arc<dyn Pane>`. The caller unpacks the fields.
  - `pub fn row_band(p: &PanePlacement, row_in_viewport: i32) -> Option<DirtyRect>`
  - `pub fn cell_rect(p: &PanePlacement, row_in_viewport: i32, col: i32) -> Option<DirtyRect>`

- [ ] **Step 1: Write the failing tests**

Create `wezterm-gui/src/termwindow/purecpu_dirty.rs` with the tests **first**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// A pane at the window's top-left: 80x24 cells of 10x20 px, content
    /// origin at (5, 30) for padding and the tab bar.
    fn top_left() -> PanePlacement {
        PanePlacement {
            origin_x: 5,
            origin_y: 30,
            cols: 80,
            rows: 24,
            cell_w: 10,
            cell_h: 20,
        }
    }

    /// The right-hand pane of a vertical split: 40 cells across, starting at
    /// cell column 40 and row 0.
    fn split_right() -> PanePlacement {
        PanePlacement {
            origin_x: 5 + 40 * 10,
            origin_y: 30,
            cols: 40,
            rows: 24,
            cell_w: 10,
            cell_h: 20,
        }
    }

    /// The bottom pane of a horizontal split: starts at cell row 12.
    fn split_bottom() -> PanePlacement {
        PanePlacement {
            origin_x: 5,
            origin_y: 30 + 12 * 20,
            cols: 80,
            rows: 12,
            cell_w: 10,
            cell_h: 20,
        }
    }

    #[test]
    fn row_band_at_origin() {
        let r = row_band(&top_left(), 3).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (5, 30 + 60, 800, 20));
    }

    #[test]
    fn row_band_honours_pos_left_and_width() {
        // I2, horizontal axis: the band must start at the pane's left edge and
        // stop at its right edge, not span the window.
        let r = row_band(&split_right(), 3).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (405, 90, 400, 20));
    }

    #[test]
    fn row_band_honours_pos_top() {
        // I2, vertical axis: row 0 of the bottom pane is not row 0 of the window.
        let r = row_band(&split_bottom(), 0).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (5, 270, 800, 20));
    }

    #[test]
    fn row_band_rejects_rows_outside_the_viewport() {
        assert!(row_band(&top_left(), -1).is_none());
        assert!(row_band(&top_left(), 24).is_none());
        assert!(row_band(&split_bottom(), 12).is_none());
    }

    #[test]
    fn cell_rect_honours_both_offsets() {
        let r = cell_rect(&split_right(), 2, 3).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (405 + 30, 30 + 40, 10, 20));
    }

    #[test]
    fn cell_rect_rejects_columns_outside_the_pane() {
        assert!(cell_rect(&split_right(), 2, -1).is_none());
        assert!(cell_rect(&split_right(), 2, 40).is_none());
    }

    #[test]
    fn placement_converts_cell_offsets_to_pixels() {
        // PositionedPane.left/top are in CELLS, not pixels.  Multiplying by
        // the cell size is the whole job of this function, and skipping it is
        // I2 reintroduced with extra steps.
        let p = pane_placement(
            /* left_cells */ 40,
            /* top_cells */ 12,
            /* cols */ 40,
            /* rows */ 12,
            /* content_left */ 5,
            /* content_top */ 30,
            /* cell_w */ 10,
            /* cell_h */ 20,
        );
        assert_eq!(p.origin_x, 405, "left_cells was not scaled by cell_w");
        assert_eq!(p.origin_y, 270, "top_cells was not scaled by cell_h");
        assert_eq!((p.cols, p.rows), (40, 12));
    }

    #[test]
    fn placement_at_the_window_origin_is_the_content_origin() {
        let p = pane_placement(0, 0, 80, 24, 5, 30, 10, 20);
        assert_eq!((p.origin_x, p.origin_y), (5, 30));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-gui --bin wezterm-gui purecpu_dirty -- --nocapture
```

Expected: FAIL to compile — `PanePlacement`, `row_band`, `cell_rect` are undefined.

- [ ] **Step 3: Implement the unit**

Above the test module in `wezterm-gui/src/termwindow/purecpu_dirty.rs`:

```rust
//! Pane-aware dirty-rect geometry for the PureCpu renderer.
//!
//! Every rect PureCpu repaints is derived here.  The pane's origin is an
//! explicit argument rather than an ambient assumption, which is the whole
//! point of the module: the previous inline version computed rects from the
//! window's content origin alone, so any pane that was not at the window's
//! top-left was repainted at the wrong place — or, because the rects then
//! matched nothing the paint pass drew, not repainted at all (findings I2).

use crate::termwindow::render::purecpu::DirtyRect;

/// Where a pane sits in the window, in pixels, with its cell metrics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanePlacement {
    /// Pixel x of the pane's left edge, including window padding and border.
    pub origin_x: i32,
    /// Pixel y of the pane's top edge, including the tab bar, padding and border.
    pub origin_y: i32,
    /// Pane width in cells.
    pub cols: i32,
    /// Pane height in cells.
    pub rows: i32,
    pub cell_w: i32,
    pub cell_h: i32,
}

/// Convert a pane's cell-space position within the tab into pixel-space
/// placement within the window.
///
/// Takes plain values rather than a `PositionedPane` so this module has no
/// `mux` dependency and can be tested without constructing an `Arc<dyn Pane>`.
/// `PositionedPane::left`/`top`/`width`/`height` are all in **cells**.
#[allow(clippy::too_many_arguments)]
pub fn pane_placement(
    left_cells: i32,
    top_cells: i32,
    cols: i32,
    rows: i32,
    content_left: i32,
    content_top: i32,
    cell_w: i32,
    cell_h: i32,
) -> PanePlacement {
    PanePlacement {
        origin_x: content_left + left_cells * cell_w,
        origin_y: content_top + top_cells * cell_h,
        cols,
        rows,
        cell_w,
        cell_h,
    }
}

/// One full-width-*within-the-pane* band for a viewport row.
/// Returns None when the row is outside the pane.
pub fn row_band(p: &PanePlacement, row_in_viewport: i32) -> Option<DirtyRect> {
    if row_in_viewport < 0 || row_in_viewport >= p.rows {
        return None;
    }
    Some(DirtyRect {
        x: p.origin_x,
        y: p.origin_y + row_in_viewport * p.cell_h,
        width: p.cols * p.cell_w,
        height: p.cell_h,
    })
}

/// One cell — used for cursor movement.
/// Returns None when the cell is outside the pane.
pub fn cell_rect(p: &PanePlacement, row_in_viewport: i32, col: i32) -> Option<DirtyRect> {
    if row_in_viewport < 0 || row_in_viewport >= p.rows || col < 0 || col >= p.cols {
        return None;
    }
    Some(DirtyRect {
        x: p.origin_x + col * p.cell_w,
        y: p.origin_y + row_in_viewport * p.cell_h,
        width: p.cell_w,
        height: p.cell_h,
    })
}
```

Add `mod purecpu_dirty;` to `wezterm-gui/src/termwindow/mod.rs` beside the existing submodule declarations. `DirtyRect` already derives `Clone, Debug`; add `PartialEq, Eq` to it so the tests can compare — that derive change is the only edit to `purecpu.rs` in this task.

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-gui --bin wezterm-gui purecpu_dirty -- --nocapture
```

Expected: 8 passed.

- [ ] **Step 5: Commit**

```bash
git add wezterm-gui/src/termwindow/purecpu_dirty.rs wezterm-gui/src/termwindow/mod.rs \
        wezterm-gui/src/termwindow/render/purecpu.rs
git commit -m "refactor(purecpu): extract pane-aware dirty-rect geometry with tests"
```

---

### Task 4: I1 + I2 — walk every pane, through the new unit

**Files:**
- Modify: `wezterm-gui/src/termwindow/mod.rs:1272-1375`

**Interfaces:**
- Consumes: `purecpu_dirty::{pane_placement, row_band, cell_rect, PanePlacement}` from Task 3; `self.get_panes_to_render() -> Vec<PositionedPane>` (`mod.rs:3879`).
- Produces: `state.dirty_pixel_rects` covering **all** panes, in window coordinates.

- [ ] **Step 1: Replace the single-pane block**

Replace the body of the `if !force_full { ... }` block at `mod.rs:1272-1375`. The existing `cell_w`, `cell_h`, `padding_left`, `padding_top`, `tab_bar_height`, `top_bar_height`, `border` and `content_top` computations stay exactly as they are — they establish the window's content origin, which is still needed. What changes is that `content_top`/`content_left` become the origin passed to `pane_placement`, and the loop runs over every pane:

```rust
        // Compute dirty pixel regions if not doing full repaint
        if !force_full {
            let cell_w = self.render_metrics.cell_size.width as i32;
            let cell_h = self.render_metrics.cell_size.height as i32;
            let (padding_left, padding_top) = self.padding_left_top();

            // Match the paint pass coordinate system (render/pane.rs):
            //   top_pixel_y = top_bar_height + padding_top + border.top
            //   left_pixel_x = padding_left + border.left
            let tab_bar_height = if self.show_tab_bar {
                self.tab_bar_pixel_height().unwrap_or(0.)
            } else {
                0.
            };
            let top_bar_height = if self.config.tab_bar_at_bottom {
                0.0
            } else {
                tab_bar_height
            };
            let border = self.get_os_border();
            let content_top = (top_bar_height + padding_top + border.top.get() as f32) as i32;
            let content_left = (padding_left + border.left.get() as f32) as i32;

            // I1: walk the same pane list paint_impl renders, not just the
            // active pane.  A build running in one pane while you read in
            // another is the ordinary reason to use splits, and with only the
            // active pane consulted the other one never repaints at all.
            let panes = self.get_panes_to_render();

            let mut collected: Vec<crate::termwindow::render::purecpu::DirtyRect> = vec![];
            let mut active_seqno: Option<SequenceNo> = None;
            let mut active_cursor: Option<(StableRowIndex, usize)> = None;

            for pos in &panes {
                let placement = crate::termwindow::purecpu_dirty::pane_placement(
                    pos.left as i32,
                    pos.top as i32,
                    pos.width as i32,
                    pos.height as i32,
                    content_left,
                    content_top,
                    cell_w,
                    cell_h,
                );

                let dims = pos.pane.get_dimensions();
                let viewport = self
                    .get_viewport(pos.pane.pane_id())
                    .unwrap_or(dims.physical_top);
                let visible_range =
                    viewport..viewport + dims.viewport_rows as StableRowIndex;
                let dirty_rows = pos.pane.get_changed_since(visible_range, last_seqno);

                for range in dirty_rows.iter() {
                    for stable_row in range.clone() {
                        let row_in_viewport = (stable_row - viewport) as i32;
                        if let Some(rect) =
                            crate::termwindow::purecpu_dirty::row_band(&placement, row_in_viewport)
                        {
                            collected.push(rect);
                        }
                    }
                }

                if pos.is_active {
                    let cursor = pos.pane.get_cursor_position();
                    active_seqno = Some(pos.pane.get_current_seqno());
                    active_cursor = Some((cursor.y, cursor.x));

                    let cursor_moved = match (last_cursor_y, last_cursor_x) {
                        (Some(prev_y), Some(prev_x)) => prev_y != cursor.y || prev_x != cursor.x,
                        _ => true, // first frame: treat as moved
                    };

                    if cursor_moved {
                        if let (Some(prev_y), Some(prev_x)) = (last_cursor_y, last_cursor_x) {
                            if let Some(rect) = crate::termwindow::purecpu_dirty::cell_rect(
                                &placement,
                                (prev_y - viewport) as i32,
                                prev_x as i32,
                            ) {
                                collected.push(rect);
                            }
                        }
                        if let Some(rect) = crate::termwindow::purecpu_dirty::cell_rect(
                            &placement,
                            (cursor.y - viewport) as i32,
                            cursor.x as i32,
                        ) {
                            collected.push(rect);
                        }
                    }
                }
            }

            let state = self.purecpu_state.as_mut().unwrap();
            state.dirty_pixel_rects.clear();
            state.dirty_pixel_rects.extend(collected);
            if let Some(seqno) = active_seqno {
                state.last_seqno = seqno;
            }
            if let Some((cy, cx)) = active_cursor {
                state.last_cursor_y = Some(cy);
                state.last_cursor_x = Some(cx);
            }
        }
```

The cursor-blink block that currently follows at `mod.rs:1377-1400` stays where it is; it is Task 8's subject. Keep its `placement` needs in mind — it marks the cursor cell dirty and must use `cell_rect` with the **active** pane's placement, not the window origin. If it currently computes its own coordinates inline, convert it to `cell_rect` in this task and note that in the commit message.

- [ ] **Step 2: Build**

```bash
cd /scratch/oetiker/wezterm && cargo build -j4 --release -p wezterm-gui 2>&1 | tail -20
cp target/release/wezterm-gui /scratch/oetiker/wezterm-builds/wezterm-gui-fixed
```

Note: `CARGO_TARGET_DIR` is set globally; adjust the `cp` source to `$CARGO_TARGET_DIR/release/wezterm-gui` if the path above does not exist.

Expected: builds clean. `last_seqno`, `last_cursor_y`, `last_cursor_x` are read earlier in the function — confirm the borrow checker is satisfied and that no read-after-move was introduced.

- [ ] **Step 3: Verify I1 — a non-active pane repaints**

Use the reproduce driver `findings.md` lists under I2 (it covers both). Copy it verbatim from the rendered document.

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
export PARITY_XAUTH=/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20
export WEZTERM_BIN=/scratch/oetiker/wezterm-builds/wezterm-gui-fixed
# then the I1/I2 driver as printed in docs/purecpu-review/findings.md
```

Expected: the left pane's progress bar advances instead of freezing mid-cycle. Compare against `tools/purecpu-parity/out/splitright-cpu.png`, the screenshot of the frozen state.

**Could this run have failed and looked like success?** Yes, three ways: the driver may not have produced a split at all (check the capture shows two panes); the non-active pane may have had no output (check the bar's source is actually running); a full repaint may have been forced by something incidental (selection change, config reload), which repairs the symptom without the fix. Rule the third out by confirming `purecpu.full_repaint.rate` is not firing every frame, or by keeping the window idle apart from the non-active pane's output.

- [ ] **Step 4: Verify I2 — the active pane repaints away from the top-left**

Run the same driver's second arm, with the active pane at a non-zero `pos.top` and again at a non-zero `pos.left`. Both axes must be exercised; the review measured both.

Expected: output appears in the active pane in both configurations.

- [ ] **Step 5: Verify no regression in the single-pane case**

The whole matrix was measured with one full-width pane. If this change breaks that, it breaks everything.

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
FUZZ=1 WEZTERM_BIN=/scratch/oetiker/wezterm-builds/wezterm-gui-fixed \
  ./compare-case.sh text-after-i1i2 "$PWD/corpus/text.sh"
```

Expected: body `PAE` at or below 257, matching the committed noise floor. A *higher* number is a regression introduced here.

- [ ] **Step 6: Commit**

```bash
git add wezterm-gui/src/termwindow/mod.rs
git commit -m "fix(purecpu): repaint every pane, at its own origin (I1, I2)"
```

---

### Task 5: M7 + M6 — clamp instead of skip

Two independent "silently not drawn" bugs of the same shape: a bounds check that skips the work instead of clamping it.

**Files:**
- Modify: `wezterm-gui/src/termwindow/render/purecpu.rs:102-115` (`clear_rect`, M7)
- Modify: `wezterm-gui/src/termwindow/render/purecpu.rs:498-519` (the band present loop, M6)

**Interfaces:**
- Produces: no signature changes.

- [ ] **Step 1: Write the failing tests**

Add to the existing `mod tests` in `purecpu.rs`:

```rust
    #[test]
    fn clear_rect_survives_a_rect_entirely_right_of_the_framebuffer() {
        // M7, the sharp end: x0 is NOT clamped to fb_w while x1 IS, so a rect
        // starting past the right edge yields x0 > x1 and the fill slices
        // fb[40..16] — a panic, not a skipped row.  A stray dirty rect from a
        // resize race takes the process down.
        let fb_w = 4usize;
        let fb_h = 3usize;
        let mut fb = vec![0xFFu8; fb_w * fb_h * 4];
        let rect = DirtyRect { x: 10, y: 0, width: 2, height: 1 };
        clear_rect(&mut fb, fb_w, fb_h, &rect);
        // Nothing is on screen, so nothing may be cleared.
        assert!(fb.iter().all(|&b| b == 0xFF), "off-screen rect touched pixels");
    }

    #[test]
    fn clear_rect_survives_a_negative_extent() {
        // The other half of M7: (rect.x + rect.width) is cast to usize AFTER
        // .min(), so a negative sum wraps to a huge x1 rather than clamping to
        // zero.  The `row_end <= fb.len()` guard then silently skips the row —
        // which is the "silently not drawn" symptom the finding names.
        let fb_w = 4usize;
        let fb_h = 3usize;
        let mut fb = vec![0xFFu8; fb_w * fb_h * 4];
        let rect = DirtyRect { x: -10, y: 0, width: 2, height: 1 };
        clear_rect(&mut fb, fb_w, fb_h, &rect);
        assert!(fb.iter().all(|&b| b == 0xFF), "off-screen rect touched pixels");
    }

    #[test]
    fn clamp_band_keeps_the_visible_rows_of_an_overhanging_band() {
        // M6: a band whose tail ran past the bottom was dropped WHOLE, so
        // every row in it went unpresented — including the visible ones.
        assert_eq!(clamp_band(90, 20, 100), Some((90, 10)));
    }

    #[test]
    fn clamp_band_passes_a_fully_visible_band_through() {
        assert_eq!(clamp_band(10, 20, 100), Some((10, 20)));
    }

    #[test]
    fn clamp_band_rejects_a_band_entirely_below_the_screen() {
        assert_eq!(clamp_band(100, 20, 100), None);
        assert_eq!(clamp_band(120, 20, 100), None);
    }

    #[test]
    fn clamp_band_clips_a_negative_origin_to_zero() {
        // A negative band origin must present from row 0, not at a negative
        // offset — the old code did `band.y.max(0)` for the slice but passed
        // the unclamped `band.y` as the destination, so the two disagreed.
        assert_eq!(clamp_band(-5, 20, 100), Some((0, 15)));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-gui --bin wezterm-gui clear_rect_survives clamp_band -- --nocapture
```

Expected:
- `clear_rect_survives_a_rect_entirely_right_of_the_framebuffer` **panics** — "range start index 40 out of range" or "slice index starts at 40 but ends at 16". That panic is the defect.
- `clear_rect_survives_a_negative_extent` fails or panics.
- All four `clamp_band` tests fail to compile — `clamp_band` does not exist yet.

- [ ] **Step 3: Fix M7 in `clear_rect`**

Two distinct defects, both from mixing `i32` arithmetic with `usize` casts: `x0`/`y0` are never clamped to the framebuffer's far edge while `x1`/`y1` are, so `x0 > x1` is reachable and slices backwards; and `(rect.x + rect.width)` is cast to `usize` *after* `.min()`, so a negative sum wraps to a huge value instead of clamping to zero.

Do the whole computation in `i32`, clamp both ends to the framebuffer, and only then cast:

```rust
/// Clear a rectangular region in the framebuffer to black (zero)
fn clear_rect(fb: &mut [u8], fb_w: usize, fb_h: usize, rect: &DirtyRect) {
    // Clamp BOTH ends in i32 before casting.  Casting a negative i32 to usize
    // wraps to a huge value, and clamping only the far end lets x0 exceed x1,
    // which slices backwards and panics (findings M7).
    let x0 = rect.x.clamp(0, fb_w as i32);
    let y0 = rect.y.clamp(0, fb_h as i32);
    let x1 = rect.x.saturating_add(rect.width).clamp(0, fb_w as i32);
    let y1 = rect.y.saturating_add(rect.height).clamp(0, fb_h as i32);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let (x0, y0, x1, y1) = (x0 as usize, y0 as usize, x1 as usize, y1 as usize);

    for y in y0..y1 {
        let row_start = (y * fb_w + x0) * 4;
        let row_end = (y * fb_w + x1) * 4;
        debug_assert!(
            row_end <= fb.len(),
            "clear_rect: row_end {row_end} exceeds framebuffer {} — \
             fb_w/fb_h disagree with the buffer length",
            fb.len()
        );
        let row_end = row_end.min(fb.len());
        if row_start < row_end {
            fb[row_start..row_end].fill(0);
        }
    }
}
```

- [ ] **Step 4: Fix M6 — extract `clamp_band`, then use it in the present loop**

The band arithmetic is inline inside `call_draw_purecpu`, where it cannot be tested. Extract it as a free function next to `coalesce_to_bands`:

```rust
/// Clamp a band to the visible rows, returning `(y, height)` in framebuffer
/// rows, or None when nothing of it is on screen.
///
/// The present loop used to `continue` whenever a band's tail ran past the
/// bottom of the window, which dropped the band WHOLE — every row in it went
/// unpresented, including the visible ones (findings M6).  It also computed
/// the slice from `band.y.max(0)` but passed the unclamped `band.y` as the
/// destination row, so the two disagreed for a negative origin.
fn clamp_band(band_y: i32, band_h: i32, screen_height: i32) -> Option<(usize, usize)> {
    if band_h <= 0 || screen_height <= 0 {
        return None;
    }
    let y0 = band_y.max(0);
    let y1 = band_y.saturating_add(band_h).min(screen_height);
    if y1 <= y0 {
        return None;
    }
    Some((y0 as usize, (y1 - y0) as usize))
}
```

Then the present loop at `purecpu.rs:498-519` becomes:

```rust
            let bands = coalesce_to_bands(&effective_dirty, screen_width);
            for band in &bands {
                let Some((y, h)) = clamp_band(band.y, band.height, screen_height as i32) else {
                    continue;
                };
                let offset = y * fb_w * 4;
                let size = h * fb_w * 4;
                if offset + size <= state.frame_buffer.len() {
                    window.present_software_frame_region(
                        &state.frame_buffer[offset..offset + size],
                        screen_width,
                        h as u32,
                        0,
                        y as i16,
                    )?;
                }
            }
```

Note the destination row argument changed from `band.y as i16` to `y as i16`: they differ exactly when `band.y` is negative, which previously presented the clamped slice at an unclamped offset.

- [ ] **Step 5: Run the tests to verify they pass**

```bash
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-gui --bin wezterm-gui -- --nocapture 2>&1 | tail -20
```

Expected: all tests pass, including the 11 pre-existing ones.

- [ ] **Step 6: Commit**

```bash
git add wezterm-gui/src/termwindow/render/purecpu.rs
git commit -m "fix(purecpu): clamp instead of dropping out-of-range clears and bands (M6, M7)"
```

---

### Task 6: The `sampler` unit — nearest-neighbour atlas sampling

Pure functions with tests; Task 7 wires them in.

The GPU samples the atlas with `MagnifySamplerFilter::Nearest` + `MinifySamplerFilter::Nearest` + `SamplerWrapFunction::Clamp` (`draw.rs:215-218`) for glyphs, colour emoji and images. `atlas_linear_sampler` is used in exactly one shader branch — `o_has_color == 2.0`, the window background attachment (`glyph-frag.glsl:121`) — which is out of scope. **Nearest is therefore not an approximation of the GPU; it is the GPU.** Bit-exact parity on scaled quads is reachable.

Two rules must match GL exactly:

1. **Pixel coverage.** GL fills a pixel when its centre lies inside the quad: pixel `x` is covered when `edge0 <= x + 0.5 < edge1`, so the first covered pixel is `ceil(edge0 - 0.5)` and the first uncovered one is `ceil(edge1 - 0.5)`. This replaces today's truncating `as i32` and is M1's fix.
2. **Texel selection.** The texcoord is interpolated linearly across the quad and then floored to a texel. For destination pixel `x`, the fraction along the quad is `u = (x + 0.5 - edge0) / (edge1 - edge0)`, and the texel is `floor(src_origin + u * src_extent)`, clamped to the atlas.

**Files:**
- Create: `wezterm-gui/src/termwindow/render/purecpu_sampler.rs`
- Modify: `wezterm-gui/src/termwindow/render/mod.rs` (add `pub mod purecpu_sampler;`)

**Interfaces:**
- Produces:
  - `pub fn cover_start(edge: f32) -> i32`
  - `pub fn cover_end(edge: f32) -> i32`
  - `pub struct Axis { src_origin: f32, src_extent: f32, dest_origin: f32, dest_extent: f32, src_limit: i32 }`
  - `pub fn Axis::new(src_origin: f32, src_extent: f32, dest_origin: f32, dest_extent: f32, src_limit: i32) -> Axis`
  - `pub fn Axis::texel(&self, dest_pixel: i32) -> i32`

- [ ] **Step 1: Write the failing tests**

Create `wezterm-gui/src/termwindow/render/purecpu_sampler.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_matches_gl_pixel_centre_rule() {
        // A pixel is covered when its centre lies inside the quad.
        assert_eq!(cover_start(10.0), 10);
        assert_eq!(cover_start(10.4), 10);
        assert_eq!(cover_start(10.5), 10); // centre of pixel 10 is exactly on the edge
        assert_eq!(cover_start(10.6), 11);
        assert_eq!(cover_end(20.0), 20);
        assert_eq!(cover_end(20.5), 20);
        assert_eq!(cover_end(20.6), 21);
    }

    #[test]
    fn coverage_rounds_rather_than_truncates() {
        // M1: truncation displaced sub-pixel-positioned quads one pixel
        // left/up, which is the fancy tab bar's bit-exact 1px shift.
        assert_eq!(cover_start(99.7), 100);
        assert_ne!(cover_start(99.7), 99.7_f32 as i32);
    }

    #[test]
    fn identity_when_source_and_dest_extents_match() {
        // THE property that protects the four bit-identical parity rows.
        // A 1:1 quad must select exactly the texels the old blit did.
        let a = Axis::new(64.0, 10.0, 100.0, 10.0, 4096);
        for k in 0..10 {
            assert_eq!(a.texel(100 + k), 64 + k, "1:1 mapping broken at k={k}");
        }
    }

    #[test]
    fn magnifies_two_to_one() {
        // A 5px source drawn across 10px: each source texel covers two
        // destination pixels.
        let a = Axis::new(0.0, 5.0, 0.0, 10.0, 4096);
        let got: Vec<i32> = (0..10).map(|x| a.texel(x)).collect();
        assert_eq!(got, vec![0, 0, 1, 1, 2, 2, 3, 3, 4, 4]);
    }

    #[test]
    fn minifies_two_to_one() {
        // A 10px source drawn across 5px: every other texel is dropped,
        // which is what Nearest minification does.
        let a = Axis::new(0.0, 10.0, 0.0, 5.0, 4096);
        let got: Vec<i32> = (0..5).map(|x| a.texel(x)).collect();
        assert_eq!(got, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn clamps_at_the_atlas_edge() {
        // SamplerWrapFunction::Clamp.  Reading outside the atlas must return
        // the edge texel, never panic and never wrap to the far side.
        let a = Axis::new(4090.0, 10.0, 0.0, 10.0, 4096);
        assert_eq!(a.texel(9), 4095);
        let b = Axis::new(-2.0, 4.0, 0.0, 4.0, 4096);
        assert_eq!(b.texel(0), 0);
    }

    #[test]
    fn degenerate_extents_do_not_panic() {
        let a = Axis::new(10.0, 0.0, 5.0, 0.0, 4096);
        let _ = a.texel(5);
        let b = Axis::new(10.0, 4.0, 5.0, 0.0, 4096);
        let _ = b.texel(5);
    }

    #[test]
    fn doubled_width_line_spans_the_doubled_cell() {
        // DECDWL: a 10px glyph drawn into a 20px cell must read every source
        // column across the full destination, not crop at 10px.  Cropping is
        // what made double-width lines render at base size.
        let a = Axis::new(0.0, 10.0, 0.0, 20.0, 4096);
        assert_eq!(a.texel(0), 0);
        assert_eq!(a.texel(19), 9);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-gui --bin wezterm-gui purecpu_sampler -- --nocapture
```

Expected: FAIL to compile — `cover_start`, `cover_end`, `Axis` are undefined.

- [ ] **Step 3: Implement the unit**

```rust
//! The atlas sampler PureCpu never had.
//!
//! The rasteriser used to blit textured quads 1:1 and crop them
//! (`purecpu.rs:346`, findings I5), so anything drawn at a size other than its
//! atlas sprite's — non-native inline images, double-width and double-height
//! lines, scaled fallback and bitmap glyphs — was cropped instead of scaled.
//!
//! This module reproduces what the GPU's fixed function does for free:
//! `MagnifySamplerFilter::Nearest` + `MinifySamplerFilter::Nearest` +
//! `SamplerWrapFunction::Clamp` (`render/draw.rs:215-218`).  Nearest is not an
//! approximation of the GPU here — `atlas_linear_sampler` is used only for the
//! window background attachment (`glyph-frag.glsl:121`), which PureCpu does not
//! draw.  Bit-exact parity on scaled quads is therefore reachable, and the
//! identity test in this module is what keeps the 1:1 case exactly as it was.

/// First destination pixel covered by a quad edge, under GL's rule that a pixel
/// is covered when its centre lies inside the primitive.
#[inline]
pub fn cover_start(edge: f32) -> i32 {
    (edge - 0.5).ceil() as i32
}

/// First destination pixel *past* the quad, same rule.
///
/// Delegates rather than repeating the expression: the two names exist because
/// the call sites mean different things, but there is only one coverage rule
/// and a second copy of it would be free to drift.
#[inline]
pub fn cover_end(edge: f32) -> i32 {
    cover_start(edge)
}

/// Maps destination pixels to atlas texels along one axis.
#[derive(Clone, Copy, Debug)]
pub struct Axis {
    src_origin: f32,
    src_extent: f32,
    dest_origin: f32,
    dest_extent: f32,
    src_limit: i32,
}

impl Axis {
    pub fn new(
        src_origin: f32,
        src_extent: f32,
        dest_origin: f32,
        dest_extent: f32,
        src_limit: i32,
    ) -> Self {
        Self {
            src_origin,
            src_extent,
            dest_origin,
            dest_extent,
            src_limit,
        }
    }

    /// The atlas texel a destination pixel samples.
    ///
    /// `u` is the fraction along the quad of the pixel's *centre*, matching the
    /// shader's linear texcoord interpolation; the result is floored to a texel
    /// and clamped, matching Nearest + Clamp.
    #[inline]
    pub fn texel(&self, dest_pixel: i32) -> i32 {
        if self.dest_extent == 0.0 {
            return self.src_origin.floor().clamp(0.0, (self.src_limit - 1) as f32) as i32;
        }
        let u = (dest_pixel as f32 + 0.5 - self.dest_origin) / self.dest_extent;
        let s = self.src_origin + u * self.src_extent;
        let idx = s.floor();
        if !idx.is_finite() {
            return 0;
        }
        (idx as i32).clamp(0, (self.src_limit - 1).max(0))
    }
}
```

`cover_start` and `cover_end` are deliberately the same expression, kept as two names because the call sites mean different things and a future change to one edge rule must not silently change the other.

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-gui --bin wezterm-gui purecpu_sampler -- --nocapture
```

Expected: 8 passed. If `minifies_two_to_one` disagrees, do **not** adjust the test to match the code — work out which of the two is wrong against the rule stated at the top of this task. This is the half-texel convention, and getting it wrong lands everything one row off while still passing a 1-LSB gate.

- [ ] **Step 5: Commit**

```bash
git add wezterm-gui/src/termwindow/render/purecpu_sampler.rs wezterm-gui/src/termwindow/render/mod.rs
git commit -m "feat(purecpu): add the nearest-neighbour atlas sampler, with tests"
```

---

### Task 7: I5 + M1 — route every textured quad through the sampler

**Files:**
- Modify: `wezterm-gui/src/termwindow/render/purecpu.rs:257-267` (destination rect), `:290-296` (source rect), `:344-382` (the blit loop)

**Interfaces:**
- Consumes: `purecpu_sampler::{cover_start, cover_end, Axis}` from Task 6.

- [ ] **Step 1: Replace the destination rect computation (M1)**

At `purecpu.rs:257-267`, keep the float edges and derive integer bounds through the coverage rule:

```rust
                    // Screen destination rect (clip-space to pixels).
                    // Keep the float edges: the sampler interpolates texcoords
                    // across the true quad, and the coverage rule needs the
                    // unrounded edge.  Truncating here displaced sub-pixel
                    // quads one pixel left/up (findings M1, the fancy tab
                    // bar's bit-exact 1px shift).
                    let dest_fx = tl.position[0] + half_w;
                    let dest_fy = tl.position[1] + half_h;
                    let dest_fx2 = br.position[0] + half_w;
                    let dest_fy2 = br.position[1] + half_h;

                    let dest_x = crate::termwindow::render::purecpu_sampler::cover_start(dest_fx);
                    let dest_y = crate::termwindow::render::purecpu_sampler::cover_start(dest_fy);
                    let dest_x2 = crate::termwindow::render::purecpu_sampler::cover_end(dest_fx2);
                    let dest_y2 = crate::termwindow::render::purecpu_sampler::cover_end(dest_fy2);

                    let dest_w = dest_x2 - dest_x;
                    let dest_h = dest_y2 - dest_y;
                    if dest_w <= 0 || dest_h <= 0 {
                        continue;
                    }
```

The `IS_SOLID_COLOR` branch at `:300-342` needs no other change — it already iterates `clip_rects` and now gets correctly rounded bounds.

- [ ] **Step 2: Replace the source rect computation**

At `purecpu.rs:290-296`, keep atlas coordinates as floats rather than truncating:

```rust
                    // Atlas pixel rect from normalized tex coords, kept as
                    // floats: the sampler needs the true source extent, and
                    // truncating both edges loses up to a texel of it.
                    let tex_fx = tl.tex[0] * atlas_w as f32;
                    let tex_fy = tl.tex[1] * atlas_h as f32;
                    let tex_fx2 = br.tex[0] * atlas_w as f32;
                    let tex_fy2 = br.tex[1] * atlas_h as f32;
                    let tex_fw = tex_fx2 - tex_fx;
                    let tex_fh = tex_fy2 - tex_fy;
```

- [ ] **Step 3: Replace the blit loop**

Replace `purecpu.rs:344-382` — everything from the `let blit_w = tex_w.min(dest_w);` crop through the per-pixel atlas index computation. The body from `let tex_r = ...` onward is unchanged; only the iteration bounds and the index derivation change:

```rust
                    // Textured quads: sample the atlas across the full
                    // destination rect.  The old code blitted 1:1 and cropped
                    // to min(tex, dest), so any quad drawn at a size other
                    // than its sprite's was cropped rather than scaled
                    // (findings I5) — that is non-native inline images,
                    // DECDWL/DECDHL, and scaled bitmap glyphs.
                    use crate::termwindow::render::purecpu_sampler::Axis;
                    let ax = Axis::new(tex_fx, tex_fw, dest_fx, dest_fx2 - dest_fx, atlas_w as i32);
                    let ay = Axis::new(tex_fy, tex_fh, dest_fy, dest_fy2 - dest_fy, atlas_h as i32);

                    for clip in &clip_rects {
                        let [cx1, cy1, cx2, cy2] = *clip;

                        let row_start = cy1.max(dest_y).max(0);
                        let row_end = cy2.min(dest_y2).min(fb_h as i32);
                        let col_start = cx1.max(dest_x).max(0);
                        let col_end = cx2.min(dest_x2).min(fb_w as i32);

                        for dy in row_start..row_end {
                            let atlas_row = ay.texel(dy);
                            if atlas_row < 0 || atlas_row >= atlas_h as i32 {
                                continue;
                            }
                            let fb_row_off = dy as usize * fb_w;
                            let atlas_row_off = atlas_row as usize * atlas_stride;

                            for dx in col_start..col_end {
                                let atlas_col = ax.texel(dx);
                                if atlas_col < 0 || atlas_col >= atlas_w as i32 {
                                    continue;
                                }

                                let ai = atlas_row_off + atlas_col as usize * 4;
                                // ... unchanged from here: tex_r/tex_g/tex_b/tex_a,
                                // the has_color branches, colour-space handling,
                                // and blend_over.
```

The inner body's `let fi = (fb_row_off + dx as usize) * 4;` already uses `dx` and needs no change.

- [ ] **Step 4: Build and verify the 1:1 rows did not move**

This is the step that protects the four bit-identical parity rows. Run all six `parity` rows before looking at anything the fix was supposed to improve.

```bash
cd /scratch/oetiker/wezterm && cargo build -j4 --release -p wezterm-gui 2>&1 | tail -5
cp target/release/wezterm-gui /scratch/oetiker/wezterm-builds/wezterm-gui-fixed
cd tools/purecpu-parity
export PARITY_XAUTH=/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20
export WEZTERM_BIN=/scratch/oetiker/wezterm-builds/wezterm-gui-fixed
# Run each parity row's command, copied verbatim from parity-matrix.md:
#   text glyphs, cursor block/bar/underline, window buttons, rounded corners,
#   split dividers, inline images at native size, retro tab bar.
```

Expected: the four bit-identical rows still report `AE = 0, PAE = 0`; native-size images still `AE = 0, PAE = 0`; body text still `PAE = 257`. **Any** drift is a regression introduced here, not a rounding difference — the sampler's identity test says the 1:1 path is unchanged, so drift means a quad that is not actually 1:1 was being cropped into looking right.

- [ ] **Step 5: Verify the rows this fixes**

Run, verbatim from the rendered documents:

- iTerm2 OSC 1337 at 200×132 px and at 20×4 cells (was `AE = 39360`, `PAE = 61423`)
- Sixel at 300×300 (was `AE = 3300`, `PAE = 1542`)
- DECDWL/DECDHL (was body `PAE = 45232` against a `PAE = 257` control **in the same capture** — that control is the reason a null result here cannot pass as success)
- The DECDHL bottom band over y=99-110 (was OpenGL 1824 ink px, PureCpu 0)
- The scaled-glyph corpus from Task 1, **only if** Task 1 found a font that triggers `glyph.scale != 1`

Expected: each crosses its gate. For DECDWL the control line must still read `PAE = 257` in the same frame — if the control moved too, the instrument changed, not the renderer.

- [ ] **Step 6: Commit**

```bash
git add wezterm-gui/src/termwindow/render/purecpu.rs
git commit -m "fix(purecpu): scale textured quads instead of cropping them (I5, M1)"
```

---

### Task 8: Subpixel antialiasing — per-channel coverage and blend

The LCD path already stores per-channel sRGB coverage in R/G/B with max-of-channels alpha in A (`wezterm-font/src/rasterizer/skrifa_rasterizer.rs:695-701`). PureCpu's `IS_GLYPH` branch reads only `tex_a` and discards R/G/B (`purecpu.rs:412-419`), then composites through the scalar-alpha `blend_over`.

GL instead emits `colorMask` as a second fragment output and selects `dual_source_blending`: `dst = src0 * src1 + dst * (1 - src1)` per channel, where `src0` is the fg colour and `src1` is the per-channel coverage (`draw.rs:181-195`, `glyph-frag.glsl:13-16,145-153`).

**Files:**
- Modify: `wezterm-gui/src/termwindow/render/purecpu.rs` (`blend_over` neighbourhood, the `IS_GLYPH` branch, and the layer loop where `idx` is known)

**Interfaces:**
- Produces: `fn blend_over_masked(fb: &mut [u8], fi: usize, sr: u8, sg: u8, sb: u8, sa: u8, mr: u8, mg: u8, mb: u8, ma: u8)`

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn blend_over_masked_blends_each_channel_by_its_own_coverage() {
        // GL's dual-source blend: dst = src * mask + dst * (1 - mask), per
        // channel.  A mask that differs per channel is the entire point of
        // subpixel AA; PureCpu previously collapsed it to the max-alpha and
        // rendered grayscale.
        // fb is BGRA.
        let mut fb = vec![0u8, 0, 0, 255];
        blend_over_masked(&mut fb, 0, 200, 100, 50, 255, 255, 128, 0, 255);
        assert_eq!(fb[2], 200, "red fully covered");
        assert_eq!(fb[1], 50, "green half covered: 100*128/255 rounds to 50");
        assert_eq!(fb[0], 0, "blue not covered at all");
    }

    #[test]
    fn blend_over_masked_with_uniform_mask_matches_blend_over() {
        // Sanity: a mask equal on all channels must agree with the scalar
        // path, so enabling subpixel cannot shift ordinary text.
        for a in [0u8, 1, 64, 128, 254, 255] {
            let mut fb1 = vec![10u8, 20, 30, 40];
            let mut fb2 = vec![10u8, 20, 30, 40];
            blend_over(&mut fb1, 0, 200, 100, 50, a);
            blend_over_masked(&mut fb2, 0, 200, 100, 50, a, a, a, a, a);
            assert_eq!(fb1, fb2, "mismatch at alpha {a}");
        }
    }
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-gui --bin wezterm-gui blend_over_masked -- --nocapture
```

Expected: FAIL — `blend_over_masked` is undefined.

- [ ] **Step 3: Implement the per-channel blend**

Next to `blend_over` in `purecpu.rs`:

```rust
/// Per-channel alpha blend, reproducing GL's dual-source blending:
///   dst = src * mask + dst * (1 - mask)
/// applied independently per channel.  `mask` is the per-channel coverage the
/// LCD rasteriser stores in the atlas RGB (skrifa_rasterizer.rs:695-701); the
/// scalar `blend_over` uses only the max-of-channels alpha in A, which is what
/// made PureCpu fall back to grayscale antialiasing.
/// fb is BGRA.
#[inline]
#[allow(clippy::too_many_arguments)]
fn blend_over_masked(
    fb: &mut [u8],
    fi: usize,
    sr: u8,
    sg: u8,
    sb: u8,
    sa: u8,
    mr: u8,
    mg: u8,
    mb: u8,
    ma: u8,
) {
    #[inline]
    fn chan(s: u8, d: u8, m: u8) -> u8 {
        if m == 255 {
            return s;
        }
        if m == 0 {
            return d;
        }
        let mf = m as f32 / 255.0;
        (s as f32 * mf + d as f32 * (1.0 - mf) + 0.5) as u8
    }
    fb[fi] = chan(sb, fb[fi], mb);
    fb[fi + 1] = chan(sg, fb[fi + 1], mg);
    fb[fi + 2] = chan(sr, fb[fi + 2], mr);
    fb[fi + 3] = chan(sa, fb[fi + 3], ma);
}
```

- [ ] **Step 4: Run to verify the tests pass**

```bash
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-gui --bin wezterm-gui blend_over -- --nocapture
```

Expected: both new tests pass, plus the three pre-existing `blend_over_*` tests.

If `blend_over_masked_with_uniform_mask_matches_blend_over` fails at `a = 255`, the cause is `blend_over`'s opaque fast path forcing alpha to 255 while `chan` computes it — reconcile in favour of `blend_over`'s existing behaviour, since that path is what the measured parity rows were produced with.

- [ ] **Step 5: Wire it into the `IS_GLYPH` branch**

Compute `use_subpixel` once, before the layer loop, with the same expression GL uses (`draw.rs:172-179`):

```rust
        let use_subpixel = match self
            .config
            .freetype_render_target
            .unwrap_or(self.config.freetype_load_target)
        {
            config::FreeTypeLoadTarget::HorizontalLcd
            | config::FreeTypeLoadTarget::VerticalLcd => true,
            _ => false,
        };
```

Inside the layer loop, `subpixel_aa` is `use_subpixel && idx == 1` — the same condition as `draw.rs:244`, since only vertex-buffer index 1 carries text.

In the `IS_GLYPH` (`has_color == 0.0`) branch, keep `out_a = tex_a` for the non-subpixel case, and when `subpixel_aa` is set, carry `(tex_r, tex_g, tex_b, tex_a)` through as the mask and blend with `blend_over_masked` using the fg colour as the source. Note the shader does **not** overwrite `color.a` with the mask when `subpixel_aa` (`glyph-frag.glsl:150-152`), so the source alpha stays `fg_a`.

The per-channel mask is already sRGB in the atlas, and PureCpu blends in 8-bit sRGB, so no colour-space conversion applies to the mask — only the fg colour goes through `linear_to_srgb` as it does today.

- [ ] **Step 6: Verify against the harness**

```bash
cd /scratch/oetiker/wezterm && cargo build -j4 --release -p wezterm-gui && \
  cp target/release/wezterm-gui /scratch/oetiker/wezterm-builds/wezterm-gui-fixed
cd tools/purecpu-parity
# Generate the subpixel case through gen-config.sh — do NOT hand-write a config
# (Global Constraints: the bell).  If gen-config.sh has no knob for
# freetype_render_target, add one there.
```

Expected: with `freetype_render_target = "HorizontalLcd"`, PureCpu now matches GL's subpixel output. **Could this have failed and looked like success?** Yes, in the review's most-repeated way: if the config was rejected, both windows fall back to the same backend and report a triumphant `AE = 0`. `check_no_config_error` guards this — confirm it ran and the log was present, since the guard passes silently if the log is missing.

Also re-run the plain text row: with `use_subpixel` false it must still read `PAE = 257`, proving the new branch is inert when not selected.

- [ ] **Step 7: Commit**

```bash
git add wezterm-gui/src/termwindow/render/purecpu.rs tools/purecpu-parity/gen-config.sh
git commit -m "feat(purecpu): per-channel subpixel antialiasing to match the GL dual-source blend"
```

---

### Task 9: I4 — test the resolved cursor shape

`mod.rs:1383` tests the **raw** pane cursor shape, but the shape the renderer draws is resolved against config first (`render/mod.rs:604-611`). So `default_cursor_style = "BlinkingBlock"` — the documented way to turn blinking on — never satisfies `cursor.shape.is_blinking()` and blink never starts. Sampled 16 times over 6.4 s, PureCpu returned `gray(224)` every time while GL swept 17–222.

**Files:**
- Modify: `wezterm-gui/src/termwindow/mod.rs:1383`

- [ ] **Step 1: Confirm how the render path resolves the shape**

```bash
cd /scratch/oetiker/wezterm && sed -n '604,610p' wezterm-gui/src/termwindow/render/mod.rs
```

Expected — the renderer resolves the raw shape through the config:

```rust
        let (cursor_shape, visibility) = match params.cursor {
            Some(cursor) => (
                params
                    .config
                    .default_cursor_style
                    .effective_shape(cursor.shape),
                cursor.visibility,
            ),
```

The fix uses `effective_shape` too — not a reimplementation of it, which would be a second copy free to drift from the renderer it is supposed to predict.

- [ ] **Step 2: Apply the resolution at the blink test**

Replace the `cursor_blinking` computation at `mod.rs:1383-1385`:

```rust
                // I4: resolve the shape the way the render path does
                // (render/mod.rs:604-611) before asking whether it blinks.
                // Testing the raw pane shape made default_cursor_style =
                // "BlinkingBlock" — the documented way to enable blinking —
                // inert, because the raw shape is Default until an
                // application sets it with DECSCUSR.
                let effective_shape = self
                    .config
                    .default_cursor_style
                    .effective_shape(cursor.shape);
                let cursor_blinking = effective_shape.is_blinking()
                    && self.config.cursor_blink_rate != 0
                    && self.focused.is_some();
```

- [ ] **Step 3: Verify with the config-driven blink case**

```bash
cd /scratch/oetiker/wezterm && cargo build -j4 --release -p wezterm-gui && \
  cp target/release/wezterm-gui /scratch/oetiker/wezterm-builds/wezterm-gui-fixed
cd tools/purecpu-parity
export PARITY_XAUTH=/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20
export WEZTERM_BIN=/scratch/oetiker/wezterm-builds/wezterm-gui-fixed
# The cursor-blink row's sample-case.sh command, verbatim from parity-matrix.md,
# with DEFAULT_CURSOR_STYLE=BlinkingBlock.
```

Expected: the sampled sequence varies over time instead of returning `gray(224)` sixteen times.

**Could this have failed and looked like success?** The inverse of the original false verdict: a *varying* sequence proves motion, so this direction is safer than the `missing` verdict was. But a sequence varying for the wrong reason (window redrawn by something else) is still possible — keep the window idle and confirm nothing else is producing output.

- [ ] **Step 4: Commit**

```bash
git add wezterm-gui/src/termwindow/mod.rs
git commit -m "fix(purecpu): resolve the cursor shape before testing whether it blinks (I4)"
```

---

### Task 10: I3 + L3 — per-feature dirty rects for time-driven content

The idle skip at `mod.rs:1436-1447` returns before the paint pass when no line is dirty. Nothing time-driven marks itself dirty, so blinking text, the visual bell and GIF frames never repaint. The `Alert::Bell` handler invalidates the window but pushes no rect, so the early exit still fires.

**The idle skip stays.** Each animated thing marks its own region dirty instead, which preserves the idle-efficiency property that is PureCpu's reason to exist. The rejected alternative — skip the early exit whenever any animation is live — would repaint the whole window at the blink rate forever, since a blinking cursor is present in almost every session.

**Files:**
- Modify: `wezterm-gui/src/termwindow/mod.rs` (the `Alert::Bell` handler, the blink scheduling path, the GIF/animation frame path, and `schedule_blink_timer_if_needed`)

- [ ] **Step 1: Fix L3 first — the integer-division blink interval**

`schedule_blink_timer_if_needed` divides by `animation_fps` in integer arithmetic, so the interval is quantised before it is used. Locate it and convert to floating point, or to a `Duration::from_secs_f64`, before building on top of it:

```bash
cd /scratch/oetiker/wezterm && grep -n "fn schedule_blink_timer_if_needed" -A 25 wezterm-gui/src/termwindow/mod.rs
```

- [ ] **Step 2: Make the visual bell push a dirty rect**

Find the `Alert::Bell` handler:

```bash
cd /scratch/oetiker/wezterm && grep -n "Alert::Bell" wezterm-gui/src/termwindow/mod.rs
```

The bell tints the whole window's background, so the rect is the whole window. Push it onto `purecpu_state.dirty_pixel_rects` when the PureCpu backend is active, alongside the existing invalidate. The bell's fade runs for `fade_in_duration_ms + fade_out_duration_ms`, so a single rect at ring time is not enough — the animation timer must keep pushing one until the fade completes, the same way the cursor-blink path reschedules itself.

- [ ] **Step 3: Make blinking text push dirty rects**

SGR 5 text blink has **no rescheduling mechanism at all** — the review calls it a structural absence, not a resolution gap, and it is why the word renders as blank space (the eased intensity starts at `fg = bg` on the one paint that happens when the window settles). Two pieces are needed: cells carrying the blink attribute must be found, and the animation timer must dirty them.

Locate where the renderer learns a cell blinks (`blink_state` / `rapid_blink_state` and their uses in `render/`), and mark those cells' `cell_rect`s dirty from the animation timer. Use `purecpu_dirty::cell_rect` with the owning pane's placement — not window-origin arithmetic, which is I2 reintroduced.

- [ ] **Step 4: Make GIF frame advance push a dirty rect**

Find where an animated image decides to advance a frame and mark the image quad's region dirty. Over 12 samples across 6 s, GL cycled both frames while PureCpu reported the same frame all 12 times.

- [ ] **Step 5: Build and verify each animation independently**

```bash
cd /scratch/oetiker/wezterm && cargo build -j4 --release -p wezterm-gui && \
  cp target/release/wezterm-gui /scratch/oetiker/wezterm-builds/wezterm-gui-fixed
```

Run the three animation rows' `sample-case.sh` commands verbatim, plus the GIF row. Each must show a varying sample sequence.

**The control experiment settles all four at once** and is the strongest instrument this project found: run the same configs with `purecpu_force_full_repaint = true`. That is the known-good ceiling — the bell and blink text return at the GPU path's own levels under it. Our fix must reach the same levels *without* it.

- [ ] **Step 6: Measure idle CPU — the regression this fix can hide**

This fix's failure mode is repainting **too much**, which looks perfect in every pixel comparison and costs CPU forever. Measure against two baselines:

```bash
# Floor: the pre-fix binary, blinking cursor on screen, window idle.
# Subject: the fixed binary, same config.
# Ceiling: the fixed binary with purecpu_force_full_repaint = true.
# Sample /proc/<pid>/stat utime+stime over 30 s for each; the gui process is
# the one with the largest RSS (see findings.md C1's note on pgrep — `| head -1`
# picks the parent and has produced a wrong number in this project before).
```

Expected: the subject sits far closer to the floor than to the ceiling. If it approaches the ceiling, the dirty rects are too large or too frequent — that is a defect in this task, not an acceptable cost.

- [ ] **Step 7: Commit**

```bash
git add wezterm-gui/src/termwindow/mod.rs
git commit -m "fix(purecpu): mark time-driven content dirty so it repaints when idle (I3, L3)"
```

---

### Task 11: M2 — re-measure, then fix or close

**Do not fix M2 before re-measuring it.** Its attribution to a specific rect overlap was tested and eliminated twice; the surviving evidence is a coverage model that predicts all 87 differing pixels within 1.91 LSB, mean residual −0.05. The dirty-rect geometry is the suspected source and Task 4 rewrote it, so M2 may no longer exist — and if it does, it will be a different overlap than the one that was hunted.

**Files:**
- Modify (only if the re-measurement shows the defect survives): `wezterm-gui/src/termwindow/render/purecpu.rs`

- [ ] **Step 1: Re-run the wide-sixel label-row case**

Copy the command from `findings.md` M2 verbatim, with `WEZTERM_BIN` pointing at the fixed binary.

- [ ] **Step 2: Decide from the evidence**

- If the 87 differing pixels are gone: M2 is closed by Tasks 4/7. Record which, with the numbers, and skip to Step 4.
- If they survive: the double-composite is real and independent of the dirty-rect geometry. Fix it by making `collect_clip_rects` produce disjoint regions, so no destination pixel is visited twice within a quad.

Note the review's own warning: M2's original analysis binned 87 pixels in 8-px bins on 10-px cells and concluded "two cells", which was the wrong geometry. If you bin anything here, make the bins align with the cell size.

- [ ] **Step 3: If fixing — write the test first**

```rust
    #[test]
    fn collect_clip_rects_produces_disjoint_regions() {
        // M2: overlapping dirty rects made the blit visit the same
        // destination pixel more than once, compositing the glyph twice.
        let rects = vec![
            DirtyRect { x: 0, y: 0, width: 20, height: 10 },
            DirtyRect { x: 10, y: 0, width: 20, height: 10 },
        ];
        let mut out = Vec::new();
        collect_clip_rects(0, 0, 30, 10, &rects, &mut out);
        // Every destination pixel must be covered at most once.
        let mut seen = std::collections::HashSet::new();
        for [x1, y1, x2, y2] in out {
            for y in y1..y2 {
                for x in x1..x2 {
                    assert!(seen.insert((x, y)), "pixel ({x},{y}) covered twice");
                }
            }
        }
    }
```

- [ ] **Step 4: Commit**

```bash
git add -A wezterm-gui/src/termwindow/render/purecpu.rs
git commit -m "fix(purecpu): composite each destination pixel once (M2)"
# or, if closed by re-measurement:
git commit --allow-empty -m "docs(purecpu): M2 closed by re-measurement after I1/I2/I5 — see report"
```

---

### Task 12: M3, M4, M5 — font-data panics

Three process aborts reachable from font data. All three are recorded Medium rather than Critical **only** because no crafted font was built to fire them (`fontTools` was unavailable). Fixing them blind is acceptable; **claiming they are verified is not.** The report must say "the panic site is guarded", not "the crash was reproduced and no longer occurs".

**Files:**
- Modify: `wezterm-font/src/rasterizer/skrifa_rasterizer.rs`

- [ ] **Step 1: M3 — the COLR empty-path fallback**

Find the unconditional panic in the empty-path fallback of the COLR glyph rasteriser. The fallback is unfinishable and panics whenever reached. Replace the panic with a graceful return of an empty glyph — a missing coloured glyph is a visual defect; a panic takes down the terminal.

```bash
cd /scratch/oetiker/wezterm && grep -n "panic!\|unreachable!\|todo!\|unimplemented!" wezterm-font/src/rasterizer/skrifa_rasterizer.rs
```

- [ ] **Step 2: M4 — zero-palette CPAL**

`cpal.color_record_indices()[0]` panics on a CPAL table with zero palettes. Use `.first()` and fall back to the default foreground colour.

- [ ] **Step 3: M5 — `unitsPerEm = 0`**

Divides by zero in both the shaper and the rasteriser. Guard both sites; a font declaring `unitsPerEm = 0` is malformed, so refusing to load it is a legitimate outcome — but it must be an error, not a division.

```bash
cd /scratch/oetiker/wezterm && grep -rn "units_per_em" wezterm-font/src/ | head -20
```

- [ ] **Step 4: Write tests for each guard**

Each guard gets a unit test that constructs the degenerate input directly — a zero-palette CPAL record, a zero `unitsPerEm` — and asserts an `Err` or a fallback rather than a panic. These test the **guard**, not the font that reaches it; say so in each test's comment so nobody later reads them as reproduction.

- [ ] **Step 5: Run the font test suite**

```bash
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-font 2>&1 | tail -20
```

Expected: the pre-existing test passes plus the new guards.

- [ ] **Step 6: Commit**

```bash
git add wezterm-font/src/rasterizer/skrifa_rasterizer.rs
git commit -m "fix(font): guard three panic sites reachable from malformed font data (M3, M4, M5)"
```

---

### Task 13: L1, L2, L4, L5 — performance and hygiene

**Files:**
- Modify: `wezterm-gui/src/termwindow/render/purecpu.rs` (L1), `wezterm-font/src/rasterizer/skrifa_rasterizer.rs` (L2), the window crate's selection-owner call (L4), `config/src/` (L5)

- [ ] **Step 1: L1 — band copy and X GC churn**

Every presented band copies the whole band and creates/destroys an X GC. Reuse a single GC across presents. Measure before and after with the same idle-CPU method as Task 10 Step 6; if there is no measurable improvement, keep the simpler code and record that.

- [ ] **Step 2: L2 — per-glyph `HintingInstance`**

`HintingInstance` is constructed per glyph rasterisation. Cache it per (font, size) instead. This is on the glyph-miss path, so its effect shows up on first paint and on font changes, not steady state — measure accordingly rather than claiming a general speedup.

- [ ] **Step 3: L4 — `SetSelectionOwner` uses `CURRENT_TIME`**

Pass the triggering event's timestamp instead. `CURRENT_TIME` is a race against other selection owners.

- [ ] **Step 4: L5 — removed backends silently substituted**

A config naming a removed backend is silently substituted rather than reported. Log a warning naming both what was asked for and what was used. Note this interacts with `check_no_config_error`: a warning must not be phrased so it trips the harness's `Configuration Error` grep, or every case starts failing.

- [ ] **Step 5: Run the full suite and commit**

```bash
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-gui --bin wezterm-gui 2>&1 | tail -10
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-font 2>&1 | tail -10
git add -A
git commit -m "perf/chore(purecpu): GC reuse, hinting cache, selection timestamp, backend warning (L1, L2, L4, L5)"
```

---

### Task 14: Final verification and documentation

The single review checkpoint. Everything below runs against the finished binary.

**Files:**
- Modify: `docs/purecpu-review/parity-matrix.md`, `docs/purecpu-review/findings.md`, `docs/purecpu-review/README.md`

- [ ] **Step 1: Build the final binary**

```bash
cd /scratch/oetiker/wezterm && cargo build -j4 --release -p wezterm-gui 2>&1 | tail -5
cp target/release/wezterm-gui /scratch/oetiker/wezterm-builds/wezterm-gui-fixed
ls -la /scratch/oetiker/wezterm-builds/
```

Expected: `wezterm-gui-prefix` (pre-fix reference), `wezterm-gui-rebased` (untouched) and `wezterm-gui-fixed` all present.

- [ ] **Step 2: The GL invariant — OpenGL must be bit-identical**

Two fixes touched code shared with the GL path (C1 in `renderstate.rs`, the pane iteration in `mod.rs`). Everything else is behind `purecpu_state` or in `purecpu.rs`.

```bash
cd /scratch/oetiker/wezterm/tools/purecpu-parity
export PARITY_XAUTH=/tmp/claude-1003/-scratch-oetiker-wezterm/659fab62-7b17-4059-a502-42815bcb9732/scratchpad/Xauthority-20
# Capture the GL backend from the pre-fix binary and from the fixed binary,
# sequentially (never concurrently — the focus confound), same corpus, and diff.
```

Expected: `AE = 0`. A non-zero result means a PureCpu fix leaked into the GL path and must be traced before anything else is reported.

- [ ] **Step 3: The no-drift set — six `parity` rows**

Re-run all six. Four must still be bit-identical (`AE = 0, PAE = 0`): static cursors in all three shapes, window buttons, rounded corners, split dividers. Native-size inline images must still be `AE = 0, PAE = 0`. Body text must still be `PAE = 257`; the retro tab bar within 1 LSB.

- [ ] **Step 4: Re-run every measured row and record the numbers**

All 14 measured rows, commands verbatim, `WEZTERM_BIN` pointing at the fixed binary. Record old number → new number → verdict for each.

- [ ] **Step 5: Update the deliverables honestly**

For each row whose verdict changed, update `parity-matrix.md` and state **which binary** it was measured against — the documents currently describe `wezterm-gui-rebased`, and a matrix mixing pre- and post-fix numbers without saying so is worse than one that was never updated.

Constraints inherited from the review, which still bind: the Verdict column holds a **bare token** (`parity` / `degraded` / `missing` / `known gap`) with caveats in Evidence; the table is **6 cells per row**, and a literal `|` inside a cell silently breaks it — this has happened twice.

In `findings.md`, mark each finding fixed with the commit that fixed it. For M3/M4/M5 write "panic site guarded; trigger never reproduced" — not "fixed". For the scaled-glyph row, if Task 1 found no triggering font, the row stays `by reading` and says why.

- [ ] **Step 6: Rewrite `README.md`'s root-cause table**

The four root causes are the document's spine and they are now repaired. Rewrite that section to describe what PureCpu does *now*, with the residue stated plainly: whatever did not reach parity, and why. Do not leave a document that reads as current while describing a binary that no longer exists.

- [ ] **Step 7: Full test suite**

```bash
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-gui --bin wezterm-gui 2>&1 | tail -10
cd /scratch/oetiker/wezterm && cargo test -j4 -p wezterm-font 2>&1 | tail -10
```

Expected: everything green, and materially more tests than the 29 + 1 the review found — the review's point was that a green suite said nothing because `purecpu.rs`'s tests did not touch the rasterising path. Tasks 3, 6 and 8 changed that; confirm the count reflects it.

- [ ] **Step 8: Commit**

```bash
git add docs/purecpu-review/
git commit -m "docs(purecpu): update the review deliverables to the post-fix binary"
```

---

## Verification Summary

| Layer | What it catches | Where |
|---|---|---|
| Unit tests | Geometry and sampling arithmetic, in isolation | Tasks 3, 6, 8, plus 2, 5, 12 |
| Harness rows | Whether the defect the row measured is gone | Tasks 4, 7, 9, 10, 11 |
| GL invariant | A PureCpu fix leaking into the GPU path | Task 14 Step 2 |
| No-drift set | The four bit-identical rows regressing under the sampler rewrite | Task 7 Step 4, Task 14 Step 3 |
| Idle CPU | Task 10 repainting too much — invisible to every pixel comparison | Task 10 Step 6 |
| Control experiment | Whether a fix reached the GPU path's own level, or merely moved | Task 10 Step 5 |

## Open Questions Carried From the Spec

- Whether a font yielding `glyph.scale != 1` exists on this machine (Task 1 Step 8). If not, that matrix row stays unverified and says so.
- Whether M2 survives Task 4 at all (Task 11 Step 2).
- Whether the shader-equivalence rewrite (spec §3 option C) becomes worthwhile now that the sampler and dirty units have tests.
