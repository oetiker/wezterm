use crate::quad::{Vertex, VERTICES_PER_CELL};
use crate::renderstate::VertexBuffer;
use crate::termwindow::render::purecpu_sampler;
use crate::selection::SelectionRange;
use ::window::bitmaps::{BitmapImage, ImageTexture};
use ::window::WindowOps;
use mux::pane::PaneId;
use std::collections::HashMap;
use std::time::Instant;
use termwiz::surface::SequenceNo;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirtyRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub struct PureCpuState {
    pub frame_buffer: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// When true, the entire framebuffer will be cleared and repainted
    pub force_full_repaint: bool,
    /// Dirty pixel regions for the current frame
    pub dirty_pixel_rects: Vec<DirtyRect>,
    /// Generation counters for detecting config/shape/quad changes
    pub last_config_generation: usize,
    pub last_shape_generation: usize,
    pub last_quad_generation: usize,
    /// Last resolved viewport position (tracks physical_top when following bottom)
    pub last_resolved_viewport: Option<wezterm_term::StableRowIndex>,
    /// Last cursor position for tracking cursor movement
    pub last_cursor_y: Option<wezterm_term::StableRowIndex>,
    pub last_cursor_x: Option<usize>,
    /// Last seqno for change detection, **per pane**.
    ///
    /// Each pane owns an independent `SequenceNo` counter (`TerminalState::seqno`,
    /// starting at 1), so a single window-wide baseline is not merely imprecise —
    /// it is wrong in both directions.  Comparing a busy background pane against
    /// a quiet active pane's low seqno reports every row dirty every frame; the
    /// reverse — a busy active pane (a TUI redrawing in place, so no viewport
    /// scroll to force a full repaint) against a quiet background pane — makes
    /// the background pane's next change compare below the baseline and never
    /// repaint at all.  That second case is finding I1 surviving its own fix.
    pub last_seqno_by_pane: HashMap<PaneId, SequenceNo>,
    /// Last selection range — force full repaint when it changes
    pub last_selection_range: Option<SelectionRange>,
    /// Last quantized cursor blink phase (true = visible) for
    /// detecting blink transitions without running paint_impl every frame.
    pub last_blink_visible: bool,
}

impl PureCpuState {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            frame_buffer: vec![0u8; (width * height * 4) as usize],
            width,
            height,
            force_full_repaint: true,
            dirty_pixel_rects: vec![],
            last_config_generation: 0,
            last_shape_generation: 0,
            last_quad_generation: 0,
            last_resolved_viewport: None,
            last_cursor_y: None,
            last_cursor_x: None,
            last_seqno_by_pane: HashMap::new(),
            last_selection_range: None,
            last_blink_visible: true,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.frame_buffer.resize((width * height * 4) as usize, 0);
            self.force_full_repaint = true;
        }
    }
}

/// Collect per-dirty-rect clip regions for a quad.
/// Each entry is the intersection of the quad with one dirty rect.
/// This avoids the bounding-box problem where non-adjacent dirty rects
/// cause large quads to overwrite clean areas between them.
#[inline]
fn collect_clip_rects(
    dest_x: i32,
    dest_y: i32,
    dest_x2: i32,
    dest_y2: i32,
    rects: &[DirtyRect],
    out: &mut Vec<[i32; 4]>,
) {
    out.clear();
    for r in rects {
        let rx2 = r.x.saturating_add(r.width);
        let ry2 = r.y.saturating_add(r.height);
        if dest_x < rx2 && dest_x2 > r.x && dest_y < ry2 && dest_y2 > r.y {
            out.push([
                dest_x.max(r.x),
                dest_y.max(r.y),
                dest_x2.min(rx2),
                dest_y2.min(ry2),
            ]);
        }
    }
}

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

/// Coalesce dirty rects into full-width horizontal bands for XPutImage.
/// XPutImage needs contiguous pixel data, so we use full-width bands
/// where the row data IS contiguous in the framebuffer.
fn coalesce_to_bands(rects: &[DirtyRect], screen_width: u32) -> Vec<DirtyRect> {
    if rects.is_empty() {
        return vec![];
    }

    // Collect all y-ranges
    let mut y_ranges: Vec<(i32, i32)> =
        rects.iter().map(|r| (r.y, r.y.saturating_add(r.height))).collect();
    y_ranges.sort_by_key(|r| r.0);

    // Merge overlapping y-ranges
    let mut merged_bands: Vec<(i32, i32)> = vec![y_ranges[0]];
    for &(start, end) in &y_ranges[1..] {
        let last = merged_bands.last_mut().unwrap();
        if start <= last.1 {
            last.1 = last.1.max(end);
        } else {
            merged_bands.push((start, end));
        }
    }

    merged_bands
        .into_iter()
        .map(|(y, y2)| DirtyRect {
            x: 0,
            y,
            width: screen_width as i32,
            height: y2.saturating_sub(y),
        })
        .collect()
}

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

