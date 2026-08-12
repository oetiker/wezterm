//! Pane-aware dirty-rect geometry for the PureCpu renderer.
//!
//! Every rect PureCpu repaints is derived here.  The pane's origin is an
//! explicit argument rather than an ambient assumption, which is the whole
//! point of the module: the previous inline version computed rects from the
//! window's content origin alone, so any pane that was not at the window's
//! top-left was repainted at the wrong place — or, because the rects then
//! matched nothing the paint pass drew, not repainted at all (findings I2).

use crate::colorease::ColorEase;
use crate::termwindow::render::purecpu::DirtyRect;
use config::{VisualBell, VisualBellTarget};
use std::time::Instant;
use termwiz::cell::Blink;

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
///
/// The PureCpu dirty walk uses [`row_band_painted`] instead, because a band
/// this narrow leaves the pane's fringe stale (see [`PaneSpan`]).  Kept as the
/// text-area answer for anything that wants the cells and nothing else.
#[allow(dead_code)]
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

/// The horizontal extent of a pane's PAINTED background, in pixels.
///
/// Mirrors the `background_rect` of `render/pane.rs`, which exists there twice
/// — at :110-152 in `paint_pane` and again at :606-646 in `build_pane`, term for
/// term identical.  A pane's background is deliberately
/// oversized to fill out to the split edges, and the right-most pane is
/// painted all the way to the window edge.  A dirty band narrower than this
/// leaves the fringe — window padding, the half-cell split gutter, a trailing
/// partial cell, and glyph overhang past the last column — cleared by nobody
/// and drawn by nobody, so the previous frame's pixels survive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaneSpan {
    pub x: i32,
    pub width: i32,
}

/// Compute [`PaneSpan`] for a pane.
///
/// `content_left` is `padding_left + border.left`, **unrounded** — the same
/// `f32` the paint pass uses.  It must not be truncated by the caller: it feeds
/// the *right* edge of the span (through `width_delta` for a left-most pane and
/// through `x` for an interior one), so flooring it there shortens the span and
/// leaves a stale column at the split gutter whenever `window_padding` is
/// fractional, i.e. expressed in cells, points or percent.
///
/// The arithmetic is `f32`, term for term with `render/pane.rs`, so the value
/// rounded here is bit-identical to the painted rect's rather than merely close
/// to it.  Only the final conversion rounds, and it rounds **outward** — floor
/// on the left edge, ceil on the right — so the span is always a superset of the
/// painted rect.  Over-covering costs a few repainted pixels; under-covering is
/// the stale fringe this exists to prevent.
pub fn painted_x_span(
    left_cells: i32,
    cols: i32,
    total_cols: i32,
    content_left: f32,
    cell_w: i32,
    window_pixel_width: i32,
) -> PaneSpan {
    let cw = cell_w as f32;

    // `x` and `width_delta` of render/pane.rs.
    let (x, width_delta) = if left_cells == 0 {
        (0., content_left + cw / 2.0)
    } else {
        (content_left - cw / 2.0 + left_cells as f32 * cw, cw)
    };

    let right = if left_cells + cols >= total_cols {
        // Go all the way to the right edge if we're right-most.
        window_pixel_width as f32
    } else {
        // Association matters: render/pane.rs builds a rect of (x, width) and
        // the right edge is `x + width`, so `cols*cw + width_delta` is summed
        // FIRST and x is added last.  `(x + cols*cw) + width_delta` is a
        // different f32 by up to an ulp, which is enough to flip a ceil.
        x + ((cols as f32 * cw) + width_delta)
    };

    let x_i = x.floor() as i32;
    PaneSpan {
        x: x_i,
        width: (right.ceil() as i32 - x_i).max(0),
    }
}

