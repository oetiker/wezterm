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
}