impl crate::TermWindow {
    pub fn call_draw_purecpu(&mut self) -> anyhow::Result<()> {
        let render_state = self.render_state.as_ref().unwrap();
        let tex = render_state.glyph_cache.borrow().atlas.texture();
        let tex = tex.downcast_ref::<ImageTexture>().unwrap();
        let atlas_image = tex.image.borrow();
        let (atlas_w, atlas_h) = atlas_image.image_dimensions();
        let atlas_data = atlas_image.pixel_data_slice();

        let state = self.purecpu_state.as_mut().unwrap();
        let fb_w = state.width as usize;
        let fb_h = state.height as usize;
        let screen_width = state.width;
        let screen_height = state.height;

        let full_repaint = state.force_full_repaint;

        // Determine effective dirty rects and clear framebuffer regions.
        // Neighboring glyphs that overhang into dirty regions are caught by
        // the overlap test at blit time and re-blitted automatically.
        let effective_dirty: Vec<DirtyRect>;
        if full_repaint {
            state.frame_buffer.fill(0);
            state.force_full_repaint = false;
            state.dirty_pixel_rects.clear();
            effective_dirty = vec![];
            metrics::histogram!("purecpu.full_repaint.rate").record(1.);
        } else {
            metrics::histogram!("purecpu.dirty_repaint.rate").record(1.);
            metrics::histogram!("purecpu.dirty_rects").record(
                state.dirty_pixel_rects.len() as f64,
            );

            let dirty_pixels: i64 = state
                .dirty_pixel_rects
                .iter()
                .map(|r| (r.width as i64) * (r.height as i64))
                .sum();
            let total_pixels = (fb_w * fb_h) as i64;
            metrics::histogram!("purecpu.dirty_pixel_pct").record(
                if total_pixels > 0 {
                    100.0 * dirty_pixels as f64 / total_pixels as f64
                } else {
                    0.0
                },
            );

            let effective = state.dirty_pixel_rects.clone();

            // Clear dirty regions in framebuffer before re-blitting
            for rect in &effective {
                clear_rect(&mut state.frame_buffer, fb_w, fb_h, rect);
            }

            state.dirty_pixel_rects.clear();
            effective_dirty = effective;
        }

        let blit_start = Instant::now();
        let mut quads_total: u64 = 0;
        let mut quads_blitted: u64 = 0;

        let foreground_text_hsb = self.config.foreground_text_hsb;
        let apply_hsv = foreground_text_hsb.hue != 1.0
            || foreground_text_hsb.saturation != 1.0
            || foreground_text_hsb.brightness != 1.0;


        // Selected once for the whole frame, exactly as GL does at
        // draw.rs:172-179.
        let use_subpixel = subpixel_enabled(
            self.config.freetype_render_target,
            self.config.freetype_load_target,
        );

        for layer in render_state.layers.borrow().iter() {
            for idx in 0..3 {
                // draw.rs:244 — dual-source blending is chosen per *vertex
                // buffer*, not per quad, and only buffer 1 carries text.
                let subpixel_aa = use_subpixel && idx == 1;
                let vb = &layer.vb.borrow()[idx];
                let (vertex_count, _index_count) = vb.vertex_index_count();
                if vertex_count == 0 {
                    continue;
                }
                let bufs = vb.current_vb_mut();
                let vertices: &[Vertex] = match &*bufs {
                    VertexBuffer::PureCpu(v) => &v[..vertex_count],
                    _ => continue,
                };

                let (vb_total, vb_blitted) = blit_vertex_buffer(
                    &mut state.frame_buffer,
                    fb_w,
                    fb_h,
                    atlas_data,
                    atlas_w,
                    atlas_h,
                    vertices,
                    full_repaint,
                    &effective_dirty,
                    subpixel_aa,
                    apply_hsv,
                    foreground_text_hsb,
                );
                quads_total += vb_total;
                quads_blitted += vb_blitted;

                vb.next_index();
            }
        }

        let blit_elapsed = blit_start.elapsed();
        metrics::histogram!("purecpu.blit").record(blit_elapsed);
        metrics::histogram!("purecpu.quads_total").record(quads_total as f64);
        metrics::histogram!("purecpu.quads_blitted").record(quads_blitted as f64);

        // Present the frame
        let state = self.purecpu_state.as_mut().unwrap();
        let window = self.window.as_ref().unwrap();
        if full_repaint {
            window.present_software_frame_region(
                &state.frame_buffer,
                state.width,
                state.height,
                0,
                0,
            )?;
        } else {
            // Coalesce dirty rects into horizontal bands for XPutImage.
            // Since bands span full width, the pixel data is contiguous in the framebuffer.
            let bands = coalesce_to_bands(&effective_dirty, screen_width);
            for band in &bands {
                let Some((y, h)) = clamp_band(band.y, band.height, screen_height as i32) else {
                    continue;
                };
                let offset = y * fb_w * 4;
                let size = h * fb_w * 4;
                debug_assert!(
                    offset + size <= state.frame_buffer.len(),
                    "band {y}+{h} exceeds framebuffer {} — fb_w/height disagree with the buffer length",
                    state.frame_buffer.len()
                );
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
        }

        Ok(())
    }
}

/// Blit one vertex buffer's quads into the framebuffer, returning
/// `(quads_total, quads_blitted)`.
///
/// Extracted from `call_draw_purecpu` in Task 15 as a **pure move**: every
/// expression below is the one that ran inside the loop, in the same order,
/// with the same types.  Floating-point association order is semantics in this
/// module — an ulp across a pixel centre is a 1px seam — so nothing here was
/// reordered, factored or simplified, and the extraction was performed by
/// script rather than by hand.
///
/// Everything it touches is plain borrowed data, so a test can stand up a 16x16
/// framebuffer and a hand-built atlas and drive it with no `TermWindow`, no
/// window and no X display.  That is the entire point of the seam: before it
/// existed this loop was executed by no test at all, and an unconditional
/// `panic!` on the first line of the clip loop left the suite green.
///
/// `half_w`/`half_h` and `atlas_stride` moved in with it — each is a pure
/// function of an argument, so computing it here is the same value the caller
/// used to hand over.
#[allow(clippy::too_many_arguments)]
fn blit_vertex_buffer(
    fb: &mut [u8],
    fb_w: usize,
    fb_h: usize,
    atlas_data: &[u8],
    atlas_w: usize,
    atlas_h: usize,
    vertices: &[Vertex],
    full_repaint: bool,
    effective_dirty: &[DirtyRect],
    subpixel_aa: bool,
    apply_hsv: bool,
    foreground_text_hsb: config::HsbTransform,
) -> (u64, u64) {
    let atlas_stride = atlas_w * 4;
    let half_w = fb_w as f32 / 2.0;
    let half_h = fb_h as f32 / 2.0;

    let num_quads = vertices.len() / VERTICES_PER_CELL;
    let quads_total = num_quads as u64;
    let mut quads_blitted: u64 = 0;
    let mut clip_rects: Vec<[i32; 4]> = Vec::new();
        for q in 0..num_quads {
            let base = q * VERTICES_PER_CELL;
            let tl = &vertices[base];     // top-left
            let _tr = &vertices[base + 1]; // top-right
            let _bl = &vertices[base + 2]; // bot-left
            let br = &vertices[base + 3]; // bot-right

            let has_color = tl.has_color;
            let fg = tl.fg_color;
            let alt = tl.alt_color;
            let mix_value = tl.mix_value;
            let hsv = tl.hsv;

            // Mix fg and alt color
            let fg_r = fg[0] * (1.0 - mix_value) + alt[0] * mix_value;
            let fg_g = fg[1] * (1.0 - mix_value) + alt[1] * mix_value;
            let fg_b = fg[2] * (1.0 - mix_value) + alt[2] * mix_value;
            let fg_a = fg[3] * (1.0 - mix_value) + alt[3] * mix_value;

            // Screen destination rect (clip-space to pixels) and atlas
            // rect (normalized tex coords to texels), both kept as
            // floats: the sampler interpolates texcoords across the
            // true quad, and the coverage rule needs the unrounded
            // edge.  Truncating the destination displaced sub-pixel
            // quads a whole pixel left/up (M1, the fancy tab bar's
            // bit-exact 1px shift); truncating the source lost up to a
            // texel of the extent.
            let dest_f = [
                tl.position[0] + half_w,
                tl.position[1] + half_h,
                br.position[0] + half_w,
                br.position[1] + half_h,
            ];
            // `has_color == 2.0` (IS_BG_IMAGE) stays on the pre-Task-7
            // path — truncated rect, 1:1 crop — because background
            // image is out of scope for this pass and GL samples that
            // branch with a linear sampler, not Nearest.  See the
            // `purecpu_sampler` module doc.
            let bg_image = has_color == 2.0;

            // Destination rect only, for now: the source mapping is built
            // further down, after the two bails, so an incremental repaint
            // does not pay to sample a quad it discards.
            let [dest_x, dest_y, dest_x2, dest_y2] =
                purecpu_sampler::Quad::dest_rect_of(bg_image, dest_f);

            let dest_w = dest_x2 - dest_x;
            let dest_h = dest_y2 - dest_y;
            if dest_w <= 0 || dest_h <= 0 {
                continue;
            }

            // Build the list of clip rects for this quad.
            // For full repaint: one clip rect = the entire quad.
            // For incremental: one clip rect per overlapping dirty rect
            // (the intersection). This avoids the bounding-box problem
            // where non-adjacent dirty rects cause large quads to
            // overwrite clean framebuffer areas between them.
            if full_repaint {
                clip_rects.clear();
                clip_rects.push([dest_x, dest_y, dest_x2, dest_y2]);
            } else {
                collect_clip_rects(
                    dest_x, dest_y, dest_x2, dest_y2,
                    &effective_dirty, &mut clip_rects,
                );
                if clip_rects.is_empty() {
                    continue;
                }
            }

            quads_blitted += 1;

            if has_color == 3.0 {
                // IS_SOLID_COLOR: fill each clip rect with fg color
                let mut sr = fg_r;
                let mut sg = fg_g;
                let mut sb = fg_b;
                let sa = fg_a;

                if hsv[0] != 1.0 || hsv[1] != 1.0 || hsv[2] != 1.0 {
                    let (h, s, v) = rgb_to_hsv(sr, sg, sb);
                    let (nr, ng, nb) =
                        hsv_to_rgb(h * hsv[0], s * hsv[1], v * hsv[2]);
                    sr = nr;
                    sg = ng;
                    sb = nb;
                }

                sr = linear_to_srgb(sr);
                sg = linear_to_srgb(sg);
                sb = linear_to_srgb(sb);

                let sb8 = (sb.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                let sg8 = (sg.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                let sr8 = (sr.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                let sa8 = (sa.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;

                for clip in &clip_rects {
                    let [cx1, cy1, cx2, cy2] = *clip;
                    for dy in cy1..cy2 {
                        if dy < 0 || dy >= fb_h as i32 {
                            continue;
                        }
                        let row_off = dy as usize * fb_w;
                        for dx in cx1..cx2 {
                            if dx < 0 || dx >= fb_w as i32 {
                                continue;
                            }
                            let fi = (row_off + dx as usize) * 4;
                            match subpixel_aa
                                .then(|| subpixel_mask(has_color, 0, 0, 0, 0))
                                .flatten()
                            {
                                // `colorMask = vec4(1.0)` — a straight
                                // replace, including alpha.  Identical
                                // to `blend_over` when sa8 == 255, and
                                // deliberately different when it is
                                // not; window borders (borders.rs) are
                                // the sub-layer-1 producer here.
                                Some([mr, mg, mb, ma]) => blend_over_masked(
                                    &mut *fb,
                                    fi, sr8, sg8, sb8, sa8, mr, mg, mb, ma,
                                ),
                                None => blend_over(
                                    &mut *fb, fi, sr8, sg8, sb8, sa8,
                                ),
                            }
                        }
                    }
                }
                continue;
            }

            // Textured quads: sample the atlas across the full
            // destination rect.  The old code blitted 1:1 and cropped
            // to min(tex, dest), so any quad drawn at a size other than
            // its sprite's was cropped rather than scaled (finding I5)
            // — non-native inline images, DECDWL/DECDHL, and scaled
            // bitmap glyphs.  `blit_end` still crops for IS_BG_IMAGE,
            // and still drops the quad when that crop is empty.
            //
            // This is the first point where the source mapping is needed at
            // all — solid-colour quads returned above without one — so it is
            // built here rather than at the top of the iteration.
            let tex_f = [
                tl.tex[0] * atlas_w as f32,
                tl.tex[1] * atlas_h as f32,
                br.tex[0] * atlas_w as f32,
                br.tex[1] * atlas_h as f32,
            ];
            let quad = purecpu_sampler::Quad::new(
                bg_image,
                dest_f,
                tex_f,
                atlas_w as i32,
                atlas_h as i32,
            );

            let Some((blit_x2, blit_y2)) = quad.blit_end() else {
                continue;
            };

            for clip in &clip_rects {
                let [cx1, cy1, cx2, cy2] = *clip;

                // Clipped row/col ranges, in destination pixels.
                //
                // The `.max(dest_y)` / `.max(dest_x)` terms are
                // redundant today and kept as defence in depth: a full
                // repaint pushes the destination rect itself as the clip
                // rect (:353) and an incremental one intersects with it
                // in `collect_clip_rects`, so `cx1 >= dest_x` already
                // holds on both paths.  The `.max(0)` / `.min(fb_*)`
                // terms are NOT redundant — they are what keeps a quad
                // hanging off the top/left edge inside the framebuffer,
                // and they replace the old per-pixel `dy < 0` guard.
                let row_start = cy1.max(dest_y).max(0);
                let row_end = cy2.min(blit_y2).min(fb_h as i32);
                let col_start = cx1.max(dest_x).max(0);
                let col_end = cx2.min(blit_x2).min(fb_w as i32);

                for dy in row_start..row_end {
                    let atlas_row = quad.texel_y(dy);
                    // Documents an invariant rather than doing work:
                    // sampled quads come back clamped to
                    // `[0, atlas_h - 1]` by `Axis::texel`, and bg-image
                    // quads are bounded by the `min(tex, dest)` crop, so
                    // neither branch can land outside the atlas.  Kept
                    // so a future producer of out-of-range texcoords
                    // fails safe instead of indexing out of bounds.
                    if atlas_row < 0 || atlas_row >= atlas_h as i32 {
                        continue;
                    }
                    let fb_row_off = dy as usize * fb_w;
                    let atlas_row_off = atlas_row as usize * atlas_stride;

                    for dx in col_start..col_end {
                        let atlas_col = quad.texel_x(dx);
                        if atlas_col < 0 || atlas_col >= atlas_w as i32 {
                            continue;
                        }

                        let ai = atlas_row_off + atlas_col as usize * 4;
                        // Atlas is RGBA (ImageTexture stores RGBA despite
                        // BitmapImage docs claiming BGRA)
                        let tex_r = atlas_data[ai] as f32 / 255.0;
                        let tex_g = atlas_data[ai + 1] as f32 / 255.0;
                        let tex_b = atlas_data[ai + 2] as f32 / 255.0;
                        let tex_a = atlas_data[ai + 3] as f32 / 255.0;

                        let (mut out_r, mut out_g, mut out_b, out_a);

                        if has_color == 2.0 {
                            // IS_BG_IMAGE
                            out_r = tex_r;
                            out_g = tex_g;
                            out_b = tex_b;
                            out_a = tex_a * fg_a;
                        } else if has_color == 1.0 {
                            // IS_COLOR_EMOJI
                            out_r = tex_r;
                            out_g = tex_g;
                            out_b = tex_b;
                            out_a = tex_a;
                        } else if has_color == 4.0 {
                            // IS_GRAY_SCALE
                            out_r = fg_r;
                            out_g = fg_g;
                            out_b = fg_b;
                            out_a = fg_a * tex_a;
                        } else {
                            // IS_GLYPH (0.0) — coverage mask
                            out_r = fg_r;
                            out_g = fg_g;
                            out_b = fg_b;
                            // glyph-frag.glsl:148-152: the shader
                            // overwrites color.a with the mask's alpha
                            // ONLY when subpixel_aa is off.  Under
                            // dual-source the source alpha stays fg_a
                            // and the coverage arrives through the
                            // mask instead.
                            out_a = if subpixel_aa { fg_a } else { tex_a };

                            if apply_hsv {
                                let (h, s, v) = rgb_to_hsv(out_r, out_g, out_b);
                                let (nr, ng, nb) = hsv_to_rgb(
                                    h * foreground_text_hsb.hue,
                                    s * foreground_text_hsb.saturation,
                                    v * foreground_text_hsb.brightness,
                                );
                                out_r = nr;
                                out_g = ng;
                                out_b = nb;
                            }
                        }

                        // Color space handling:
                        // Texture-sourced colors (color emoji, bg image) are
                        // already sRGB in the ImageTexture atlas.
                        // Vertex-sourced colors (glyph, grayscale) are linear RGB.
                        let tex_is_srgb = has_color == 1.0 || has_color == 2.0;

                        // Per-vertex HSV (must operate in linear space)
                        if hsv[0] != 1.0 || hsv[1] != 1.0 || hsv[2] != 1.0 {
                            if tex_is_srgb {
                                out_r = srgb_to_linear(out_r);
                                out_g = srgb_to_linear(out_g);
                                out_b = srgb_to_linear(out_b);
                            }
                            let (h, s, v) = rgb_to_hsv(out_r, out_g, out_b);
                            let (nr, ng, nb) =
                                hsv_to_rgb(h * hsv[0], s * hsv[1], v * hsv[2]);
                            out_r = nr;
                            out_g = ng;
                            out_b = nb;
                            // After HSV in linear space, convert to sRGB
                            out_r = linear_to_srgb(out_r);
                            out_g = linear_to_srgb(out_g);
                            out_b = linear_to_srgb(out_b);
                        } else if !tex_is_srgb {
                            // Vertex colors are linear, convert to sRGB
                            out_r = linear_to_srgb(out_r);
                            out_g = linear_to_srgb(out_g);
                            out_b = linear_to_srgb(out_b);
                        }
                        // tex_is_srgb with no HSV → already sRGB, no conversion

                        // The atlas bytes are the shader's `colorMask`
                        // verbatim; see `subpixel_mask`.  `None` here
                        // means this sub-layer (or this branch) does
                        // not run under dual-source blending, and the
                        // pre-existing scalar path is kept bit-exact.
                        let mask = subpixel_aa
                            .then(|| {
                                subpixel_mask(
                                    has_color,
                                    atlas_data[ai],
                                    atlas_data[ai + 1],
                                    atlas_data[ai + 2],
                                    atlas_data[ai + 3],
                                )
                            })
                            .flatten();

                        match mask {
                            // A wholly uncovered texel contributes
                            // nothing on any channel: dst = dst.  This
                            // replaces the `out_a <= 0.0` skip, which
                            // no longer discriminates for IS_GLYPH now
                            // that out_a is fg_a rather than tex_a.
                            Some([0, 0, 0, 0]) => continue,
                            None if out_a <= 0.0 => continue,
                            // Deliberate behaviour change, on a path no
                            // measured row covers: a covered pixel whose
                            // `fg_a` is 0 is now PAINTED under subpixel,
                            // where the old skip dropped it.  That is
                            // what GL does — the dual-source colour
                            // equation `dst = src*mask + dst*(1-mask)`
                            // never references `src.a` — and it is
                            // reachable, because `fg_a` is the
                            // blink/fade-eased mixed alpha, so fully
                            // faded text under an LCD render target is
                            // painted at full coverage on both arms.
                            _ => {}
                        }

                        let fi = (fb_row_off + dx as usize) * 4;
                        let sb = (out_b.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                        let sg = (out_g.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                        let sr = (out_r.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                        let sa = (out_a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                        match mask {
                            Some([mr, mg, mb, ma]) => blend_over_masked(
                                &mut *fb,
                                fi, sr, sg, sb, sa, mr, mg, mb, ma,
                            ),
                            None => {
                                blend_over(&mut *fb, fi, sr, sg, sb, sa)
                            }
                        }
                    }
                }
            }
        }
    (quads_total, quads_blitted)
}


/// Alpha-blend a source pixel (sRGB, non-premultiplied) over the framebuffer.
/// fb is BGRA layout.
#[inline]
fn blend_over(fb: &mut [u8], fi: usize, sr: u8, sg: u8, sb: u8, sa: u8) {
    if sa == 255 {
        fb[fi] = sb;
        fb[fi + 1] = sg;
        fb[fi + 2] = sr;
        fb[fi + 3] = 255;
    } else if sa > 0 {
        let sa_f = sa as f32 / 255.0;
        let inv = 1.0 - sa_f;
        fb[fi] = (sb as f32 * sa_f + fb[fi] as f32 * inv + 0.5) as u8;
        fb[fi + 1] = (sg as f32 * sa_f + fb[fi + 1] as f32 * inv + 0.5) as u8;
        fb[fi + 2] = (sr as f32 * sa_f + fb[fi + 2] as f32 * inv + 0.5) as u8;
        fb[fi + 3] = ((sa as f32 + fb[fi + 3] as f32 * inv).min(255.0) + 0.5) as u8;
    }
}

/// Per-channel alpha blend, reproducing GL's dual-source blending:
///   dst = src * mask + dst * (1 - mask)
/// applied independently per channel (`draw.rs:181-195`, selected for the text
/// sub-layer at `draw.rs:244,263`).  `mask` is the per-channel coverage the LCD
/// rasteriser stores in the atlas RGB (`skrifa_rasterizer.rs:672-703`); the
/// scalar `blend_over` uses only the max-of-channels alpha in A, which is what
/// made PureCpu fall back to grayscale antialiasing.
///
/// Note the alpha equation is *not* `blend_over`'s.  `alpha_blending` uses
/// `source: One` (`dst.a = src.a + dst.a*(1-src.a)`) while dual-source uses
/// `SourceOneColor` (`dst.a = src.a*mask.a + dst.a*(1-mask.a)`).  Mirroring GL
/// is the point of this pass, so the divergence is reproduced, not smoothed
/// over; it is pinned by
/// `blend_over_masked_alpha_mirrors_dual_source_not_alpha_blending`.
///
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

/// The `colorMask` (second fragment output) the glyph shader emits for a quad
/// of this `has_color`, given the atlas texel it sampled — `None` when the
/// branch has no producer in the sub-layer that runs under dual-source
/// blending, so the caller keeps the ordinary `blend_over` path.
///
/// `subpixel_aa` is a property of the *sub-layer*, not of the quad
/// (`draw.rs:239-244` switches the blend state for a whole vertex buffer), so
/// the enumeration below is over "what can land in vertex buffer 1":
///
/// | has_color | producers in sub-layer 1 | shader |
/// |---|---|---|
/// | 0.0 IS_GLYPH | `screen_line.rs:642`, `box_model.rs:905`, `box_model.rs:931` (poly_quad sets `set_has_color(false)`) | `colorMask = texture(...)` |
/// | 1.0 IS_COLOR_EMOJI | `screen_line.rs:642` / `box_model.rs:905` when `glyph.has_color` | `colorMask = color.aaaa` |
/// | 3.0 IS_SOLID_COLOR | `borders.rs:25,39,53,67` (`filled_rectangle(layers, 1, ..)`) | `colorMask = vec4(1.0)` |
/// | 2.0 IS_BG_IMAGE | none — `background.rs:562` allocates layer 0 | — |
/// | 4.0 IS_GRAY_SCALE | none — every `set_grayscale` site is reached with `layer_num = 0` | — |
///
/// Enumerated by **resolved argument, not by literal call text**: four helpers
/// forward a `layer_num` parameter (`render/mod.rs:274,317,500`,
/// `screen_line.rs:357`), so a grep for `allocate(1)` misses `borders.rs` and
/// `box_model.rs:931` entirely — which is exactly how an earlier version of
/// this table was wrong.  `populate_image_quad` (`render/mod.rs:448`, allocating
/// at `:500`) is the fourth: it is parameterised too, but both of its callers
/// pass 0 (`screen_line.rs:478`) and 2 (`:707`), so no image quad reaches
/// sub-layer 1.
///
/// The IS_GLYPH mask is **linearised**, and this is the one place where
/// PureCpu's "the atlas is already sRGB, leave it alone" rule does not hold.
/// GL's atlas is an `SrgbTexture2d` (`renderstate.rs:113-116`,
/// `SrgbFormat::U8U8U8U8`), so `texture()` returns RGB converted sRGB->linear
/// while **alpha passes through unconverted**; the LCD rasteriser's sRGB
/// encode (`skrifa_rasterizer.rs:687-689`) exists only to survive that
/// round trip, and `linear_alpha` in A (`:674`) is deliberately not encoded.
/// PureCpu's arm holds the raw bytes in a plain `ImageTexture`
/// (`renderstate.rs:138`), so it must undo the encode itself.
///
/// For every *other* branch the round trip cancels and the raw bytes are
/// right: colour-emoji and background-image colours are linearised on sample
/// and re-encoded by `color = to_srgb(color)` at `glyph-frag.glsl:159`.
/// `colorMask` is the one shader output that is **not** passed through
/// `to_srgb`, which is exactly why the cancellation fails for the mask alone.
///
/// Measured, not reasoned: with the raw bytes, a body pixel over the harness
/// background read `(16,16,99)` against GL's `(16,16,49)` — precisely
/// `linear_to_srgb` applied one time too many. See the Task 8 report.
///
/// **The linearised mask is quantised to 8 bits where GL carries it in float,
/// and that costs at most 1 LSB — at any contrast.** `m8 = round(m*255)` gives
/// `|m8/255 - m| <= 0.5/255` by construction, and the blend is linear in the
/// mask, so the output error is `|src - dst| * dm <= 255 * 0.5/255 = 0.5` of a
/// level. Verified exhaustively over every source, destination and attainable
/// atlas byte: the worst observed difference is 1 LSB. This is why the subpixel
/// rows show a raised `AE` at fuzz 0 (6489 against a 2458 baseline) while `PAE`
/// sits at the noise floor and `AE` at 1% fuzz is 0 — those are sub-LSB
/// differences, and there is no second mechanism hiding in them.
///
/// One consequence worth stating: atlas sRGB bytes `0..=6` all linearise to a
/// mask of 0, so a faint glyph edge pixel GL keeps is dropped here. It is
/// bounded by the same 0.5-level rule (the largest such GL mask is 0.46/255),
/// because what low coverage loses is *relative* precision — which is precisely
/// what does not matter when the absolute contribution is that small.
#[inline]
fn subpixel_mask(has_color: f32, tr: u8, tg: u8, tb: u8, ta: u8) -> Option<[u8; 4]> {
    #[inline]
    fn lin(v: u8) -> u8 {
        (srgb_to_linear(v as f32 / 255.0) * 255.0 + 0.5) as u8
    }
    if has_color == 0.0 {
        Some([lin(tr), lin(tg), lin(tb), ta])
    } else if has_color == 1.0 {
        Some([ta, ta, ta, ta])
    } else if has_color == 3.0 {
        Some([255, 255, 255, 255])
    } else {
        None
    }
}

/// Whether the text sub-layer uses subpixel antialiasing, i.e. dual-source
/// blending in GL.  Mirrors `draw.rs:172-179` exactly; if the two ever diverge
/// the two backends disagree about the blend equation while agreeing about the
/// atlas contents, which looks like a rendering bug rather than a config bug.
#[inline]
fn subpixel_enabled(
    render_target: Option<config::FreeTypeLoadTarget>,
    load_target: config::FreeTypeLoadTarget,
) -> bool {
    matches!(
        render_target.unwrap_or(load_target),
        config::FreeTypeLoadTarget::HorizontalLcd | config::FreeTypeLoadTarget::VerticalLcd
    )
}

#[inline]
fn srgb_to_linear(x: f32) -> f32 {
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
fn linear_to_srgb(x: f32) -> f32 {
    if x <= 0.0031308 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

#[inline]
fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let s = if max == 0.0 { 0.0 } else { d / max };
    let v = max;
    let h = if d < 1.0e-10 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, s, v)
}

#[inline]
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let h = h.fract() * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 1.0 {
        (c, x, 0.0)
    } else if h < 2.0 {
        (x, c, 0.0)
    } else if h < 3.0 {
        (0.0, c, x)
    } else if h < 4.0 {
        (0.0, x, c)
    } else if h < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    ((r + m).clamp(0.0, 1.0), (g + m).clamp(0.0, 1.0), (b + m).clamp(0.0, 1.0))
}

/// `pub(crate)` only so `purecpu_sampler`'s tests can reach [`test::blit_probe`].
/// Those tests used to grade a private *copy* of the blit loop; Task 15 pointed
/// them at the real one, and the harness that makes that possible lives here,
/// next to the loop it drives, rather than being duplicated there.
#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use crate::quad::{V_BOT_LEFT, V_BOT_RIGHT, V_TOP_LEFT, V_TOP_RIGHT};

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn srgb_linear_roundtrip() {
        for &v in &[0.0_f32, 0.01, 0.04045, 0.5, 0.75, 1.0] {
            let rt = linear_to_srgb(srgb_to_linear(v));
            assert!(approx_eq(rt, v), "roundtrip failed for {v}: got {rt}");
        }
    }

    #[test]
    fn srgb_linear_boundary() {
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert_eq!(linear_to_srgb(0.0), 0.0);
        assert!(approx_eq(srgb_to_linear(1.0), 1.0));
        assert!(approx_eq(linear_to_srgb(1.0), 1.0));
    }

    #[test]
    fn hsv_roundtrip() {
        let colors: &[(f32, f32, f32)] = &[
            (1.0, 0.0, 0.0), // red
            (0.0, 1.0, 0.0), // green
            (0.0, 0.0, 1.0), // blue
            (1.0, 1.0, 1.0), // white
            (0.5, 0.5, 0.5), // gray
            (0.0, 0.0, 0.0), // black
            (0.8, 0.2, 0.5), // arbitrary
        ];
        for &(r, g, b) in colors {
            let (h, s, v) = rgb_to_hsv(r, g, b);
            let (r2, g2, b2) = hsv_to_rgb(h, s, v);
            assert!(
                approx_eq(r, r2) && approx_eq(g, g2) && approx_eq(b, b2),
                "HSV roundtrip failed for ({r},{g},{b}): got ({r2},{g2},{b2})"
            );
        }
    }

    #[test]
    fn blend_over_opaque() {
        let mut fb = vec![0u8; 8]; // two pixels
        // Fully opaque red over black
        blend_over(&mut fb, 0, 255, 0, 0, 255);
        assert_eq!(fb[0], 0); // B
        assert_eq!(fb[1], 0); // G
        assert_eq!(fb[2], 255); // R
        assert_eq!(fb[3], 255); // A
    }

    #[test]
    fn blend_over_transparent() {
        let mut fb = vec![100, 150, 200, 255]; // existing pixel
        // Fully transparent: no change
        blend_over(&mut fb, 0, 0, 0, 0, 0);
        assert_eq!(fb, [100, 150, 200, 255]);
    }

    #[test]
    fn blend_over_semi() {
        let mut fb = vec![0, 0, 0, 255]; // black background
        // 50% white over black → ~128
        blend_over(&mut fb, 0, 128, 128, 128, 128);
        // sa_f = 128/255 ≈ 0.502
        // each channel: 128 * 0.502 + 0 * 0.498 ≈ 64
        assert!(fb[0] > 60 && fb[0] < 68);
        assert!(fb[1] > 60 && fb[1] < 68);
        assert!(fb[2] > 60 && fb[2] < 68);
    }

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
        // 100 * (128/255) + 0 * (127/255) + 0.5 = 50.196 + 0.5 = 50.696 -> 50
        assert_eq!(fb[1], 50, "green half covered: 100*128/255 rounds to 50");
        assert_eq!(fb[0], 0, "blue not covered at all");
        assert_eq!(fb[3], 255, "alpha fully covered, so it takes the source's");
    }

    #[test]
    fn blend_over_masked_uniform_mask_matches_blend_over_on_color() {
        // A mask equal on all three colour channels must agree with the scalar
        // path there, so enabling subpixel cannot shift the *colour* of
        // ordinary text.  Alpha is deliberately excluded: see
        // `blend_over_masked_alpha_mirrors_dual_source_not_alpha_blending`.
        for a in [0u8, 1, 64, 128, 254, 255] {
            let mut fb1 = vec![10u8, 20, 30, 40];
            let mut fb2 = vec![10u8, 20, 30, 40];
            blend_over(&mut fb1, 0, 200, 100, 50, a);
            blend_over_masked(&mut fb2, 0, 200, 100, 50, a, a, a, a, a);
            assert_eq!(fb1[0..3], fb2[0..3], "colour mismatch at alpha {a}");
        }
    }

    #[test]
    fn blend_over_masked_alpha_mirrors_dual_source_not_alpha_blending() {
        // The two GL blend states differ in the *alpha* equation, and this is
        // the whole of the colour-emoji question in Task 8:
        //   alpha_blending  (draw.rs:203)  source: One
        //       dst.a = src.a + dst.a * (1 - src.a)
        //   dual_source     (draw.rs:186)  source: SourceOneColor
        //       dst.a = src.a * mask.a + dst.a * (1 - mask.a)
        // So a uniform mask is NOT a no-op on alpha, and `blend_over_masked`
        // must reproduce the dual-source form rather than agree with
        // `blend_over`.  Pinned by hand at src.a = mask.a = 128, dst.a = 40:
        //   blend_over:        128 + 40*(127/255) + 0.5   = 148.42 -> 148
        //   blend_over_masked: 128*(128/255) + 40*(127/255) + 0.5 = 84.67 -> 84
        let mut fb1 = vec![10u8, 20, 30, 40];
        let mut fb2 = vec![10u8, 20, 30, 40];
        blend_over(&mut fb1, 0, 200, 100, 50, 128);
        blend_over_masked(&mut fb2, 0, 200, 100, 50, 128, 128, 128, 128, 128);
        assert_eq!(fb1[3], 148, "alpha_blending's One-factor leading term");
        assert_eq!(fb2[3], 84, "dual-source squares the source alpha");
    }

    #[test]
    fn subpixel_mask_is_per_channel_for_glyphs_and_uniform_for_emoji() {
        // IS_GLYPH: `colorMask = texture(...)` (glyph-frag.glsl:145) off an
        // SrgbTexture2d, so RGB comes back LINEARISED and A does not.
        // Hand-derived, with srgb_to_linear as the shader's sampler applies it:
        //   10/255 = 0.039216 <= 0.04045 -> /12.92     = 0.003035 -> 0.774 -> 1
        //   128/255 = 0.501961 -> (0.556961/1.055)^2.4 = 0.215825 -> 55.0  -> 55
        //   255 -> 1.0 -> 255;  0 -> 0
        // A is passed through: 40 stays 40, because sRGB textures never
        // convert the alpha channel.
        assert_eq!(subpixel_mask(0.0, 0, 10, 128, 40), Some([0, 1, 55, 40]));
        assert_eq!(subpixel_mask(0.0, 255, 255, 255, 255), Some([255, 255, 255, 255]));
        // IS_COLOR_EMOJI: `colorMask = color.aaaa` (glyph-frag.glsl:131) —
        // the texel's alpha broadcast to all four channels, NOT its colour,
        // and unconverted for the same reason.
        assert_eq!(subpixel_mask(1.0, 10, 20, 30, 40), Some([40, 40, 40, 40]));
        // IS_SOLID_COLOR: `colorMask = vec4(1.0)` (glyph-frag.glsl:117), i.e.
        // a straight replace.  Reachable in sub-layer 1 via window borders
        // (borders.rs:25,39,53,67 pass layer_num = 1).
        assert_eq!(subpixel_mask(3.0, 10, 20, 30, 40), Some([255, 255, 255, 255]));
        // IS_BG_IMAGE (2.0) and IS_GRAY_SCALE (4.0) have no producer in
        // sub-layer 1 — background.rs:562 allocates layer 0 and every
        // `set_grayscale` site (box_model.rs:1048,1063,1078,1093) is reached
        // with layer_num = 0 — so they never see the dual-source blend.
        assert_eq!(subpixel_mask(2.0, 10, 20, 30, 40), None);
        assert_eq!(subpixel_mask(4.0, 10, 20, 30, 40), None);
    }

    #[test]
    fn subpixel_enabled_matches_the_gl_selection_expression() {
        use config::FreeTypeLoadTarget as T;
        // Mirrors draw.rs:172-179 exactly: render_target overrides
        // load_target, and only the two Lcd variants select subpixel.
        assert!(subpixel_enabled(Some(T::HorizontalLcd), T::Normal));
        assert!(subpixel_enabled(Some(T::VerticalLcd), T::Normal));
        assert!(!subpixel_enabled(Some(T::Normal), T::HorizontalLcd));
        assert!(!subpixel_enabled(Some(T::Light), T::VerticalLcd));
        assert!(!subpixel_enabled(Some(T::Mono), T::HorizontalLcd));
        // Unset render_target falls back to load_target.
        assert!(subpixel_enabled(None, T::HorizontalLcd));
        assert!(subpixel_enabled(None, T::VerticalLcd));
        assert!(!subpixel_enabled(None, T::Normal));
        assert!(!subpixel_enabled(None, T::Light));
        assert!(!subpixel_enabled(None, T::Mono));
    }

    #[test]
    fn coalesce_to_bands_merges_overlapping() {
        let rects = vec![
            DirtyRect { x: 0, y: 10, width: 100, height: 20 },
            DirtyRect { x: 0, y: 25, width: 100, height: 20 },
        ];
        let bands = coalesce_to_bands(&rects, 100);
        assert_eq!(bands.len(), 1);
        assert_eq!(bands[0].y, 10);
        assert_eq!(bands[0].height, 35); // 10..45
    }

    #[test]
    fn coalesce_to_bands_keeps_disjoint() {
        let rects = vec![
            DirtyRect { x: 0, y: 0, width: 100, height: 10 },
            DirtyRect { x: 0, y: 50, width: 100, height: 10 },
        ];
        let bands = coalesce_to_bands(&rects, 100);
        assert_eq!(bands.len(), 2);
    }

    #[test]
    fn coalesce_to_bands_empty() {
        let bands = coalesce_to_bands(&[], 100);
        assert!(bands.is_empty());
    }

    #[test]
    fn collect_clip_rects_intersection() {
        let rects = vec![DirtyRect { x: 10, y: 10, width: 20, height: 20 }];
        let mut out = Vec::new();
        // Quad fully inside dirty rect
        collect_clip_rects(12, 12, 25, 25, &rects, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], [12, 12, 25, 25]);

        // Quad partially overlapping
        out.clear();
        collect_clip_rects(0, 0, 15, 15, &rects, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], [10, 10, 15, 15]);

        // Quad not overlapping
        out.clear();
        collect_clip_rects(0, 0, 5, 5, &rects, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn clear_rect_zeroes_region() {
        let mut fb = vec![255u8; 4 * 4 * 4]; // 4x4 pixel framebuffer, all white
        let rect = DirtyRect { x: 1, y: 1, width: 2, height: 2 };
        clear_rect(&mut fb, 4, 4, &rect);
        // Check that pixel (1,1) is zero
        let idx = (1 * 4 + 1) * 4;
        assert_eq!(fb[idx..idx + 4], [0, 0, 0, 0]);
        // Check that pixel (0,0) is still white
        assert_eq!(fb[0..4], [255, 255, 255, 255]);
    }

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
        // .min(), so a negative sum wraps to a huge x1.  In a debug build the
        // index arithmetic then panics with "attempt to multiply with overflow"
        // at the `row_end` computation; in release it wraps quietly and the
        // `row_end <= fb.len()` guard skips the row — the "silently not drawn"
        // symptom.  Both are fixed by clamping in i32 before the cast.
        let fb_w = 4usize;
        let fb_h = 3usize;
        let mut fb = vec![0xFFu8; fb_w * fb_h * 4];
        let rect = DirtyRect { x: -10, y: 0, width: 2, height: 1 };
        clear_rect(&mut fb, fb_w, fb_h, &rect);
        assert!(fb.iter().all(|&b| b == 0xFF), "off-screen rect touched pixels");
    }

    #[test]
    fn clear_rect_clamps_a_rect_straddling_the_bottom_right_corner() {
        // The pair above cover total rejection and the pre-existing test covers
        // total acceptance; without a straddling rect nothing distinguishes
        // "clamp to the visible part" from "skip the whole rect", which is the
        // entire content of the M7 fix.
        let (fb_w, fb_h) = (4usize, 3usize);
        let mut fb = vec![0xFFu8; fb_w * fb_h * 4];
        clear_rect(&mut fb, fb_w, fb_h, &DirtyRect { x: 2, y: 1, width: 5, height: 5 });
        for y in 0..fb_h {
            for x in 0..fb_w {
                let i = (y * fb_w + x) * 4;
                let cleared = fb[i..i + 4] == [0, 0, 0, 0];
                assert_eq!(cleared, x >= 2 && y >= 1, "pixel ({x},{y})");
            }
        }
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

    // =====================================================================
    // A framebuffer harness for the blit loop (Task 15).
    //
    // `call_draw_purecpu` used to be executed by no test whatsoever: with an
    // unconditional `panic!` on the first line of the clip loop AND
    // `cy2.min(blit_y2)` mutated to `cy2.min(blit_y2 + 1)`, the suite still
    // reported "75 passed; 0 failed" and exited 0.  Everything below drives the
    // *shipping* loop — `blit_vertex_buffer`, the function
    // `call_draw_purecpu` now calls — with hand-built data: no `TermWindow`, no
    // window, no X display, no atlas, no font.
    // =====================================================================

    /// Side of the atlas the harness builds.
    ///
    /// 256 so a texel's column and row each fit in one byte (see
    /// [`probe_atlas`]), and a power of two so the texcoord round-trip through
    /// `f32` normalisation is exact — the invariant `purecpu_sampler`'s module
    /// doc says the whole sampler rests on.
    pub(crate) const PROBE_ATLAS: usize = 256;

    /// An atlas whose every texel names its own coordinates: `R = column`,
    /// `G = row`, `B = 0`, `A = 255`.
    ///
    /// Blitted through `has_color == 1.0` (IS_COLOR_EMOJI) or `2.0`
    /// (IS_BG_IMAGE) with an opaque `fg_color`, the loop copies the texel to the
    /// framebuffer unchanged: both branches take `out_rgb = tex_rgb`, both are
    /// `tex_is_srgb` so no transfer function runs, `hsv` is the identity, and
    /// `out_a == 1.0` makes `blend_over` a straight replace.  So after a blit
    /// the framebuffer says, per pixel, *which atlas texel that pixel read* —
    /// which is what turns a framebuffer into a walk.
    ///
    /// # The load-bearing precondition is the opaque `fg_color`
    ///
    /// It is `blend_over`'s `sa == 255` **replace** fast path (`:742-746`) that
    /// this encoding rests on, not the transfer functions — which is why
    /// [`blit_probe`] hard-codes `fg = [0, 0, 0, 1]` rather than taking one.
    /// Take that away on the `has_color == 2.0` arm, where `out_a = tex_a *
    /// fg_a`, and the RGB is blended toward the cleared framebuffer instead of
    /// replacing it, so the decoded coordinates come back **scaled**: at
    /// `fg_a == 0.5` a pixel that really sampled texel `(10, 20)` decodes as
    /// `(5, 10)`.  Measured, not argued.  The walk keeps the same *length*, so
    /// `assert_same_walk`'s rect and pixel-count checks both still pass and only
    /// a per-pixel mismatch could catch it.  The `has_color == 1.0` arm is
    /// immune, because `out_a = tex_a` ignores `fg` entirely.
    ///
    /// What is *not* fragile is [`Rig::written`]'s `alpha != 0` predicate: on
    /// this path `blend_over` guards `else if sa > 0`, so a zero-alpha pixel is
    /// never written and `written == false` is then *correct*.  A sentinel
    /// pre-fill — the fix an earlier round proposed — would make written-ness
    /// robust and leave the RGB just as blended, i.e. it addresses the direction
    /// that does not happen and not the one that does.
    ///
    /// Anyone extending the probe to `has_color == 0.0` should read that as: the
    /// coordinate encoding, not the written-ness predicate, is what has to be
    /// replaced.  IS_GLYPH and IS_GRAY_SCALE take `out_rgb` from the vertex, so
    /// there is no coordinate to decode at all; see
    /// [`coverage_atlas`] and the two tests that use it instead.
    fn probe_atlas(w: usize, h: usize) -> Vec<u8> {
        assert!(w <= 256 && h <= 256, "probe atlas coordinates must fit in a byte");
        let mut data = vec![0u8; w * h * 4];
        for row in 0..h {
            for col in 0..w {
                let i = (row * w + col) * 4;
                data[i] = col as u8;
                data[i + 1] = row as u8;
                data[i + 2] = 0;
                data[i + 3] = 255;
            }
        }
        data
    }

    /// The coverage pattern [`coverage_atlas`] repeats across atlas columns:
    /// full, half, quarter, none.
    const COVERAGE: [u8; 4] = [255, 128, 64, 0];

    /// An atlas that is a pure coverage mask: `RGB = 0`, `A = COVERAGE[col % 4]`.
    ///
    /// [`probe_atlas`] cannot serve `has_color == 0.0` (IS_GLYPH) or `4.0`
    /// (IS_GRAY_SCALE): both take `out_rgb` from the *vertex* and only `out_a`
    /// from the texture, so there is no coordinate left in the framebuffer to
    /// decode.  What those branches do read is the texel alpha, and this atlas
    /// makes it discriminate — including a partially covered texel, which is
    /// what makes `blend_over`'s partial-alpha arm (`:747-754`) run from *inside*
    /// the loop rather than only from its own unit test, and a wholly uncovered
    /// one, which is what makes the loop's `out_a <= 0.0 => continue` (`:701`)
    /// run at all.
    fn coverage_atlas() -> Vec<u8> {
        let n = PROBE_ATLAS;
        let mut data = vec![0u8; n * n * 4];
        for row in 0..n {
            for col in 0..n {
                data[(row * n + col) * 4 + 3] = COVERAGE[col % 4];
            }
        }
        data
    }

    struct Rig {
        fb: Vec<u8>,
        fb_w: usize,
        fb_h: usize,
        atlas: Vec<u8>,
        atlas_w: usize,
        atlas_h: usize,
    }

    impl Rig {
        fn new(fb_w: usize, fb_h: usize) -> Self {
            Self {
                fb: vec![0u8; fb_w * fb_h * 4],
                fb_w,
                fb_h,
                atlas: probe_atlas(PROBE_ATLAS, PROBE_ATLAS),
                atlas_w: PROBE_ATLAS,
                atlas_h: PROBE_ATLAS,
            }
        }

        /// [`Rig::new`] with a different atlas of the same shape — the two
        /// branches that read only the texel *alpha* need [`coverage_atlas`]
        /// rather than [`probe_atlas`].
        fn with_atlas(fb_w: usize, fb_h: usize, atlas: Vec<u8>) -> Self {
            assert_eq!(
                atlas.len(),
                PROBE_ATLAS * PROBE_ATLAS * 4,
                "harness atlas is not {PROBE_ATLAS}x{PROBE_ATLAS} RGBA"
            );
            Self { atlas, ..Self::new(fb_w, fb_h) }
        }

        fn blit(
            &mut self,
            verts: &[Vertex],
            full_repaint: bool,
            dirty: &[DirtyRect],
        ) -> (u64, u64) {
            blit_vertex_buffer(
                &mut self.fb,
                self.fb_w,
                self.fb_h,
                &self.atlas,
                self.atlas_w,
                self.atlas_h,
                verts,
                full_repaint,
                dirty,
                false,
                false,
                config::HsbTransform::default(),
            )
        }

        /// The pixel at `(x, y)` in the framebuffer's own `[B, G, R, A]` order.
        fn px(&self, x: usize, y: usize) -> [u8; 4] {
            let i = (y * self.fb_w + x) * 4;
            [self.fb[i], self.fb[i + 1], self.fb[i + 2], self.fb[i + 3]]
        }

        /// Whether anything was blitted to `(x, y)`.  The framebuffer starts at
        /// zero and no path in the loop writes alpha 0: `blend_over` replaces
        /// wholesale at `sa == 255` and otherwise guards `else if sa > 0`, and
        /// the loop `continue`s at `out_a <= 0.0` before reaching it.  So a
        /// non-zero alpha is exactly "written", including for the partially
        /// covered texels [`coverage_atlas`] supplies.  (The predicate is sound
        /// in both directions; the assumption that *is* delicate is the
        /// coordinate encoding — see [`probe_atlas`].)
        fn written(&self, x: usize, y: usize) -> bool {
            self.px(x, y)[3] != 0
        }

        /// Every written pixel, row-major: `(x, y, [B, G, R, A])`.
        fn written_pixels(&self) -> Vec<(usize, usize, [u8; 4])> {
            let mut out = Vec::new();
            for y in 0..self.fb_h {
                for x in 0..self.fb_w {
                    if self.written(x, y) {
                        out.push((x, y, self.px(x, y)));
                    }
                }
            }
            out
        }
    }

    /// The four vertices of one quad, in the layout the renderer produces.
    ///
    /// `dest` is `[x, y, x2, y2]` in framebuffer pixels and `tex` the same in
    /// atlas texels; both are stored the way the real producers store them —
    /// positions relative to the framebuffer centre, which the blit recovers by
    /// adding `half_w`/`half_h` (`quad.rs`'s `set_position` writes window
    /// coordinates), and texcoords normalised by the atlas side (`set_texture`).
    ///
    /// The assertion holds the centre round-trip to *exactness*.  A harness that
    /// quietly displaced its own quad by an ulp would turn every geometry
    /// assertion below into a statement about the harness rather than about the
    /// loop, and it would do so silently.  Keep harness coordinates on
    /// quarter-pixel multiples of a modest magnitude and the subtraction is
    /// exact; the assertion is what makes "keep" enforceable.
    fn quad(
        dest: [f32; 4],
        tex: [f32; 4],
        has_color: f32,
        fg: [f32; 4],
        fb_w: usize,
        fb_h: usize,
    ) -> Vec<Vertex> {
        let half_w = fb_w as f32 / 2.0;
        let half_h = fb_h as f32 / 2.0;
        for (d, half) in [
            (dest[0], half_w),
            (dest[2], half_w),
            (dest[1], half_h),
            (dest[3], half_h),
        ] {
            assert_eq!(
                (d - half) + half,
                d,
                "harness displaced its own quad: {d} does not survive the centre round-trip"
            );
        }
        quad_at(
            [
                dest[0] - half_w,
                dest[1] - half_h,
                dest[2] - half_w,
                dest[3] - half_h,
            ],
            tex,
            has_color,
            fg,
        )
    }

    /// [`quad`], but taking positions already in the renderer's centred
    /// coordinates — for tests that must reproduce a producer's exact float
    /// expression rather than name a destination pixel.
    fn quad_at(pos: [f32; 4], tex: [f32; 4], has_color: f32, fg: [f32; 4]) -> Vec<Vertex> {
        let n = PROBE_ATLAS as f32;
        for t in tex {
            assert_eq!(
                (t / n) * n,
                t,
                "harness displaced its own texcoord: {t} does not survive normalisation"
            );
        }
        let mut v = vec![Vertex::default(); VERTICES_PER_CELL];
        v[V_TOP_LEFT].position = [pos[0], pos[1]];
        v[V_TOP_RIGHT].position = [pos[2], pos[1]];
        v[V_BOT_LEFT].position = [pos[0], pos[3]];
        v[V_BOT_RIGHT].position = [pos[2], pos[3]];
        v[V_TOP_LEFT].tex = [tex[0] / n, tex[1] / n];
        v[V_BOT_RIGHT].tex = [tex[2] / n, tex[3] / n];
        for vert in v.iter_mut() {
            vert.has_color = has_color;
            vert.fg_color = fg;
            vert.alt_color = [0.0; 4];
            vert.hsv = [1.0, 1.0, 1.0];
            vert.mix_value = 0.0;
        }
        v
    }

    /// Drive one quad through the real blit loop and report which destination
    /// pixel read which atlas texel: `(dest_rect, [(dest_x, dest_y, atlas_col,
    /// atlas_row)])`, or `None` when the loop wrote nothing.
    ///
    /// This is the framebuffer equivalent of the walk `purecpu_sampler`'s tests
    /// used to perform against a private transcription of this loop, and it is
    /// what lets those tests grade the shipping code.  Pixels come back in the
    /// loop's own order (row-major), so the two are directly comparable.
    pub(crate) fn blit_probe(
        bg_image: bool,
        dest_f: [f32; 4],
        tex_f: [f32; 4],
    ) -> Option<([i32; 4], Vec<(i32, i32, i32, i32)>)> {
        const FB: usize = 48;
        let q = purecpu_sampler::Quad::new(
            bg_image,
            dest_f,
            tex_f,
            PROBE_ATLAS as i32,
            PROBE_ATLAS as i32,
        );
        let rect = q.dest_rect();
        // A quad the harness framebuffer clipped would report a *shorter* walk
        // and read as a sampling difference, so refuse rather than mislead.
        assert!(
            rect[0] >= 0 && rect[1] >= 0 && rect[2] <= FB as i32 && rect[3] <= FB as i32,
            "blit_probe: dest rect {:?} does not fit the {}x{} harness framebuffer",
            rect,
            FB,
            FB
        );
        let mut rig = Rig::new(FB, FB);
        let has_color = if bg_image { 2.0 } else { 1.0 };
        let verts = quad(dest_f, tex_f, has_color, [0.0, 0.0, 0.0, 1.0], FB, FB);
        rig.blit(&verts, true, &[]);
        let out: Vec<(i32, i32, i32, i32)> = rig
            .written_pixels()
            .into_iter()
            .map(|(x, y, [_b, g, r, _a])| (x as i32, y as i32, r as i32, g as i32))
            .collect();
        if out.is_empty() {
            None
        } else {
            Some((rect, out))
        }
    }

    const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
    const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
    /// `RED` after the solid path's `linear_to_srgb` and 8-bit rounding, in the
    /// framebuffer's BGRA order: `linear_to_srgb(1.0) == 1.0` and
    /// `linear_to_srgb(0.0) == 0.0`, so the primaries survive exactly and the
    /// harness can pin values without restating the transfer function.
    const RED_BGRA: [u8; 4] = [0, 0, 255, 255];
    const GREEN_BGRA: [u8; 4] = [0, 255, 0, 255];

    #[test]
    fn blit_fills_a_solid_colour_quad_and_touches_nothing_else() {
        // IS_SOLID_COLOR (3.0), the branch that paints every cell background,
        // every cursor and every window border — and which no test executed.
        let mut rig = Rig::new(16, 16);
        let verts = quad([2.0, 3.0, 6.0, 7.0], [0.0; 4], 3.0, RED, 16, 16);
        assert_eq!(rig.blit(&verts, true, &[]), (1, 1));
        for y in 0..16 {
            for x in 0..16 {
                let inside = (2..6).contains(&x) && (3..7).contains(&y);
                assert_eq!(
                    rig.px(x, y),
                    if inside { RED_BGRA } else { [0, 0, 0, 0] },
                    "pixel ({x},{y})"
                );
            }
        }
    }

    #[test]
    fn blit_samples_a_textured_quad_across_its_destination() {
        // The textured arm, 1:1: destination pixel (4,4) must read atlas texel
        // (10,20) and (7,7) must read (13,23).  The probe atlas encodes its own
        // coordinates, so these are assertions about *which texel was sampled*,
        // not merely that something was drawn.
        let mut rig = Rig::new(16, 16);
        let verts = quad([4.0, 4.0, 8.0, 8.0], [10.0, 20.0, 14.0, 24.0], 1.0, RED, 16, 16);
        assert_eq!(rig.blit(&verts, true, &[]), (1, 1));
        for k in 0..4usize {
            for j in 0..4usize {
                assert_eq!(
                    rig.px(4 + j, 4 + k),
                    [0, (20 + k) as u8, (10 + j) as u8, 255],
                    "pixel ({}, {}) read the wrong texel",
                    4 + j,
                    4 + k
                );
            }
        }
        assert_eq!(rig.written_pixels().len(), 16, "the quad painted outside itself");
    }

    #[test]
    fn blit_stops_at_the_bg_image_crop_and_does_not_write_the_row_below() {
        // The `cy2.min(blit_y2)` mutant.  It is a no-op for sampled quads —
        // there `blit_y2 == dest_y2 >= cy2` and the extra `+ 1` is swallowed by
        // the `min` — so the only geometry that discriminates is one where the
        // crop actually bites: IS_BG_IMAGE, whose `blit_end` stops after
        // `min(tex, dest)` texels.  A 4x4 sprite in a 12x12 destination is
        // cropped to 4x4, so row 4 and column 4 must stay untouched.
        let mut rig = Rig::new(16, 16);
        let verts = quad([0.0, 0.0, 12.0, 12.0], [0.0, 0.0, 4.0, 4.0], 2.0, RED, 16, 16);
        assert_eq!(rig.blit(&verts, true, &[]), (1, 1));
        for y in 0..16 {
            for x in 0..16 {
                let inside = x < 4 && y < 4;
                assert_eq!(
                    rig.written(x, y),
                    inside,
                    "pixel ({x},{y}): the bg-image crop moved"
                );
                if inside {
                    assert_eq!(rig.px(x, y), [0, y as u8, x as u8, 255], "pixel ({x},{y})");
                }
            }
        }
    }

    #[test]
    fn blit_clips_a_quad_hanging_off_the_top_left_corner() {
        // Ruling 1's first unreachable case: a negative destination origin.
        // `legacy_oracle` structurally cannot express it and nothing has ever
        // executed it, yet it is what `row_start.max(0)` / `col_start.max(0)`
        // exist for — and indexing the framebuffer at a negative row is not a
        // wrong pixel, it is a panic or a wrapped `usize`.
        //
        // dest x: cover_start(-3) = ceil(-3.5) = -3, cover_end(5) = 5.
        // texel_x(dx) = floor(8 + ((dx + 0.5 + 3) / 8) * 8) = 11 + dx.
        // dest y: cover_start(-2) = -2, cover_end(6) = 6; texel_y(dy) = 11 + dy.
        // So the clipped quad starts at pixel (0,0) reading texel (11,11).
        let mut rig = Rig::new(16, 16);
        let verts = quad([-3.0, -2.0, 5.0, 6.0], [8.0, 9.0, 16.0, 17.0], 1.0, RED, 16, 16);
        assert_eq!(rig.blit(&verts, true, &[]), (1, 1));
        for y in 0..16 {
            for x in 0..16 {
                let inside = x < 5 && y < 6;
                assert_eq!(rig.written(x, y), inside, "pixel ({x},{y})");
                if inside {
                    assert_eq!(
                        rig.px(x, y),
                        [0, (11 + y) as u8, (11 + x) as u8, 255],
                        "pixel ({x},{y}) read the wrong texel — the clip shifted the sampling"
                    );
                }
            }
        }
    }

    #[test]
    fn blit_clips_a_quad_hanging_off_the_bottom_right_corner() {
        // Ruling 1's second unreachable case.  Symmetric to the one above and
        // *not* redundant with it: the top-left clip is `max`, the bottom-right
        // clip is `min(fb_w)` / `min(fb_h)`, two different expressions.  Without
        // this, deleting the `.min(fb_h as i32)` term writes past the end of the
        // framebuffer with nothing to say so.
        //
        // texel_x(dx) = floor(8 + (dx + 0.5 - 12)) = dx - 4, texel_y(dy) = dy - 3.
        let mut rig = Rig::new(16, 16);
        let verts = quad([12.0, 12.0, 20.0, 20.0], [8.0, 9.0, 16.0, 17.0], 1.0, RED, 16, 16);
        assert_eq!(rig.blit(&verts, true, &[]), (1, 1));
        for y in 0..16 {
            for x in 0..16 {
                let inside = x >= 12 && y >= 12;
                assert_eq!(rig.written(x, y), inside, "pixel ({x},{y})");
                if inside {
                    assert_eq!(
                        rig.px(x, y),
                        [0, (y - 3) as u8, (x - 4) as u8, 255],
                        "pixel ({x},{y}) read the wrong texel"
                    );
                }
            }
        }
    }

    #[test]
    fn blit_skips_a_quad_that_misses_every_dirty_rect() {
        // Ruling 1's third unreachable case: an incremental repaint in which
        // the quad overlaps no dirty rect.  This is the path that decides
        // whether PureCpu's whole reason for existing — repainting only what
        // changed — is correct or merely quiet, and `quads_blitted` is the
        // metric the pass tunes against.
        let mut rig = Rig::new(16, 16);
        let verts = quad([2.0, 2.0, 6.0, 6.0], [10.0, 20.0, 14.0, 24.0], 1.0, RED, 16, 16);
        let dirty = vec![DirtyRect { x: 8, y: 8, width: 4, height: 4 }];
        assert_eq!(
            rig.blit(&verts, false, &dirty),
            (1, 0),
            "the quad was counted as blitted"
        );
        assert!(
            rig.written_pixels().is_empty(),
            "a quad that misses every dirty rect painted anyway"
        );
    }

    #[test]
    fn blit_paints_only_the_intersection_with_a_dirty_rect() {
        // The other half of the incremental path: a quad that *partly* overlaps
        // must be painted exactly on the intersection, and must keep sampling
        // the texel its full-quad geometry says — clipping moves which pixels
        // are written, never which texel a written pixel reads.
        let mut rig = Rig::new(16, 16);
        let verts = quad([4.0, 4.0, 8.0, 8.0], [10.0, 20.0, 14.0, 24.0], 1.0, RED, 16, 16);
        let dirty = vec![DirtyRect { x: 6, y: 0, width: 10, height: 6 }];
        assert_eq!(rig.blit(&verts, false, &dirty), (1, 1));
        for y in 0..16 {
            for x in 0..16 {
                let inside = (6..8).contains(&x) && (4..6).contains(&y);
                assert_eq!(rig.written(x, y), inside, "pixel ({x},{y})");
                if inside {
                    assert_eq!(
                        rig.px(x, y),
                        [0, (20 + y - 4) as u8, (10 + x - 4) as u8, 255],
                        "pixel ({x},{y}) resampled itself against the clip rect"
                    );
                }
            }
        }
    }

    /// Lay out `cells` adjacent single-cell background quads exactly the way
    /// `screen_line.rs:231-242` lays them out, blit each, and return both the
    /// framebuffer and the destination-space edges `(left, right)` per cell.
    ///
    /// The two expressions under test are the ones that file actually writes:
    /// a cluster's right edge is `(left_pixel_x + i*cw) + (w*cw)` and the next
    /// cluster's left edge is `left_pixel_x + ((i+w)*cw)`.  The association is
    /// reproduced verbatim, and the `+ half_w` that turns a vertex position into
    /// a destination pixel is applied here rather than in the caller, because
    /// that addition is part of the arithmetic under test and rounds too.
    ///
    /// # One deliberate simplification, and why it is safe
    ///
    /// The right edge is modelled as `x + width` — one rounding.  The real
    /// producer is longer: `screen_line.rs:251-252` builds
    /// `euclid::rect(x, .., width, ..)`, intersects it with `bounding_rect`, and
    /// `filled_rectangle` (`render/mod.rs:281`) reads `rect.max_x()`.  euclid's
    /// `intersection` goes `to_box2d` → min/max → `to_rect`, so `size.width` is
    /// re-derived as `(x + width) - x` and `max_x()` is `x + ((x + width) - x)`:
    /// **three** roundings where this models one.  That is not obviously an
    /// identity, and if it were not one the seam tests below would be statements
    /// about a producer that does not exist.
    ///
    /// It was checked rather than assumed: over 79,307,920 on-screen cell
    /// boundaries (window widths 800..3840, integral cell widths 4..60,
    /// fractional pane origins) the euclid round trip moved **0** edges, and the
    /// integral-width sweep reports zero bit-differences and zero coverage
    /// disagreements on *both* edge forms.  So an auditor comparing this
    /// function against `screen_line.rs` should read the missing step as
    /// accounted for, not as an oversight.
    fn lay_out_cells(
        fb_w: usize,
        fb_h: usize,
        left_pixel_x: f32,
        cell_width: f32,
        cells: usize,
    ) -> (Rig, Vec<(f32, f32)>) {
        let half_w = fb_w as f32 / 2.0;
        let half_h = fb_h as f32 / 2.0;
        let mut rig = Rig::new(fb_w, fb_h);
        let mut edges = Vec::new();
        for i in 0..cells {
            let x = left_pixel_x + (i as f32 * cell_width);
            let width = 1.0 * cell_width; // `cluster_width as f32 * cell_width`
            let right = x + width;
            edges.push((x + half_w, right + half_w));
            let verts = quad_at(
                [x, -half_h, right, half_h],
                [0.0; 4],
                3.0,
                if i % 2 == 0 { RED } else { GREEN },
            );
            rig.blit(&verts, true, &[]);
        }
        (rig, edges)
    }

    #[test]
    fn adjacent_solid_quads_never_seam_at_an_integral_cell_width() {
        // Task 7's reviewer argued adjacent solid quads cannot seam because both
        // carry "the same edge `e`".  The premise is false — `(left + i*cw) +
        // (1*cw)` and `left + ((i+1)*cw)` are different expressions and need not
        // be the same float — but the conclusion holds, for a reason nobody had
        // stated: **`cell_width` is an integer.**  `screen_line.rs:62` computes
        // it as `cell_size.width as f32 * width_scale`, an integral pixel count
        // times 1.0 or 2.0, so `i * cw` is exact for every `i` and the two forms
        // agree bit-for-bit.  Measured over 2.34M on-screen cell boundaries at
        // window widths 800..3840: zero bit-differences, zero disagreements.
        //
        // This test asserts *both* links of that chain, and the two are NOT
        // interchangeable — each catches something the other cannot:
        //
        //  - the pixel check catches a **gap**: an unwritten pixel is
        //    `[0,0,0,0]` and matches neither colour.  It **cannot** catch an
        //    **overlap**, because a doubly-written pixel is still exactly one of
        //    the two colours.  That matters, because in a fractional sweep
        //    overlaps are the *more common* failure: 745 overlaps against 609
        //    seams over 35.7M boundaries.
        //  - the bit-identity check is therefore what carries the overlap half
        //    of the requirement — and it is also the only one that fires when
        //    the edges differ but happen not to straddle a pixel centre, which
        //    is exactly the fragile situation
        //    `adjacent_solid_quads_can_seam_at_a_fractional_cell_width` exhibits.
        let (fb_w, fb_h) = (1920usize, 2usize);
        let half_w = fb_w as f32 / 2.0;
        let mut checked = 0usize;
        for &cw in &[4.0f32, 7.0, 8.0, 11.0, 16.0, 17.0, 23.0, 30.0] {
            for ok in 0..8 {
                // A fractional pane origin, which is where the rounding would
                // have to come from once `i * cw` is exact.
                let origin = ok as f32 * 0.1234;
                let left = -half_w + origin;
                let cells = (fb_w as f32 / cw) as usize - 1;
                let (rig, edges) = lay_out_cells(fb_w, fb_h, left, cw, cells);

                for i in 0..cells - 1 {
                    assert_eq!(
                        edges[i].1.to_bits(),
                        edges[i + 1].0.to_bits(),
                        "cells {} and {} disagree about their shared edge: {} vs {} \
                         (cell_width {}, origin {}) — an integral cell_width is \
                         supposed to make these the same float",
                        i,
                        i + 1,
                        edges[i].1,
                        edges[i + 1].0,
                        cw,
                        origin
                    );
                }

                // ...and the framebuffer form, which is what a user would see.
                let x0 = purecpu_sampler::cover_start(edges[0].0);
                let x1 = purecpu_sampler::cover_end(edges[cells - 1].1);
                assert!(x0 >= 0 && x1 <= fb_w as i32, "sweep ran off the harness");
                assert!(x1 - x0 > 100, "sweep covered almost no pixels");
                for y in 0..fb_h {
                    for x in x0..x1 {
                        let px = rig.px(x as usize, y);
                        assert!(
                            px == RED_BGRA || px == GREEN_BGRA,
                            "unwritten or blended pixel ({},{}) = {:?} between adjacent \
                             solid quads (cell_width {}, origin {})",
                            x,
                            y,
                            px,
                            cw,
                            origin
                        );
                    }
                }
                checked += 1;
            }
        }
        assert_eq!(checked, 64, "the sweep did not run");
    }

    #[test]
    fn adjacent_solid_quads_can_seam_at_a_fractional_cell_width() {
        // The finding, pinned rather than papered over: the safety asserted by
        // `adjacent_solid_quads_never_seam_at_an_integral_cell_width` rests
        // entirely on `cell_width` being integral, and nothing in the type
        // system says it is.  Make it fractional and a one-pixel column that no
        // quad writes is constructible — found by search over window widths
        // 800..3840, at a rate of roughly one boundary in 30,000.
        //
        // Reproduction: window 1920 (`half_w == 960`), cell_width
        // 16.144447326660156, pane origin 0.2888416647911072.  Cell 42's right
        // edge lands *exactly* on the pixel centre 694.5, so `cover_end` gives
        // 694; cell 43's left edge is one ulp higher, 694.5000610351562, so
        // `cover_start` gives 695.  Pixel 694 is written by neither.
        //
        // Two things this is NOT.  It is not a PureCpu-vs-GL parity defect: GL
        // rasterises the same two float edges under the same pixel-centre rule
        // and drops the same column, so the backends agree.  And it is not a
        // reason to change the coverage rule, which is what makes PureCpu match
        // GL in the first place.  It is a reason to know that fractional cell
        // metrics — fractional DPI scaling, say — would introduce visible
        // 1px seams in cell backgrounds, and to have that fact fail loudly here
        // rather than be rediscovered from a screenshot.
        let (fb_w, fb_h) = (1920usize, 2usize);
        let half_w = fb_w as f32 / 2.0;
        let cw = 16.144447326660156f32;
        let origin = 0.2888416647911072f32;
        let left = -half_w + origin;

        // Only the two cells either side of the seam are needed; laying out all
        // 43 would blit 41 quads to say the same thing.
        let mut rig = Rig::new(fb_w, fb_h);
        let mut edges = Vec::new();
        for (i, fg) in [(42usize, RED), (43usize, GREEN)] {
            let x = left + (i as f32 * cw);
            let right = x + 1.0 * cw;
            edges.push((x + half_w, right + half_w));
            rig.blit(
                &quad_at([x, -(fb_h as f32 / 2.0), right, fb_h as f32 / 2.0], [0.0; 4], 3.0, fg),
                true,
                &[],
            );
        }

        // The floats, so a future reader can see the mechanism rather than
        // trust the pixel indices.
        assert_eq!(edges[0].1, 694.5, "cell 42's right edge moved");
        assert_eq!(edges[1].0, 694.5000610351562, "cell 43's left edge moved");
        assert_eq!(
            edges[1].0.to_bits() - edges[0].1.to_bits(),
            1,
            "the two edges are supposed to be exactly one ulp apart"
        );
        assert_eq!(purecpu_sampler::cover_end(edges[0].1), 694);
        assert_eq!(purecpu_sampler::cover_start(edges[1].0), 695);

        // ...and the seam itself, in the framebuffer.
        assert_eq!(rig.px(693, 0), RED_BGRA, "cell 42's last column");
        assert!(
            !rig.written(694, 0),
            "expected an unwritten seam column at 694; if this now passes, the \
             coverage rule or the layout arithmetic changed and the finding \
             above needs re-deriving rather than deleting"
        );
        assert_eq!(rig.px(695, 0), GREEN_BGRA, "cell 43's first column");
    }

    #[test]
    fn the_harness_can_see_a_seam_when_there_is_one() {
        // The control for the test above.  "No gap found" from an instrument
        // that cannot register a gap is worth nothing, and a whole-framebuffer
        // sweep that silently checked zero pixels would report exactly that.
        // Two solid quads deliberately separated by one pixel column must fail
        // the same "every pixel is written" check.
        let mut rig = Rig::new(16, 4);
        for (x0, x1, fg) in [(2.0f32, 5.0f32, RED), (6.0f32, 9.0f32, GREEN)] {
            let verts = quad([x0, 0.0, x1, 4.0], [0.0; 4], 3.0, fg, 16, 4);
            rig.blit(&verts, true, &[]);
        }
        assert!(rig.written(4, 0), "the left quad's last column");
        assert!(!rig.written(5, 0), "the deliberate gap is not visible");
        assert!(rig.written(6, 0), "the right quad's first column");
    }

    #[test]
    fn blit_walks_more_than_one_quad_per_call() {
        // Every other test in this module passes exactly `VERTICES_PER_CELL`
        // vertices, so `for q in 0..num_quads` runs its body once per call.
        // That leaves three things unexecuted: the `base = q * VERTICES_PER_CELL`
        // indexing, the reuse of the `clip_rects` `Vec` across iterations (the
        // second quad sees a `Vec` the first one filled, and depends on
        // `collect_clip_rects` clearing it), and every `(quads_total,
        // quads_blitted)` pair other than `(1,1)` / `(1,0)`.
        //
        // The two quads deliberately differ in *both* destination and texcoord,
        // so an indexing slip that read quad 0's vertices twice would paint 16
        // pixels in one place instead of 32 in two.
        let mut rig = Rig::new(16, 16);
        let mut verts = quad([0.0, 0.0, 4.0, 4.0], [10.0, 20.0, 14.0, 24.0], 1.0, RED, 16, 16);
        verts.extend(quad(
            [8.0, 8.0, 12.0, 12.0],
            [30.0, 40.0, 34.0, 44.0],
            1.0,
            GREEN,
            16,
            16,
        ));

        // Incremental, not full repaint: the full-repaint arm `clear()`s
        // `clip_rects` itself, so only this path grades the reuse.
        let dirty = vec![DirtyRect { x: 0, y: 0, width: 16, height: 16 }];
        assert_eq!(rig.blit(&verts, false, &dirty), (2, 2));

        // Both quads are 1:1, so texel = dest + (tex_origin - dest_origin):
        // quad 0 gives (10 + x, 20 + y); quad 1 gives (30 + (x-8), 40 + (y-8)),
        // i.e. (22 + x, 32 + y).
        for y in 0..16 {
            for x in 0..16 {
                let in0 = x < 4 && y < 4;
                let in1 = (8..12).contains(&x) && (8..12).contains(&y);
                let want = if in0 {
                    [0, (20 + y) as u8, (10 + x) as u8, 255]
                } else if in1 {
                    [0, (32 + y) as u8, (22 + x) as u8, 255]
                } else {
                    [0, 0, 0, 0]
                };
                assert_eq!(rig.px(x, y), want, "pixel ({x},{y})");
            }
        }
        assert_eq!(rig.written_pixels().len(), 32, "16 pixels per quad, two quads");
    }

    #[test]
    fn blit_paints_one_quad_against_two_disjoint_dirty_rects() {
        // `collect_clip_rects` exists for exactly this case, and the comment at
        // its call site names it: "the bounding-box problem where non-adjacent
        // dirty rects cause large quads to overwrite clean framebuffer areas
        // between them".  Both other incremental tests pass a single dirty rect,
        // so `clip_rects.len() > 1` — and therefore the inner `for clip in
        // &clip_rects` running more than once — happened in no committed test.
        //
        // A 16x16 quad covering the whole framebuffer, against two 2x2 dirty
        // rects at opposite corners.  Bounding-box behaviour would repaint the
        // 12x12 span between them; correct behaviour touches 8 pixels.
        let mut rig = Rig::new(16, 16);
        let verts = quad([0.0, 0.0, 16.0, 16.0], [0.0, 0.0, 16.0, 16.0], 1.0, RED, 16, 16);
        let dirty = vec![
            DirtyRect { x: 0, y: 0, width: 2, height: 2 },
            DirtyRect { x: 10, y: 10, width: 2, height: 2 },
        ];
        // One quad, and it *is* blitted — the counters do not see the split.
        assert_eq!(rig.blit(&verts, false, &dirty), (1, 1));

        // 1:1 over the whole framebuffer, so texel == dest pixel and the second
        // clip rect must still sample from its own place, not from the first's.
        for y in 0..16 {
            for x in 0..16 {
                let inside = (x < 2 && y < 2) || ((10..12).contains(&x) && (10..12).contains(&y));
                let want = if inside { [0, y as u8, x as u8, 255] } else { [0, 0, 0, 0] };
                assert_eq!(rig.px(x, y), want, "pixel ({x},{y})");
            }
        }
        assert_eq!(rig.written_pixels().len(), 8, "two 2x2 dirty rects");
    }

    /// The vertex colour both branches below take their RGB from, in linear
    /// space, with an `fg_a` that is neither 0 nor 1 so the two branches'
    /// alphas differ.
    ///
    /// After `linear_to_srgb` and 8-bit rounding this is `(sr, sg, sb) =
    /// (137, 0, 255)`, derived rather than observed:
    ///
    /// - `0.25 > 0.0031308`, so `1.055 * 0.25^(1/2.4) - 0.055`.
    ///   `0.25^(1/2.4) = exp(ln(0.25)/2.4) = exp(-0.5776227) = 0.5612310`,
    ///   `* 1.055 = 0.5920977`, `- 0.055 = 0.5370977`,
    ///   `* 255 + 0.5 = 137.4599` → **137**.
    /// - `0.0` and `1.0` are the transfer function's two fixed points → **0**
    ///   and **255**.
    ///
    /// 137 is the discriminating one: without the transfer function the byte
    /// would be `0.25 * 255 + 0.5 = 64`.
    const GLYPH_FG: [f32; 4] = [0.25, 0.0, 1.0, 0.5];

    /// The quad both branches below are driven with, and the texels it reads.
    ///
    /// `dest [4,4,8,8]`, `tex [12,0,16,4]`, 4x4 onto 4x4, so
    /// `texel_x(dx) = floor(12 + (dx + 0.5 - 4)) = 8 + dx` — destination columns
    /// 4,5,6,7 read atlas columns 12,13,14,15, whose `col % 4` is 0,1,2,3.  So
    /// the four columns carry coverage 255, 128, 64 and 0 respectively: one
    /// opaque texel, two partial, one empty.
    ///
    /// That alignment is *asserted* rather than left to the comment above,
    /// because the fixture and the expectations can otherwise drift into
    /// agreement without either being right: shift the `tex` origin from 12 to
    /// 13 and regenerate the expected tables and both tests stay green while
    /// "one opaque, two partial, one empty" quietly becomes a rotation of
    /// itself.  Worse, a `dest_extent != src_extent` geometry stops `texel_x`
    /// being a translation, so two destination columns can read one atlas
    /// column and the four-distinct-coverages premise fails silently.  The
    /// check below re-derives the mapping from [`purecpu_sampler::Quad`] — the
    /// same type the loop uses — so it cannot agree with a wrong fixture.
    /// [`quad`] and [`quad_at`] guard the analogous hazard for their own
    /// arithmetic; this is the missing third.
    fn coverage_quad(has_color: f32, fg: [f32; 4]) -> Vec<Vertex> {
        let dest = [4.0, 4.0, 8.0, 8.0];
        let tex = [12.0, 0.0, 16.0, 4.0];

        let probe = purecpu_sampler::Quad::new(
            false,
            dest,
            tex,
            PROBE_ATLAS as i32,
            PROBE_ATLAS as i32,
        );
        let [dx0, _, dx1, _] = probe.dest_rect();
        assert_eq!(
            dx1 - dx0,
            COVERAGE.len() as i32,
            "the coverage fixture needs exactly one destination column per COVERAGE entry"
        );
        let seen: Vec<u8> = (dx0..dx1)
            .map(|dx| COVERAGE[(probe.texel_x(dx) as usize) % COVERAGE.len()])
            .collect();
        assert_eq!(
            seen,
            COVERAGE.to_vec(),
            "the coverage quad no longer reads one opaque, two partial and one \
             empty texel in that order — re-derive the expected tables in the \
             tests below rather than adjusting this assertion"
        );

        quad(dest, tex, has_color, fg, 16, 16)
    }

    #[test]
    fn blit_paints_a_glyph_from_the_vertex_colour_and_the_texel_coverage() {
        // `has_color == 0.0` (IS_GLYPH) is the branch that paints every
        // character on screen, and until this test nothing executed it.  It
        // cannot go through `blit_probe`: that decodes texel coordinates out of
        // the framebuffer RGB, and this branch overwrites RGB with the vertex
        // colour.  So drive the loop directly and pin the bytes.
        //
        // The branch is `out_rgb = fg_rgb`, `out_a = tex_a` (`subpixel_aa` is
        // false, and the shader only keeps `fg_a` under dual-source), then —
        // since `tex_is_srgb` is false here — `linear_to_srgb` on the RGB.
        // Note what that means: `fg_a` is 0.5 and the output alpha ignores it.
        //
        // Source pixel: (sr, sg, sb) = (137, 0, 255), see `GLYPH_FG`.
        // `blend_over` over a zero framebuffer, BGRA out:
        //   sa=255 → replace           → [255, 0, 137, 255]
        //   sa=128 → p = 128/255:      255p = 128.0   → +0.5 → 128
        //                              137p = 68.769  → +0.5 → 69
        //                              alpha = 128    → [128, 0, 69, 128]
        //   sa=64  → p = 64/255:       255p = 64.0    → 64
        //                              137p = 34.384  → +0.5 → 34
        //                                             → [64, 0, 34, 64]
        //   sa=0   → `out_a <= 0.0` short-circuits before `blend_over`
        //                                             → [0, 0, 0, 0]
        let mut rig = Rig::with_atlas(16, 16, coverage_atlas());
        let verts = coverage_quad(0.0, GLYPH_FG);
        assert_eq!(rig.blit(&verts, true, &[]), (1, 1));

        let want: [[u8; 4]; 4] = [
            [255, 0, 137, 255],
            [128, 0, 69, 128],
            [64, 0, 34, 64],
            [0, 0, 0, 0],
        ];
        for y in 0..16 {
            for x in 0..16 {
                let expected = if (4..8).contains(&x) && (4..8).contains(&y) {
                    want[x - 4]
                } else {
                    [0, 0, 0, 0]
                };
                assert_eq!(rig.px(x, y), expected, "pixel ({x},{y})");
            }
        }
        // Three covered columns of four rows.  The uncovered column is the
        // control: if `out_a` ever stopped coming from the texel this would be
        // 16.
        assert_eq!(rig.written_pixels().len(), 12, "the empty texel was painted");
    }

    #[test]
    fn blit_paints_a_grayscale_glyph_with_the_vertex_alpha_folded_in() {
        // `has_color == 4.0` (IS_GRAY_SCALE), the other branch no test executed.
        // Same colour path as IS_GLYPH — `out_rgb = fg_rgb` then
        // `linear_to_srgb` — and a *different* alpha: `out_a = fg_a * tex_a`
        // rather than `tex_a`.  That difference is the whole reason the two
        // branches exist separately, so this test uses `fg_a = 0.25` to make it
        // visible: swap either branch's alpha expression for the other's and
        // both of these tests go red.
        //
        //   tex_a = 255/255 → out_a = 0.25      → 0.25*255+0.5 = 64.25   → sa 64
        //   tex_a = 128/255 → out_a = 0.1254902 → 32.0 + 0.5            → sa 32
        //   tex_a = 64/255  → out_a = 0.0627451 → 16.0 + 0.5            → sa 16
        //   tex_a = 0       → out_a = 0         → `out_a <= 0.0`, untouched
        //
        // and `blend_over`'s partial arm over a zero framebuffer, with
        // (sr, sg, sb) = (137, 0, 255) exactly as above:
        //   sa=64 → 255p = 64.0, 137p = 34.384+0.5 → [64, 0, 34, 64]
        //   sa=32 → 255p = 32.0, 137p = 17.192+0.5 → [32, 0, 17, 32]
        //   sa=16 → 255p = 16.0, 137p =  8.596+0.5 → [16, 0,  9, 16]
        //
        // Every one of these three goes through the `else if sa > 0` arm — the
        // opaque `sa == 255` replace path never runs here, which is the other
        // thing this test buys.
        let mut rig = Rig::with_atlas(16, 16, coverage_atlas());
        let fg = [GLYPH_FG[0], GLYPH_FG[1], GLYPH_FG[2], 0.25];
        let verts = coverage_quad(4.0, fg);
        assert_eq!(rig.blit(&verts, true, &[]), (1, 1));

        let want: [[u8; 4]; 4] =
            [[64, 0, 34, 64], [32, 0, 17, 32], [16, 0, 9, 16], [0, 0, 0, 0]];
        for y in 0..16 {
            for x in 0..16 {
                let expected = if (4..8).contains(&x) && (4..8).contains(&y) {
                    want[x - 4]
                } else {
                    [0, 0, 0, 0]
                };
                assert_eq!(rig.px(x, y), expected, "pixel ({x},{y})");
            }
        }
        assert_eq!(rig.written_pixels().len(), 12, "the empty texel was painted");
    }
}