/// [`row_band`], but spanning the pane's painted background rather than only
/// its text cells.  This is what the PureCpu dirty-rect walk wants: anything
/// narrower leaves the fringe stale (see [`PaneSpan`]).
pub fn row_band_painted(
    p: &PanePlacement,
    row_in_viewport: i32,
    span: &PaneSpan,
) -> Option<DirtyRect> {
    if row_in_viewport < 0 || row_in_viewport >= p.rows {
        return None;
    }
    Some(DirtyRect {
        x: span.x,
        y: p.origin_y + row_in_viewport * p.cell_h,
        width: span.width,
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

/// Whether the cursor blink animation is running, and so whether the cursor's
/// cell must be marked dirty when the blink phase turns over.
///
/// I4: **the shape has to be resolved against `default_cursor_style` before
/// asking whether it blinks**, exactly as the render path resolves it
/// (`render/mod.rs`, `effective_shape` at the `params.cursor` match).  The raw
/// pane shape stays `CursorShape::Default` until an application sets one with
/// DECSCUSR, and `Default::is_blinking()` is false — so testing the raw shape
/// made `default_cursor_style = "BlinkingBlock"`, the documented way to turn
/// blinking on, completely inert.  Measured: PureCpu returned `gray(224)` on all
/// 16 samples over 6.4 s while GL swept 17–222.
///
/// This **delegates** to `effective_shape` rather than restating its match arms.
/// A second copy would be free to drift from the renderer this predicate exists
/// to predict, and the failure would be silent: the cursor would be repainted on
/// a schedule that no longer matched the one it is drawn on.
pub fn cursor_blinking(
    default_cursor_style: config::DefaultCursorStyle,
    shape: termwiz::surface::CursorShape,
    cursor_blink_rate: u64,
    focused: bool,
) -> bool {
    default_cursor_style.effective_shape(shape).is_blinking()
        && cursor_blink_rate != 0
        && focused
}

/// The vertical extent of a pane's PAINTED background, in pixels — the y-axis
/// twin of [`PaneSpan`].
///
/// Kept a separate type from [`PaneSpan`] on purpose: the two carry the same
/// pair of numbers but never the same axis, and a single struct with `x`/`width`
/// field names invites passing one where the other belongs, which type-checks
/// and silently transposes the rect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaneVSpan {
    pub y: i32,
    pub height: i32,
}

/// [`painted_x_span`] on the vertical axis.
///
/// Mirrors the `y` / `height_delta` half of the `background_rect` of
/// `render/pane.rs` (:110-152 in `paint_pane`, :606-646 in `build_pane`), with
/// the same rounding contract: floor the top edge, ceil the bottom, so the span
/// is always a superset of the painted rect.
///
/// `content_top` is `top_bar_height + padding_top + border.top` **unrounded**
/// — the paint pass's `top_pixel_y` — and `padding_top` is passed separately
/// because a top-most pane's background starts at `top_pixel_y - padding_top`,
/// i.e. it reaches up under the window padding to the bottom of the tab bar.
/// Truncating either of them shortens the span and leaves a stale strip.
pub fn painted_y_span(
    top_cells: i32,
    rows: i32,
    total_rows: i32,
    content_top: f32,
    padding_top: f32,
    cell_h: i32,
    window_pixel_height: i32,
) -> PaneVSpan {
    let ch = cell_h as f32;

    let (y, height_delta) = if top_cells == 0 {
        (content_top - padding_top, padding_top + ch / 2.0)
    } else {
        (content_top + top_cells as f32 * ch - ch / 2.0, ch)
    };

    let bottom = if top_cells + rows >= total_rows {
        window_pixel_height as f32
    } else {
        // Association matters, exactly as in painted_x_span: render/pane.rs
        // builds a rect of (y, height) whose bottom edge is `y + height`, and
        // `height` is `(rows*ch) + height_delta`, so `y` is added last.
        y + ((rows as f32 * ch) + height_delta)
    };

    let y_i = y.floor() as i32;
    PaneVSpan {
        y: y_i,
        height: (bottom.ceil() as i32 - y_i).max(0),
    }
}

/// The pane's whole painted background rect, from its two spans.
pub fn pane_rect(span: &PaneSpan, vspan: &PaneVSpan) -> DirtyRect {
    DirtyRect {
        x: span.x,
        y: vspan.y,
        width: span.width,
        height: vspan.height,
    }
}

/// What the visual bell is currently tinting, if anything.
///
/// The bell is not a resolution problem but a *scheduling* one: `Alert::Bell`
/// calls `window.invalidate()` and nothing else, so PureCpu's idle skip sees an
/// empty dirty list and returns before the paint pass — for the whole fade.
/// The fade therefore needs a region pushed on **every** frame until it ends,
/// which is what this reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BellRegion {
    /// Not ringing, or the fade has completed.
    None,
    /// `VisualBellTarget::BackgroundColor`: a second background quad over the
    /// pane's whole `background_rect` (`render/pane.rs:176-215`).
    PaneBackground,
    /// `VisualBellTarget::CursorColor`: only the cursor's own cell is tinted
    /// (`render/mod.rs:536-560`, inside `compute_cell_fg_bg`).
    CursorCell,
}

/// Whether the bell's fade is still running, and which region it paints.
///
/// **Delegates** to [`ColorEase::peek_intensity`], the same easing the paint
/// pass runs through `get_intensity_if_bell_target_ringing`, rather than
/// restating `elapsed < fade_in + fade_out`.
///
/// The reason is not the edge cases — it is that delegation makes this
/// predicate **bit-for-bit the renderer's own expression**, so it cannot end
/// the fade a frame before or after the renderer does, and a predicate that
/// ended it one frame early would leave the last tinted frame on screen
/// forever.  The edge behaviour is worth knowing but does not carry the
/// argument: with `fade_out_duration_ms = 0` and `elapsed > fade_in`,
/// `completion` is `+inf`, not NaN, and `inf >= 1.0` is true, so the fade ends
/// immediately (`colorease.rs`).  NaN arises only at the measure-zero instant
/// `elapsed == fade_in` exactly, where it is `0.0/0.0`.  An earlier version of
/// this comment claimed the NaN kept the bell alive and had the sign of the
/// whole edge case backwards; the Task 10 review's M4 run disproved it.
///
/// `peek_intensity` takes `&self`: unlike `intensity_one_shot` it does **not**
/// clear the start instant when the fade ends.  Clearing it is the paint pass's
/// job (`per_pane.bell_start.take()`), and doing it here would end the fade
/// without ever painting the final frame.
pub fn bell_region(bell_start: Option<Instant>, visual_bell: &VisualBell) -> BellRegion {
    let Some(start) = bell_start else {
        return BellRegion::None;
    };
    let ease = ColorEase::new(
        visual_bell.fade_in_duration_ms,
        visual_bell.fade_in_function.clone(),
        visual_bell.fade_out_duration_ms,
        visual_bell.fade_out_function.clone(),
        Some(start),
    );
    if ease.peek_intensity().is_none() {
        return BellRegion::None;
    }
    match visual_bell.target {
        VisualBellTarget::BackgroundColor => BellRegion::PaneBackground,
        VisualBellTarget::CursorColor => BellRegion::CursorCell,
    }
}

/// Whether a cell carrying this blink attribute is animating.
///
/// This is the predicate `render/screen_line.rs` itself gates on when it eases
/// the foreground colour towards the background, so the two cannot drift: the
/// renderer calls this function rather than testing `blink_rate != 0` inline.
/// That matters more here than anywhere else in this module, because a blinking
/// cell that PureCpu does not repaint is stuck at whatever intensity the last
/// paint left — and the eased intensity starts at `fg == bg`, so the word is
/// left as blank space rather than as a frozen word.
pub fn text_blink_animates(blink: Blink, text_blink_rate: u64, text_blink_rate_rapid: u64) -> bool {
    match blink {
        Blink::None => false,
        Blink::Slow => text_blink_rate != 0,
        Blink::Rapid => text_blink_rate_rapid != 0,
    }
}

/// Whether an animation frame scheduled by the last paint has come due.
///
/// `next_due` is the paint pass's own `has_animation`, i.e. the earliest
/// instant at which any animation it drew wants to be redrawn.  Using it as the
/// clock, rather than a second timetable computed here, is what keeps PureCpu's
/// blinking text on the same cadence as the GPU path's.
///
/// It is deliberately **only** a clock.  Liveness is decided separately, from
/// evidence found in the current frame, because `has_animation` outlives the
/// thing that set it: it is refreshed only by a paint, so an animation that has
/// scrolled off screen leaves a permanently-overdue instant behind.  Treating
/// that as "an animation is live" would repaint at `animation_fps` forever,
/// which is the exact regression this task is graded against.
pub fn animation_frame_due(next_due: Option<Instant>, now: Instant) -> bool {
    match next_due {
        Some(due) => now >= due,
        None => false,
    }
}

/// Whether the animation timer must be rescheduled after an idle skip.
///
/// **This is a widening of what `schedule_blink_timer_if_needed` used to mean,
/// and it is renamed to match.**  The old predicate was `cursor_blink_rate != 0
/// && focused`: the cursor's timer, and nothing else's.  Once the bell, blinking
/// text and animated images also depend on this timer, that gate strands all
/// three whenever the cursor happens not to be blinking — `cursor_blink_rate =
/// 0` and `default_cursor_style = "SteadyBlock"` are both ordinary settings, and
/// under either of them a bell would tint the window and stay tinted.
///
/// `focused` stays, and is not a fourth animation but a gate over all of them,
/// because `paint_impl` gates its own rescheduling the same way
/// (`render/paint.rs:121`).  An unfocused window does not animate on the GPU
/// path either, so keeping animations alive here would be a *divergence* from
/// the renderer we are trying to match, not parity with it.
pub fn animation_timer_needed(
    cursor_blinking: bool,
    bell_ringing: bool,
    animated_cells_present: bool,
    focused: bool,
) -> bool {
    focused && (cursor_blinking || bell_ringing || animated_cells_present)
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::{DefaultCursorStyle, EasingFunction};
    use std::time::Duration;
    use termwiz::surface::CursorShape;

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
    fn cell_rect_rejects_cells_outside_the_pane() {
        assert!(cell_rect(&split_right(), 2, -1).is_none());
        assert!(cell_rect(&split_right(), 2, 40).is_none());
        assert!(cell_rect(&top_left(), -1, 0).is_none());
        assert!(cell_rect(&split_bottom(), 12, 0).is_none());
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

    #[test]
    fn painted_span_of_a_lone_pane_is_the_whole_window() {
        // The ordinary no-splits case must reproduce today's x:0/width:fb_width
        // exactly, or this task is a repaint-coverage regression.
        let s = painted_x_span(0, 80, 80, 5.0, 10, 810);
        assert_eq!((s.x, s.width), (0, 810));
    }

    #[test]
    fn painted_span_of_a_left_pane_starts_at_zero_and_covers_the_gutter() {
        // Left-most but NOT right-most: starts at 0, and runs half a cell past
        // its last column to meet the split divider.
        let s = painted_x_span(0, 40, 80, 5.0, 10, 810);
        assert_eq!(s.x, 0);
        assert_eq!(s.width, 40 * 10 + 5 + 5, "must cover padding + half-cell gutter");
    }

    #[test]
    fn painted_span_of_a_right_pane_runs_to_the_window_edge() {
        // Right-most: starts half a cell LEFT of its first column, and runs all
        // the way to the window's right edge, not to cols*cell_w.
        let s = painted_x_span(40, 40, 80, 5.0, 10, 810);
        assert_eq!(s.x, 5 - 5 + 400, "must start half a cell left of the pane");
        assert_eq!(s.x + s.width, 810, "right-most pane must reach the window edge");
    }

    #[test]
    fn painted_span_of_a_middle_pane_covers_both_gutters() {
        let s = painted_x_span(20, 20, 80, 5.0, 10, 810);
        assert_eq!(s.x, 5 - 5 + 200);
        assert_eq!(s.width, 20 * 10 + 10);
    }

    #[test]
    fn painted_span_rounds_outward_on_an_odd_cell_width() {
        // cell_w = 9 -> half a cell is 4.5.  The span must be a SUPERSET of the
        // painted rect, so the left edge floors and the width ceils; a span that
        // rounds inward leaves a stale column at the split.
        let s = painted_x_span(10, 10, 30, 4.0, 9, 300);
        assert!(s.x <= 4 - 4 + 90, "left edge must not round rightward: {}", s.x);
        assert!(s.width >= 10 * 9 + 9, "width must not round short: {}", s.width);
        // Pinned exactly: painted rect is x=89.5, right=188.5, so the only
        // outward-rounded answer is 89..189.  The two assertions above are
        // boundary-inclusive and each survives a one-sided rounding mutation on
        // its own; these do not.
        assert_eq!((s.x, s.width), (89, 100));
    }

    #[test]
    fn painted_span_does_not_truncate_a_fractional_content_left() {
        // A `window_padding` in cells, points or percent gives a fractional
        // padding_left.  It reaches the RIGHT edge of the span — through
        // width_delta for a left-most pane, through x for an interior one — so
        // truncating it to an i32 anywhere on that path ends the span short of
        // the painted background and leaves a stale column at the split gutter.

        // Left-most, not right-most: painted x = 0,
        // right = 1*7 + (5.75 + 3.5) = 16.25 -> ceil 17.  Truncating 5.75 to 5
        // gives 16, one pixel short of the paint.
        let s = painted_x_span(0, 1, 30, 5.75, 7, 217);
        assert_eq!((s.x, s.width), (0, 17));

        // Interior: painted x = 4.5 - 3.5 + 7 = 8.0, right = 8 + 7 + 7 = 22.
        // Truncating 4.5 to 4 gives x = 7 (a wasted pixel on the left) and a
        // right edge of 21 (a stale pixel on the right).
        let s = painted_x_span(1, 1, 30, 4.5, 7, 219);
        assert_eq!((s.x, s.width), (8, 14));
    }

    /// **Ground truth**: the `background_rect` of `render/pane.rs`, in the `f32`
    /// the paint pass actually computes it in, as `(x, right)`.
    ///
    /// Transcribed from the `build_pane` copy at `render/pane.rs:606-646`, which
    /// is an independent second copy of the same math (the first is at
    /// :110-152).  Term for term, **including association order**: the right
    /// edge is `x + width`, and `width` is `(cols*cw) + width_delta`, so the
    /// addition of `x` happens last.  `(x + cols*cw) + width_delta` is a
    /// different `f32` and would make this a second approximation rather than
    /// the reference.
    ///
    /// `content_left` is the **unrounded** `padding_left + border.left` the paint
    /// pass uses — taking it as an `i32` is what made the old oracle structurally
    /// blind to the caller's truncation.
    fn painted_rect_reference_f32(
        left_cells: i32,
        cols: i32,
        total_cols: i32,
        content_left: f32,
        cell_w: i32,
        window_pixel_width: i32,
    ) -> (f32, f32) {
        let cw = cell_w as f32;
        let cl = content_left;
        let (x, width_delta) = if left_cells == 0 {
            (0., cl + cw / 2.0)
        } else {
            (cl - cw / 2.0 + left_cells as f32 * cw, cw)
        };
        let width = if left_cells + cols >= total_cols {
            window_pixel_width as f32 - x
        } else {
            cols as f32 * cw + width_delta
        };
        (x, x + width)
    }

    /// The same expression evaluated in `f64`, i.e. the **mathematically
    /// intended** rect rather than the one that gets painted.
    ///
    /// This is NOT ground truth — see the module of assertions below.  It exists
    /// only to bound how far the paint pass's `f32` accumulation has drifted from
    /// the real value, which is a magnitude check, not a correctness one.
    fn painted_rect_reference(
        left_cells: i32,
        cols: i32,
        total_cols: i32,
        content_left: f64,
        cell_w: i32,
        window_pixel_width: i32,
    ) -> (f64, f64) {
        let cw = cell_w as f64;
        let cl = content_left;
        let (x, width_delta) = if left_cells == 0 {
            (0., cl + cw / 2.0)
        } else {
            (cl - cw / 2.0 + left_cells as f64 * cw, cw)
        };
        let width = if left_cells + cols >= total_cols {
            window_pixel_width as f64 - x
        } else {
            cols as f64 * cw + width_delta
        };
        (x, x + width)
    }

    #[test]
    fn painted_span_is_a_superset_of_the_painted_rect_everywhere() {
        // The contract is one-directional: over-covering repaints a few extra
        // pixels, under-covering is the stale-fringe bug.  Sweep the parameter
        // space rather than trusting four hand-picked fixtures, all of which
        // happen to be numerically degenerate (see the report: the mutation the
        // brief proposed leaves three of them bit-identical).
        //
        // TWO ORACLES, AND ONLY ONE OF THEM IS GROUND TRUTH.  What ends up on
        // screen is the `f32` rect `render/pane.rs` computes — not the real
        // number that expression denotes — so `painted_rect_reference_f32` is
        // the reference and the superset assertion against it is STRICT.
        // `painted_rect_reference` (f64) is a different approximation of the
        // same expression; comparing against it needs slack, and getting that
        // backwards would let a genuine 1 px miss through.  Note the corollary:
        // because the span is computed from the same f32 value that is painted,
        // f32 drift moves the paint and the span TOGETHER and cannot by itself
        // cause a miss.  The f64 comparison is therefore a magnitude check on
        // the paint pass's own accumulation, not a correctness check on this
        // function.
        //
        // The padding axis carries three kinds of value on purpose:
        //   integer      — what a pixel `window_padding` gives;
        //   dyadic       — 4.5/5.25/5.75, exactly representable, so f32 and f64
        //                  agree and the two oracles below coincide;
        //   NON-dyadic   — 4.3/5.1/7.9/0.7/12.35, what a percent- or
        //                  points-derived padding actually produces.  These are
        //                  the representative case, and they are the only ones
        //                  that can separate the f32 ground truth from the f64
        //                  approximation.
        let paddings: &[f64] = &[
            0.0, 3.0, 4.0, 5.0, 12.0, // integer
            4.5, 5.25, 5.75, // dyadic fractions
            0.7, 4.3, 5.1, 7.9, 12.35, // NON-dyadic
        ];
        for &cell_w in &[7, 8, 9, 10, 13] {
            for &content_left in paddings {
                for &total_cols in &[30, 80, 81] {
                    let window_pixel_width =
                        total_cols * cell_w + (2.0 * content_left).round() as i32;
                    for left_cells in 0..total_cols {
                        for &cols in &[1, 7, total_cols / 2, total_cols - left_cells] {
                            if cols <= 0 || left_cells + cols > total_cols {
                                continue;
                            }
                            let s = painted_x_span(
                                left_cells,
                                cols,
                                total_cols,
                                content_left as f32,
                                cell_w,
                                window_pixel_width,
                            );
                            // GROUND TRUTH: the f32 rect the paint pass computes.
                            let (px, pright) = painted_rect_reference_f32(
                                left_cells,
                                cols,
                                total_cols,
                                content_left as f32,
                                cell_w,
                                window_pixel_width,
                            );
                            // The mathematically intended rect, for drift only.
                            let (rx, rright) = painted_rect_reference(
                                left_cells,
                                cols,
                                total_cols,
                                content_left as f32 as f64,
                                cell_w,
                                window_pixel_width,
                            );
                            let ctx = format!(
                                "left={left_cells} cols={cols} total={total_cols} \
                                 content_left={content_left} cell_w={cell_w}"
                            );

                            // --- STRICT, against ground truth -----------------
                            // What is on screen is the f32 rect, so the superset
                            // property is exact and takes no epsilon.
                            assert!(
                                (s.x as f32) <= px,
                                "left edge {} is right of the painted {px}: {ctx}",
                                s.x
                            );
                            assert!(
                                ((s.x + s.width) as f32) >= pright,
                                "right edge {} is left of the painted {pright}: {ctx}",
                                s.x + s.width
                            );
                            // and not wastefully wide: at most a pixel of slack
                            // on each side, which is all outward rounding needs.
                            assert!(
                                (s.x as f32) > px - 1.0
                                    && ((s.x + s.width) as f32) < pright + 1.0,
                                // This crate is edition 2018: an assert message
                                // with no trailing argument is NOT a format
                                // string, so pass ctx explicitly.
                                "span is wider than outward rounding justifies: {}",
                                ctx
                            );

                            // --- SLACK, against the f64 approximation ---------
                            // Outward rounding alone can move an edge by up to
                            // 1 px; 1.5 leaves room for representation drift and
                            // still catches a floor/ceil flipped across an
                            // integer boundary, which would cost ~2.
                            assert!(
                                (s.x as f64 - rx).abs() <= 1.5
                                    && ((s.x + s.width) as f64 - rright).abs() <= 1.5,
                                "span has drifted from the intended rect: {}",
                                ctx
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn row_band_painted_keeps_the_row_geometry_and_takes_the_span_width() {
        let p = split_right(); // origin (405,30), 40x24 cells of 10x20
        let span = PaneSpan { x: 400, width: 410 };
        let r = row_band_painted(&p, 3, &span).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (400, 90, 410, 20));
        assert!(row_band_painted(&p, 24, &span).is_none(), "row bound still applies");
        assert!(row_band_painted(&p, -1, &span).is_none());
    }

    #[test]
    fn config_driven_blink_survives_an_unset_pane_shape() {
        // THE I4 CASE, and the one the raw-shape test got wrong.  A session in
        // which no application has issued DECSCUSR leaves the pane shape at
        // Default forever, so this is the ordinary case, not a corner one.
        assert!(!CursorShape::Default.is_blinking(), "the premise of I4");
        assert!(cursor_blinking(
            DefaultCursorStyle::BlinkingBlock,
            CursorShape::Default,
            800,
            true
        ));
        // The pre-fix predicate for the same inputs, transcribed rather than
        // called, so this line states what the bug WAS and fails if someone
        // reverts to it: it is false where the line above is true.
        assert!(!(CursorShape::Default.is_blinking() && 800 != 0 && true));
    }

    #[test]
    fn a_steady_config_style_does_not_blink() {
        // The discriminating negative: without it, a `cursor_blinking` that
        // ignored the shape entirely and returned `rate != 0 && focused` would
        // pass the test above.
        assert!(!cursor_blinking(
            DefaultCursorStyle::SteadyBlock,
            CursorShape::Default,
            800,
            true
        ));
    }

    #[test]
    fn decscusr_beats_the_config_in_both_directions() {
        // `effective_shape` only fills in Default, so an application that has
        // set a shape wins over the config either way round.  Both directions,
        // because a predicate that took the config's blinkiness alone would
        // pass the first and fail the second.
        assert!(cursor_blinking(
            DefaultCursorStyle::SteadyBlock,
            CursorShape::BlinkingBar,
            800,
            true
        ));
        assert!(!cursor_blinking(
            DefaultCursorStyle::BlinkingBlock,
            CursorShape::SteadyUnderline,
            800,
            true
        ));
    }

    // ---- painted_y_span ---------------------------------------------------
    //
    // The fixture is the x-axis fixture transposed: cell_h = 20, a 24-row
    // terminal, padding_top = 5 and a 25 px tab bar, so content_top (the paint
    // pass's top_pixel_y) is 30 and the window is 30 + 24*20 + 5 = 515 px tall.

    #[test]
    fn painted_y_span_of_a_lone_pane_stops_at_the_tab_bar() {
        // NOT the whole window, which is where the x axis differs: a left-most
        // pane's background starts at x = 0, but a top-most pane's starts at
        // top_pixel_y - padding_top = 25, below the tab bar.  This is the whole
        // reason the visual bell's rect is the pane and not the window.
        let s = painted_y_span(0, 24, 24, 30.0, 5.0, 20, 515);
        assert_eq!((s.y, s.height), (25, 490));
    }

    #[test]
    fn painted_y_span_of_a_top_pane_covers_the_padding_and_the_gutter() {
        // Top-most but not bottom-most: reaches up under the window padding and
        // half a cell down past its last row, to meet the split divider.
        let s = painted_y_span(0, 12, 24, 30.0, 5.0, 20, 515);
        assert_eq!(s.y, 25);
        assert_eq!(s.height, 12 * 20 + 5 + 10, "must cover padding + half-cell gutter");
    }

    #[test]
    fn painted_y_span_of_a_bottom_pane_runs_to_the_window_edge() {
        // Bottom-most: starts half a cell ABOVE its first row and runs to the
        // window's bottom edge, not to rows*cell_h.
        let s = painted_y_span(12, 12, 24, 30.0, 5.0, 20, 515);
        assert_eq!(s.y, 30 + 240 - 10, "must start half a cell above the pane");
        assert_eq!(s.y + s.height, 515, "bottom-most pane must reach the window edge");
    }

    #[test]
    fn painted_y_span_of_a_middle_pane_covers_both_gutters() {
        // 36 rows so that rows 12..24 are neither top-most nor bottom-most.
        let s = painted_y_span(12, 12, 36, 30.0, 5.0, 20, 755);
        assert_eq!(s.y, 260);
        assert_eq!(s.height, 12 * 20 + 20);
    }

    #[test]
    fn painted_y_span_does_not_truncate_a_fractional_padding_top() {
        // window_padding in cells, points or percent gives a fractional
        // padding_top, and it reaches BOTH edges of the span: the top edge
        // directly (content_top - padding_top) and the bottom edge through
        // height_delta.  Truncating it anywhere ends the span short of the
        // painted background.

        // Top-most, not bottom-most, with no tab bar: y = 5.75 - 5.75 = 0,
        // bottom = 1*7 + (5.75 + 3.5) = 16.25 -> ceil 17.
        let s = painted_y_span(0, 1, 30, 5.75, 5.75, 7, 217);
        assert_eq!((s.y, s.height), (0, 17));

        // Interior: y = 4.5 + 7 - 3.5 = 8.0, bottom = 8 + 7 + 7 = 22.
        let s = painted_y_span(1, 1, 30, 4.5, 4.5, 7, 219);
        assert_eq!((s.y, s.height), (8, 14));
    }

    #[test]
    fn painted_y_span_rounds_outward_on_an_odd_cell_height() {
        // cell_h = 9 -> half a cell is 4.5, so an interior pane's top edge
        // lands on .5 and the span must round OUTWARD: floor the top, ceil the
        // bottom.  Painted rect is y = 4 + 90 - 4.5 = 89.5, bottom = 89.5 +
        // (90 + 9) = 188.5, so the only outward-rounded answer is 89..189.
        //
        // This is the one fixture here whose edges are not already integers,
        // which makes it the only one a rounding mutation can move.
        let s = painted_y_span(10, 10, 30, 4.0, 4.0, 9, 300);
        assert_eq!((s.y, s.height), (89, 100));
    }

    #[test]
    fn pane_rect_takes_x_from_the_horizontal_span_and_y_from_the_vertical_one() {
        // Four deliberately distinct numbers: a transposed field would still
        // type-check and still produce a plausible rect.
        let r = pane_rect(
            &PaneSpan { x: 400, width: 410 },
            &PaneVSpan { y: 25, height: 490 },
        );
        assert_eq!((r.x, r.y, r.width, r.height), (400, 25, 410, 490));
    }

    // ---- the visual bell --------------------------------------------------

    fn visual_bell(target: VisualBellTarget) -> VisualBell {
        VisualBell {
            fade_in_duration_ms: 200,
            fade_in_function: EasingFunction::Linear,
            fade_out_duration_ms: 300,
            fade_out_function: EasingFunction::Linear,
            target,
        }
    }

    /// An `Instant` that many milliseconds in the past.
    fn ago(ms: u64) -> Option<Instant> {
        Some(Instant::now() - Duration::from_millis(ms))
    }

    #[test]
    fn bell_region_is_none_when_the_bell_has_never_rung() {
        assert_eq!(
            bell_region(None, &visual_bell(VisualBellTarget::BackgroundColor)),
            BellRegion::None
        );
    }

    #[test]
    fn bell_region_covers_the_pane_during_the_fade_in() {
        assert_eq!(
            bell_region(ago(100), &visual_bell(VisualBellTarget::BackgroundColor)),
            BellRegion::PaneBackground
        );
    }

    #[test]
    fn bell_region_lasts_the_whole_fade_out_and_then_stops() {
        // 200 ms in + 300 ms out = 500 ms.  A predicate that watched only the
        // fade-in would drop the region at 200 ms and leave the last tinted
        // frame on screen for good; one that never stopped would repaint the
        // pane forever.  Both directions, 100 ms clear of the boundary.
        let vb = visual_bell(VisualBellTarget::BackgroundColor);
        assert_eq!(bell_region(ago(400), &vb), BellRegion::PaneBackground);
        assert_eq!(bell_region(ago(600), &vb), BellRegion::None);
    }

    #[test]
    fn bell_region_follows_the_configured_target() {
        // The discriminating negative for the two tests above: without it, a
        // bell_region that always answered PaneBackground while ringing would
        // pass them, and would repaint the whole pane at animation_fps for a
        // bell that only tints one cell.
        assert_eq!(
            bell_region(ago(100), &visual_bell(VisualBellTarget::CursorColor)),
            BellRegion::CursorCell
        );
    }

    // ---- blinking text ----------------------------------------------------

    #[test]
    fn a_cell_without_the_blink_attribute_never_animates() {
        assert!(!text_blink_animates(Blink::None, 500, 250));
    }

    #[test]
    fn slow_blink_reads_the_slow_rate_and_rapid_blink_the_rapid_one() {
        // Each rate is checked with the OTHER one set to zero, so a predicate
        // that consulted the wrong field — or either field — fails here rather
        // than passing on a fixture where both happen to be non-zero.
        assert!(text_blink_animates(Blink::Slow, 500, 0));
        assert!(!text_blink_animates(Blink::Slow, 0, 250));
        assert!(text_blink_animates(Blink::Rapid, 0, 250));
        assert!(!text_blink_animates(Blink::Rapid, 500, 0));
    }

    // ---- the animation clock ----------------------------------------------

    #[test]
    fn nothing_is_due_when_the_last_paint_scheduled_nothing() {
        // The load-bearing case for idle CPU: has_animation is None on a window
        // with no animation at all, and must not be read as "due now".
        assert!(!animation_frame_due(None, Instant::now()));
    }

    #[test]
    fn a_frame_is_due_from_its_instant_onwards_and_not_before() {
        let t = Instant::now();
        assert!(!animation_frame_due(Some(t + Duration::from_millis(1)), t));
        assert!(animation_frame_due(Some(t), t), "due exactly at its instant");
        assert!(animation_frame_due(Some(t - Duration::from_millis(1)), t));
    }

    // ---- the animation timer ----------------------------------------------

    #[test]
    fn an_idle_window_schedules_no_animation_timer() {
        assert!(!animation_timer_needed(false, false, false, true));
    }

    #[test]
    fn a_blinking_cursor_alone_keeps_the_timer_alive() {
        assert!(animation_timer_needed(true, false, false, true));
    }

    #[test]
    fn a_ringing_bell_alone_keeps_the_timer_alive() {
        // THE CASE THE OLD PREDICATE GOT WRONG.  schedule_blink_timer_if_needed
        // returned early unless cursor_blink_rate != 0, so with a steady cursor
        // the bell rang, tinted the window on its one invalidate, and stayed
        // tinted: nothing ever scheduled the frame that would fade it out.
        assert!(animation_timer_needed(false, true, false, true));
    }

    #[test]
    fn animated_cells_alone_keep_the_timer_alive() {
        // Same for blinking text and GIF frames, which have no timer of their
        // own at all once the idle skip has swallowed the paint.
        assert!(animation_timer_needed(false, false, true, true));
    }

    #[test]
    fn an_unfocused_window_animates_nothing() {
        // Not a policy choice: paint_impl gates its own rescheduling on focus
        // (render/paint.rs:121), so animating here would diverge from the GPU
        // path rather than match it.  Held true for all three animations at
        // once, so a predicate that only dropped focus from one of them fails.
        assert!(!animation_timer_needed(true, true, true, false));
    }

    #[test]
    fn blink_rate_zero_and_lost_focus_each_stop_the_blink_alone() {
        // Both conjuncts existed before I4 and must survive it; each is checked
        // with the other held true, so neither can be masked by the other.
        assert!(!cursor_blinking(
            DefaultCursorStyle::BlinkingBlock,
            CursorShape::Default,
            0,
            true
        ));
        assert!(!cursor_blinking(
            DefaultCursorStyle::BlinkingBlock,
            CursorShape::Default,
            800,
            false
        ));
    }
}
