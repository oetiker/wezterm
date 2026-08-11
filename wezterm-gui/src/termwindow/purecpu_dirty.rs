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
/// Mirrors `render/pane.rs:110-152`.  A pane's background is deliberately
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

/// Halve a half-pixel value, rounding down (towards -inf).
fn floor_half(v: i32) -> i32 {
    v.div_euclid(2)
}

/// Halve a half-pixel value, rounding up (towards +inf).
fn ceil_half(v: i32) -> i32 {
    (v + 1).div_euclid(2)
}

/// Compute [`PaneSpan`] for a pane.
///
/// `content_left` is `padding_left + border.left`, already truncated to an
/// `i32` by the caller.  All arithmetic happens in *half-pixels* so that the
/// `cell_width / 2.0` of the paint pass is exact for an odd `cell_w`; only the
/// final conversion rounds, and it rounds **outward** — floor on the left edge,
/// ceil on the right — so the span is always a superset of the painted rect.
/// Over-covering costs a few repainted pixels; under-covering is the stale
/// fringe this exists to prevent.
pub fn painted_x_span(
    left_cells: i32,
    cols: i32,
    total_cols: i32,
    content_left: i32,
    cell_w: i32,
    window_pixel_width: i32,
) -> PaneSpan {
    // `x` and `width_delta` of render/pane.rs, in half-pixels.
    let (x2, width_delta2) = if left_cells == 0 {
        (0, 2 * content_left + cell_w)
    } else {
        (
            2 * content_left - cell_w + 2 * left_cells * cell_w,
            2 * cell_w,
        )
    };

    let right2 = if left_cells + cols >= total_cols {
        // Go all the way to the right edge if we're right-most.
        2 * window_pixel_width
    } else {
        x2 + 2 * cols * cell_w + width_delta2
    };

    let x = floor_half(x2);
    PaneSpan {
        x,
        width: (ceil_half(right2) - x).max(0),
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
        let s = painted_x_span(0, 80, 80, 5, 10, 810);
        assert_eq!((s.x, s.width), (0, 810));
    }

    #[test]
    fn painted_span_of_a_left_pane_starts_at_zero_and_covers_the_gutter() {
        // Left-most but NOT right-most: starts at 0, and runs half a cell past
        // its last column to meet the split divider.
        let s = painted_x_span(0, 40, 80, 5, 10, 810);
        assert_eq!(s.x, 0);
        assert_eq!(s.width, 40 * 10 + 5 + 5, "must cover padding + half-cell gutter");
    }

    #[test]
    fn painted_span_of_a_right_pane_runs_to_the_window_edge() {
        // Right-most: starts half a cell LEFT of its first column, and runs all
        // the way to the window's right edge, not to cols*cell_w.
        let s = painted_x_span(40, 40, 80, 5, 10, 810);
        assert_eq!(s.x, 5 - 5 + 400, "must start half a cell left of the pane");
        assert_eq!(s.x + s.width, 810, "right-most pane must reach the window edge");
    }

    #[test]
    fn painted_span_of_a_middle_pane_covers_both_gutters() {
        let s = painted_x_span(20, 20, 80, 5, 10, 810);
        assert_eq!(s.x, 5 - 5 + 200);
        assert_eq!(s.width, 20 * 10 + 10);
    }

    #[test]
    fn painted_span_rounds_outward_on_an_odd_cell_width() {
        // cell_w = 9 -> half a cell is 4.5.  The span must be a SUPERSET of the
        // painted rect, so the left edge floors and the width ceils; a span that
        // rounds inward leaves a stale column at the split.
        let s = painted_x_span(10, 10, 30, 4, 9, 300);
        assert!(s.x <= 4 - 4 + 90, "left edge must not round rightward: {}", s.x);
        assert!(s.width >= 10 * 9 + 9, "width must not round short: {}", s.width);
        // Pinned exactly: painted rect is x=89.5, right=188.5, so the only
        // outward-rounded answer is 89..189.  The two assertions above are
        // boundary-inclusive and each survives a one-sided rounding mutation on
        // its own; these do not.
        assert_eq!((s.x, s.width), (89, 100));
    }

    /// The f32 arithmetic of `render/pane.rs:110-152`, in f64, as the oracle.
    fn painted_rect_reference(
        left_cells: i32,
        cols: i32,
        total_cols: i32,
        content_left: i32,
        cell_w: i32,
        window_pixel_width: i32,
    ) -> (f64, f64) {
        let cw = cell_w as f64;
        let cl = content_left as f64;
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
        for &cell_w in &[7, 8, 9, 10, 13] {
            for &content_left in &[0, 3, 4, 5, 12] {
                for &total_cols in &[30, 80, 81] {
                    let window_pixel_width = total_cols * cell_w + 2 * content_left;
                    for left_cells in 0..total_cols {
                        for &cols in &[1, 7, total_cols / 2, total_cols - left_cells] {
                            if cols <= 0 || left_cells + cols > total_cols {
                                continue;
                            }
                            let s = painted_x_span(
                                left_cells,
                                cols,
                                total_cols,
                                content_left,
                                cell_w,
                                window_pixel_width,
                            );
                            let (rx, rright) = painted_rect_reference(
                                left_cells,
                                cols,
                                total_cols,
                                content_left,
                                cell_w,
                                window_pixel_width,
                            );
                            let ctx = format!(
                                "left={left_cells} cols={cols} total={total_cols} \
                                 content_left={content_left} cell_w={cell_w}"
                            );
                            assert!(
                                (s.x as f64) <= rx,
                                "left edge {} is right of the painted {rx}: {ctx}",
                                s.x
                            );
                            assert!(
                                ((s.x + s.width) as f64) >= rright,
                                "right edge {} is left of the painted {rright}: {ctx}",
                                s.x + s.width
                            );
                            // and not wastefully wide: at most a pixel of slack
                            // on each side, which is all outward rounding needs.
                            assert!(
                                (s.x as f64) > rx - 1.0
                                    && ((s.x + s.width) as f64) < rright + 1.0,
                                // This crate is edition 2018: an assert message
                                // with no trailing argument is NOT a format
                                // string, so pass ctx explicitly.
                                "span is wider than outward rounding justifies: {}",
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
}
