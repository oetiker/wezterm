//! The atlas sampler PureCpu never had.
//!
//! The rasteriser used to blit textured quads 1:1 and crop them
//! (`purecpu.rs:346`, findings I5), so anything drawn at a size other than its
//! atlas sprite's — non-native inline images, double-width and double-height
//! lines, scaled fallback and bitmap glyphs — was cropped instead of scaled.
//!
//! This module reproduces what the GPU's fixed function does for free:
//! `MagnifySamplerFilter::Nearest` + `MinifySamplerFilter::Nearest` +
//! `SamplerWrapFunction::Clamp` (`render/draw.rs:215-218`).
//!
//! **Scope.** `atlas_linear_sampler` is used in exactly one shader branch,
//! `o_has_color == 2.0` — the background image (`glyph-frag.glsl:119`).  PureCpu
//! *does* draw that branch (`purecpu.rs:440`, `IS_BG_IMAGE`), and those quads are
//! scaled by construction, so Nearest would be a deliberate approximation there.
//! Everywhere else — glyphs, colour emoji, grayscale quads — Nearest is not an
//! approximation of the GPU, it *is* the GPU, and parity is exact.
//!
//! Settled, do not re-decide: **background image is out of scope for this pass,
//! so the `has_color == 2.0` branch must not be routed through this sampler.**
//! Leave it on its current path.  Routing it would expand scope silently and
//! would manufacture parity diffs that look like sampler defects and are not.
//!
//! **An invariant this module silently depends on: the atlas side is a power of
//! two.**  Texcoords reach us as `coords.min_x() / width` (`to_texture_coords`,
//! `window/src/bitmaps/mod.rs:34-42`) and we multiply by the side again to get
//! texels.  That round-trip is exact only because both operations are by a power
//! of two — `ATLAS_SIZE = 128` (`termwindow/mod.rs:90`) growing by
//! `(side * 2).max(size.next_power_of_two())` (`window/src/bitmaps/atlas.rs:117-120`).
//! On a non-power-of-two side the round-trip would land at, say, `99.99997`, and
//! `floor` would read one texel into the sprite to the *left* — on the first
//! pixel of every sprite.  It would look like a font-rendering bug, not an atlas
//! one.  If the atlas sizing ever changes, this module needs a half-texel inset.
//!
//! One place exact parity is still expected to wobble: at exact minification
//! ratios (2:1, 3:1, …) every sample lands precisely *on* a texel boundary,
//! where `floor` takes the upper texel and the hardware's finite-precision
//! texcoord interpolation decides the tie.  A 1-texel diff there is interpolation
//! precision, not a sampler bug; triage it that way.
//!
//! Task 7 routes the blit loop through this module, via [`Quad`].

/// First destination pixel covered by a quad edge, under GL's rule that a pixel
/// is covered when its centre lies inside the primitive.
///
/// Pixel `x` is covered when `edge0 <= x + 0.5`, so the first covered pixel is
/// `ceil(edge - 0.5)`.  This replaces the truncating `as i32` that displaced
/// sub-pixel-positioned quads a whole pixel left/up (M1).
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
///
/// `src_limit` is the atlas dimension in texels; sampling clamps to
/// `[0, src_limit - 1]`, because `src_limit` itself is one texel outside it.
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
        // A zero-extent destination would divide by zero here.  Rust would not
        // panic — the quotient is +/-inf or NaN and `f32 as i32` saturates — but
        // relying on that leaves the answer to a language rule nobody wrote
        // down, so say it out loud: a quad zero pixels wide covers no pixel, so
        // no caller ever blits with this result.  Returning the clamped source
        // origin (10 for both cases in `degenerate_extents_do_not_panic`) keeps
        // the function total and in range whatever a caller does with it.
        if self.dest_extent == 0.0 {
            return self.src_origin.floor().clamp(0.0, (self.src_limit - 1).max(0) as f32) as i32;
        }
        let u = (dest_pixel as f32 + 0.5 - self.dest_origin) / self.dest_extent;
        let s = self.src_origin + u * self.src_extent;
        let idx = s.floor();
        // This branch is *value-equivalent to deleting it* — it is documentation,
        // not logic: `NaN as i32` already saturates to 0 and `0.clamp(0, hi)` is
        // 0.  It is here so the next reader does not have to know that language
        // rule to know what a NaN texcoord does.  Infinities deliberately do
        // *not* short-circuit: saturation takes them to `i32::MIN`/`i32::MAX` and
        // the clamp then yields the *near* edge texel, which is what
        // CLAMP_TO_EDGE means.  Returning 0 for `+inf` would be a wrap to the far
        // edge — precisely the bug the clamp exists to prevent.
        if idx.is_nan() {
            return 0;
        }
        (idx as i32).clamp(0, (self.src_limit - 1).max(0))
    }
}

/// One axis of a quad: the coverage rule and the source mapping, joined.
///
/// Until Task 7 these two halves had never been used together — `cover_start`/
/// `cover_end` decide *which* destination pixels a quad touches, `Axis::texel`
/// decides *what* each of them reads, and every test drove `texel` with a
/// hand-written pixel range.  Joining them at each call site would put an
/// off-by-one (`start..=end`) somewhere no unit test can see, so the join lives
/// here, once, and `covered_texels` tests it directly.
#[derive(Clone, Copy, Debug)]
pub struct Span {
    /// First destination pixel the quad covers.
    pub start: i32,
    /// One past the last: the range is half-open, `start..end`, never `..=`.
    pub end: i32,
    axis: Axis,
}

impl Span {
    pub fn new(
        src_origin: f32,
        src_extent: f32,
        dest_edge0: f32,
        dest_edge1: f32,
        src_limit: i32,
    ) -> Self {
        Self {
            start: cover_start(dest_edge0),
            end: cover_end(dest_edge1),
            // The Axis interpolates across the *true* float quad, not across
            // the rounded pixel range.  Feeding it `start`/`end - start` here
            // would reintroduce exactly the half-pixel displacement that M1 is
            // about, and it would do so invisibly: every 1:1 case still passes.
            axis: Axis::new(
                src_origin,
                src_extent,
                dest_edge0,
                dest_edge1 - dest_edge0,
                src_limit,
            ),
        }
    }

    #[inline]
    pub fn texel(&self, dest_pixel: i32) -> i32 {
        self.axis.texel(dest_pixel)
    }

    /// The texel each covered pixel reads — the joined path, in one call.
    #[cfg(test)]
    fn covered_texels(&self) -> Vec<i32> {
        (self.start..self.end).map(|p| self.texel(p)).collect()
    }
}

/// A textured quad's destination rect and per-pixel source mapping.
///
/// This is the whole of what Task 7 changed about the blit loop, hoisted out of
/// `purecpu.rs` so it can be tested without a framebuffer.
///
/// Two rules live here, not one.  Ordinary textured quads — glyphs, colour
/// emoji, grayscale, solid colour — get the coverage rule and nearest-neighbour
/// resampling.  `bg_image` (`has_color == 2.0`) instead keeps the pre-Task-7
/// arithmetic *exactly*: a truncated destination rect and a 1:1 atlas copy
/// cropped to `min(tex, dest)`.  Background image is out of scope for this pass
/// by the user's own scope decision (see the module doc), and "out of scope"
/// means leave it alone, not pretend it is not there.  `legacy_bg_image_quads_
/// are_untouched` holds that rule to an independent transcription of the old
/// code.
pub struct Quad {
    bg_image: bool,
    /// Destination edges in framebuffer pixels: `[x, y, x2, y2]`, unrounded.
    dest_f: [f32; 4],
    /// Atlas edges in texels: `[x, y, x2, y2]`, unrounded.  `x2 < x` for a
    /// mirrored background tile.
    tex_f: [f32; 4],
    x: Span,
    y: Span,
}

impl Quad {
    pub fn new(
        bg_image: bool,
        dest_f: [f32; 4],
        tex_f: [f32; 4],
        atlas_w: i32,
        atlas_h: i32,
    ) -> Self {
        let [fx, fy, fx2, fy2] = dest_f;
        let [tfx, tfy, tfx2, tfy2] = tex_f;
        Self {
            bg_image,
            dest_f,
            tex_f,
            x: Span::new(tfx, tfx2 - tfx, fx, fx2, atlas_w),
            y: Span::new(tfy, tfy2 - tfy, fy, fy2, atlas_h),
        }
    }

    /// The integer destination rect, `[x, y, x2, y2]`, half-open in both axes.
    pub fn dest_rect(&self) -> [i32; 4] {
        Self::dest_rect_of(self.bg_image, self.dest_f)
    }

    /// The same rect, without building a `Quad` to ask for it.
    ///
    /// The blit loop needs the destination rect *before* it knows whether the
    /// quad is worth drawing: on an incremental repaint most quads overlap no
    /// dirty rect and bail, and building the source mapping for them is work
    /// thrown away in the hot loop of the renderer whose entire reason to exist
    /// is not doing work.  So the rect rule lives here and [`Quad::dest_rect`]
    /// delegates to it — one copy, reachable from both.  A second copy inlined
    /// at the call site would be free to drift from the one the sampler samples
    /// against, and nothing would fail while it did.
    pub fn dest_rect_of(bg_image: bool, dest_f: [f32; 4]) -> [i32; 4] {
        let [fx, fy, fx2, fy2] = dest_f;
        if bg_image {
            [fx as i32, fy as i32, fx2 as i32, fy2 as i32]
        } else {
            // Identical by construction to `[x.start, y.start, x.end, y.end]`:
            // `Span::new` builds those from the same `cover_start`/`cover_end`
            // on the same edges.
            [
                cover_start(fx),
                cover_start(fy),
                cover_end(fx2),
                cover_end(fy2),
            ]
        }
    }

    /// One past the last destination pixel the blit writes, per axis.
    ///
    /// `None` means the quad writes nothing and the caller must skip it.  A
    /// sampled quad always covers its whole destination rect, so the `None` and
    /// the crop it comes from exist only for `bg_image` — including the
    /// mirrored-tile case, where the negative source extent makes `blit_w`
    /// negative and the tile is dropped, exactly as it is today.
    pub fn blit_end(&self) -> Option<(i32, i32)> {
        let [dest_x, dest_y, dest_x2, dest_y2] = self.dest_rect();
        if !self.bg_image {
            return Some((dest_x2, dest_y2));
        }
        let [tfx, tfy, tfx2, tfy2] = self.tex_f;
        let blit_w = (tfx2 as i32 - tfx as i32).min(dest_x2 - dest_x);
        let blit_h = (tfy2 as i32 - tfy as i32).min(dest_y2 - dest_y);
        if blit_w <= 0 || blit_h <= 0 {
            return None;
        }
        Some((dest_x + blit_w, dest_y + blit_h))
    }

    /// The atlas column a destination pixel reads.
    ///
    /// Sampled quads come back clamped into the atlas; `bg_image` does not,
    /// because the old code did not — its caller bounds-checks and skips.
    #[inline]
    pub fn texel_x(&self, dest_pixel: i32) -> i32 {
        if self.bg_image {
            self.tex_f[0] as i32 + (dest_pixel - self.dest_f[0] as i32)
        } else {
            self.x.texel(dest_pixel)
        }
    }

    /// The atlas row a destination pixel reads.  See [`Quad::texel_x`].
    #[inline]
    pub fn texel_y(&self, dest_pixel: i32) -> i32 {
        if self.bg_image {
            self.tex_f[1] as i32 + (dest_pixel - self.dest_f[1] as i32)
        } else {
            self.y.texel(dest_pixel)
        }
    }
}

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

        // Integral origins alone are too weak: the general formula's
        // `64 + k + 0.5` and a shortcut's `64 + k` floor alike, so the case above
        // cannot tell the formula from a special case, and it cannot see the
        // half-texel `+ 0.5` being dropped either.  A sub-pixel destination
        // origin — the fancy tab bar's case — separates them.
        // s = 64 + (100 + k + 0.5 - 100.25) = 64.25 + k  ->  floor  ->  64 + k.
        let b = Axis::new(64.0, 10.0, 100.25, 10.0, 4096);
        for k in 0..10 {
            assert_eq!(b.texel(100 + k), 64 + k, "1:1 fractional origin broken at k={k}");
        }

        // ...and a sub-pixel *source* origin, which is what separates the true
        // offset (computed in float, then floored) from one computed by
        // truncating each origin to an integer first.
        // s = 64.9 + (100 + k + 0.5 - 100.25) = 65.15 + k  ->  floor  ->  65 + k.
        let c = Axis::new(64.9, 10.0, 100.25, 10.0, 4096);
        for k in 0..10 {
            assert_eq!(c.texel(100 + k), 65 + k, "1:1 fractional source broken at k={k}");
        }
    }

    #[test]
    fn negative_src_extent_walks_the_source_backwards() {
        // `BackgroundRepeat::Mirror` swaps the texcoords (`background.rs:572-577`),
        // so a mirrored tile arrives with max_x as its origin and a negative
        // extent.  The formula already handles it; nothing used to check.
        // s = 110 - (x + 0.5) = 109.5 - x  ->  floor  ->  109 - x.
        let a = Axis::new(110.0, -10.0, 0.0, 10.0, 4096);
        let got: Vec<i32> = (0..10).map(|x| a.texel(x)).collect();
        assert_eq!(got, vec![109, 108, 107, 106, 105, 104, 103, 102, 101, 100]);
    }

    #[test]
    fn non_positive_src_limit_yields_texel_zero() {
        // `(src_limit - 1).max(0)` is what stops `clamp` panicking on inverted
        // bounds, and a zero-sized atlas is the only input that reaches it.
        // Every pixel must collapse onto texel 0, in range for an empty atlas.
        assert_eq!(Axis::new(1000.0, 4.0, 0.0, 4.0, 0).texel(3), 0);
        assert_eq!(Axis::new(-1000.0, 4.0, 0.0, 4.0, 0).texel(3), 0);
        assert_eq!(Axis::new(1000.0, 4.0, 0.0, 4.0, -5).texel(3), 0);
        // ...including down the degenerate-extent path, which clamps separately.
        assert_eq!(Axis::new(1000.0, 0.0, 0.0, 0.0, 0).texel(3), 0);
    }

    #[test]
    fn scaled_quad_at_a_fractional_destination_origin() {
        // Task 7's actual case, which no other test has both halves of: a quad
        // that is *both* scaled and placed at a sub-pixel origin (a background
        // tile at a scroll offset, the fancy tab bar).  2:1 minification shifted
        // a quarter of a destination pixel is half a texel, enough to move every
        // sample across a boundary:
        // s = 2 * (x + 0.5 - 0.25) = 2x + 0.5  ->  floor  ->  2x.
        let a = Axis::new(0.0, 10.0, 0.25, 5.0, 4096);
        let got: Vec<i32> = (0..5).map(|x| a.texel(x)).collect();
        assert_eq!(got, vec![0, 2, 4, 6, 8]);
        // The same axis at an integral origin lands on the odd texels, so the
        // fraction is doing the work here rather than riding along.
        let b = Axis::new(0.0, 10.0, 0.0, 5.0, 4096);
        assert_eq!(b.texel(0), 1);
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
    fn degenerate_extents_yield_the_clamped_source_origin() {
        // Not in the plan; added because `degenerate_extents_do_not_panic`
        // discriminates only panics: deleting the `dest_extent == 0.0` guard
        // leaves it green (the division then yields inf/NaN and `f32 as i32`
        // saturates to 4095 and 0 respectively).  This pins the value the guard
        // exists to produce, so the claim in `texel`'s comment is checked.
        assert_eq!(Axis::new(10.0, 0.0, 5.0, 0.0, 4096).texel(5), 10);
        assert_eq!(Axis::new(10.0, 4.0, 5.0, 0.0, 4096).texel(5), 10);
        // ...and it stays inside the atlas when the origin does not.
        assert_eq!(Axis::new(9000.0, 4.0, 5.0, 0.0, 4096).texel(5), 4095);
        assert_eq!(Axis::new(-3.0, 4.0, 5.0, 0.0, 4096).texel(5), 0);
    }

    /// Walks a quad through **`purecpu.rs`'s real blit loop** on a full repaint
    /// (clip rect == destination rect), yielding `(dest_x, dest_y, atlas_col,
    /// atlas_row)` per written pixel.
    ///
    /// Until Task 15 this was a second implementation of that loop, written in
    /// this file and free to drift from the one that ships — the precise hazard
    /// the equivalence tests below exist to prevent, reintroduced one layer
    /// down.  `blit_probe` drives the shipping `blit_vertex_buffer` against a
    /// framebuffer and an atlas whose texels name their own coordinates, so the
    /// tuples below are read back out of a real blit rather than predicted.
    ///
    /// `legacy_oracle` stays a deliberate transcription of the *old* code and
    /// delegates to nothing; that asymmetry is the point.
    ///
    /// Two consequences of grading the real loop, both wanted: the walk is now
    /// bounded by the harness framebuffer (`blit_probe` asserts the quad fits
    /// rather than silently truncating) and by the atlas bounds check the loop
    /// performs, and the atlas side is `purecpu::test::PROBE_ATLAS` rather
    /// than a nominal 4096 — every texcoord these tests use is far inside it,
    /// so no clamp behaviour changes.
    fn walk(
        bg_image: bool,
        dest_f: [f32; 4],
        tex_f: [f32; 4],
    ) -> Option<([i32; 4], Vec<(i32, i32, i32, i32)>)> {
        crate::termwindow::render::purecpu::test::blit_probe(bg_image, dest_f, tex_f)
    }

    /// The atlas side `walk` blits against, as an `i32` for `Quad::new`.
    const ATLAS: i32 = crate::termwindow::render::purecpu::test::PROBE_ATLAS as i32;

    /// The pre-Task-7 blit arithmetic, transcribed from `purecpu.rs` as it
    /// stood at b7936ac (`:303-307` rect, `:337-342` source, `:392-425` crop
    /// and 1:1 walk).
    ///
    /// Deliberately written in the *old code's* shape and not delegating to
    /// anything in this module, so it keeps saying what the old code said no
    /// matter what `Quad` grows into.
    fn legacy_oracle(
        dest_f: [f32; 4],
        tex_f: [f32; 4],
    ) -> Option<([i32; 4], Vec<(i32, i32, i32, i32)>)> {
        let [fx, fy, fx2, fy2] = dest_f;
        let [tfx, tfy, tfx2, tfy2] = tex_f;
        let dest_x = fx as i32;
        let dest_y = fy as i32;
        let dest_x2 = fx2 as i32;
        let dest_y2 = fy2 as i32;
        let dest_w = dest_x2 - dest_x;
        let dest_h = dest_y2 - dest_y;
        if dest_w <= 0 || dest_h <= 0 {
            return None;
        }
        let tex_px_x = tfx as i32;
        let tex_px_y = tfy as i32;
        let tex_w = tfx2 as i32 - tex_px_x;
        let tex_h = tfy2 as i32 - tex_px_y;
        let blit_w = tex_w.min(dest_w);
        let blit_h = tex_h.min(dest_h);
        if blit_w <= 0 || blit_h <= 0 {
            return None;
        }
        let mut out = Vec::new();
        for row in 0..blit_h {
            for col in 0..blit_w {
                out.push((dest_x + col, dest_y + row, tex_px_x + col, tex_px_y + row));
            }
        }
        Some(([dest_x, dest_y, dest_x2, dest_y2], out))
    }

    /// The five geometries the oracle is exercised over: `(name, dest, tex)`.
    /// Every shape the crop rule can meet — 1:1, sub-pixel, magnified,
    /// minified, mirrored.
    const BG_CASES: [(&str, [f32; 4], [f32; 4]); 5] = [
        ("integral 1:1", [10.0, 20.0, 30.0, 40.0], [100.0, 200.0, 120.0, 220.0]),
        ("sub-pixel origin", [10.6, 20.6, 30.6, 40.6], [100.0, 200.0, 120.0, 220.0]),
        ("magnified 1:4", [0.0, 0.0, 40.0, 40.0], [0.0, 0.0, 10.0, 10.0]),
        ("minified 4:1", [0.0, 0.0, 5.0, 5.0], [0.0, 0.0, 20.0, 20.0]),
        ("mirrored tile", [0.0, 0.0, 10.0, 10.0], [110.0, 0.0, 100.0, 10.0]),
    ];

    /// Compares two walks and fails on the *first* differing pixel.
    ///
    /// A bare `assert_eq!` on these two vectors prints 400 four-tuples per side —
    /// 12 KB of terminal in a project whose hard constraint is that nothing may
    /// emit raw bytes to the terminal.  Same teeth, readable message: this still
    /// dies on a rect change, on a pixel-count change, and on a single moved
    /// texel, which are the three ways the legacy path can drift.
    #[track_caller]
    fn assert_same_walk(
        got: &Option<([i32; 4], Vec<(i32, i32, i32, i32)>)>,
        want: &Option<([i32; 4], Vec<(i32, i32, i32, i32)>)>,
        what: &str,
    ) {
        match (got, want) {
            (Some((got_rect, got_px)), Some((want_rect, want_px))) => {
                assert_eq!(got_rect, want_rect, "rect moved for {what}");
                assert_eq!(got_px.len(), want_px.len(), "pixel count moved for {what}");
                if let Some((i, (g, w))) = got_px
                    .iter()
                    .zip(want_px.iter())
                    .enumerate()
                    .find(|(_, (g, w))| g != w)
                {
                    panic!(
                        "sampling moved for {what}: pixel {i} of {} reads {g:?}, legacy reads {w:?} \
                         (tuples are dest_x, dest_y, atlas_col, atlas_row)",
                        got_px.len()
                    );
                }
            }
            _ => assert_eq!(
                got.is_some(),
                want.is_some(),
                "the drop-or-draw decision moved for {what}"
            ),
        }
    }

    #[test]
    fn fractional_texcoords_are_exercised_at_all() {
        // Every case in BG_CASES uses whole-number atlas coordinates, so nothing
        // in this module ever drove a fractional source origin — and fractional
        // is the interesting case, because it is where the power-of-two atlas
        // invariant in the module doc lives.
        //
        // A 1:4 magnification from a source origin of 100.4:
        //   s(p) = 100.4 + ((p + 0.5) / 40) * 10 = 100.4 + (p + 0.5) / 4
        // The fractional origin makes the run lengths *uneven* — 2 then 4 — which
        // is exactly what a whole-number origin cannot produce and what a
        // half-texel inset (were one ever wrongly added) would flatten.
        let dest_f = [0.0, 0.0, 40.0, 40.0];
        let tex_f = [100.4, 0.0, 110.4, 10.0];
        let sampled = Quad::new(false, dest_f, tex_f, 4096, 4096);
        let cols: Vec<i32> = (0..10).map(|dx| sampled.texel_x(dx)).collect();
        assert_eq!(cols, vec![100, 100, 101, 101, 101, 101, 102, 102, 102, 102]);

        // ...and the bg-image path truncates the source origin exactly as the old
        // code did, so a fractional texcoord must not move it.
        assert_same_walk(
            &walk(true, dest_f, tex_f),
            &legacy_oracle(dest_f, tex_f),
            "fractional texcoords",
        );
    }

    #[test]
    fn span_joins_coverage_to_sampling() {
        // THE seam test.  `cover_start`/`cover_end` and `Axis::texel` had never
        // been used together: every other test in this module drives `texel`
        // with a hand-written pixel range, so an off-by-one in the join
        // (`start..=end`) or an Axis built from the *rounded* range rather than
        // the float quad is invisible to all of them.  This drives the joined
        // path: float edges in, texels out.

        // 2:1 minification at a quarter-pixel destination origin.  Minification
        // is where a fractional origin has leverage: at 2:1 a quarter of a
        // destination pixel is half a texel, enough to move every sample across
        // a boundary.  (At 2:1 *magnification* it is an eighth of a texel and
        // every sample floors identically — a vacuous test.)
        //   start = ceil(100.25 - 0.5) = 100,  end = ceil(105.25 - 0.5) = 105
        //   s(p)  = 0 + ((p + 0.5 - 100.25) / 5) * 10 = 2p - 199.5
        //   p = 100..104  ->  0.5, 2.5, 4.5, 6.5, 8.5  ->  floor  ->  0,2,4,6,8
        let a = Span::new(0.0, 10.0, 100.25, 105.25, 4096);
        assert_eq!((a.start, a.end), (100, 105));
        assert_eq!(a.covered_texels(), vec![0, 2, 4, 6, 8]);

        // The M1 case: an edge whose fraction is past the pixel centre, so the
        // coverage rule and truncation disagree about the first pixel.
        //   start = ceil(99.7 - 0.5) = 100, and 99.7 as i32 = 99.
        //   s(p)  = ((p + 0.5 - 99.7) / 5) * 10 = 2p - 198.4
        //   p = 100..104  ->  1.6, 3.6, 5.6, 7.6, 9.6  ->  floor  ->  1,3,5,7,9
        let b = Span::new(0.0, 10.0, 99.7, 104.7, 4096);
        assert_eq!((b.start, b.end), (100, 105));
        assert_ne!(b.start, 99.7_f32 as i32, "truncation would start a pixel left");
        assert_eq!(b.covered_texels(), vec![1, 3, 5, 7, 9]);

        // The other direction, so the seam is pinned under magnification too.
        //   start = ceil(10.0) = 10,  end = ceil(20.0) = 20
        //   s(p)  = ((p + 0.5 - 10.5) / 10) * 5 = (p - 10) / 2
        let c = Span::new(0.0, 5.0, 10.5, 20.5, 4096);
        assert_eq!((c.start, c.end), (10, 20));
        assert_eq!(c.covered_texels(), vec![0, 0, 1, 1, 2, 2, 3, 3, 4, 4]);

        // A 1:1 span still walks the source one texel per pixel, start to end:
        // this is the property the four bit-identical parity rows rest on, now
        // asserted through the join rather than through `texel` alone.
        let d = Span::new(64.0, 10.0, 100.0, 110.0, 4096);
        assert_eq!((d.start, d.end), (100, 110));
        assert_eq!(d.covered_texels(), (64..74).collect::<Vec<i32>>());
    }

    #[test]
    fn dest_rect_of_is_the_rect_a_built_quad_reports() {
        // The blit loop calls `dest_rect_of` to decide whether a quad is worth
        // building, and `Quad`'s own `dest_rect` to decide where to write.  If
        // those two ever disagreed, an incremental repaint would test one rect
        // against the dirty list and paint another — so pin them to each other,
        // on edges where the two branches of the rule give different answers.
        //
        // Sub-pixel edges, chosen so truncation and the coverage rule disagree
        // on y1 (4.7 truncates to 4, covers from 5) but agree on x1:
        //   sampled: ceil(2.3-0.5)=2, ceil(4.7-0.5)=5, ceil(6.1-0.5)=6, ceil(9.2-0.5)=9
        //   bg_image: 2, 4, 6, 9   (plain truncation)
        let dest_f = [2.3, 4.7, 6.1, 9.2];
        let tex_f = [10.0, 20.0, 14.0, 24.0];

        let sampled = Quad::new(false, dest_f, tex_f, 4096, 4096);
        assert_eq!(sampled.dest_rect(), [2, 5, 6, 9]);
        assert_eq!(Quad::dest_rect_of(false, dest_f), sampled.dest_rect());

        let bg = Quad::new(true, dest_f, tex_f, 4096, 4096);
        assert_eq!(bg.dest_rect(), [2, 4, 6, 9]);
        assert_eq!(Quad::dest_rect_of(true, dest_f), bg.dest_rect());

        // …and the two branches really are distinguishable on this input, so a
        // `dest_rect_of` that ignored `bg_image` could not pass both above.
        assert_ne!(
            Quad::dest_rect_of(false, dest_f),
            Quad::dest_rect_of(true, dest_f)
        );
    }

    #[test]
    fn legacy_bg_image_quads_are_untouched() {
        // `has_color == 2.0` (IS_BG_IMAGE) is out of scope for this pass, which
        // means byte-identical output, not "close enough": GL samples that
        // branch with a *linear* sampler, so routing it through a nearest one
        // would both expand scope and manufacture parity diffs that look like
        // sampler defects.  Asserting "I did not touch it" is worth nothing;
        // this holds it to an independent transcription of the old code.
        for (name, dest_f, tex_f) in BG_CASES {
            assert_same_walk(
                &walk(true, dest_f, tex_f),
                &legacy_oracle(dest_f, tex_f),
                name,
            );
        }

        // ...and the converse error — leaving an ordinary textured quad on the
        // crop path — would make these agree.  Every case but the 1:1 one must
        // now differ, because that is exactly what I5 and M1 are.
        for (name, dest_f, tex_f) in BG_CASES {
            if name == "integral 1:1" {
                // The identity property: a 1:1 quad at integral coordinates
                // must still select the very texels the old blit did, so this
                // case is *expected* to agree and cannot carry the assertion.
                assert_same_walk(
                    &walk(false, dest_f, tex_f),
                    &legacy_oracle(dest_f, tex_f),
                    "the 1:1 identity",
                );
                continue;
            }
            assert_ne!(
                walk(false, dest_f, tex_f),
                legacy_oracle(dest_f, tex_f),
                "{name} is still being cropped rather than scaled"
            );
        }

        // Pinned values, so the pair above cannot both be wrong the same way.
        // Sub-pixel origin: truncation gives 10, the coverage rule ceil(10.1)
        // gives 11 — the whole-pixel displacement M1 names.
        let (dest_f, tex_f) = (BG_CASES[1].1, BG_CASES[1].2);
        assert_eq!(Quad::new(true, dest_f, tex_f, ATLAS, ATLAS).dest_rect(), [10, 20, 30, 40]);
        assert_eq!(Quad::new(false, dest_f, tex_f, ATLAS, ATLAS).dest_rect(), [11, 21, 31, 41]);
        // Magnified 1:4: same rect either way, but the crop stops after 10 of
        // the 40 destination pixels while the sampler covers all 40.
        let (dest_f, tex_f) = (BG_CASES[2].1, BG_CASES[2].2);
        assert_eq!(Quad::new(true, dest_f, tex_f, ATLAS, ATLAS).blit_end(), Some((10, 10)));
        assert_eq!(Quad::new(false, dest_f, tex_f, ATLAS, ATLAS).blit_end(), Some((40, 40)));
    }

    #[test]
    fn mirrored_tiles_are_still_dropped_because_they_are_background_images() {
        // `BackgroundRepeat::Mirror` swaps the texcoords
        // (`background.rs:572-577`), so a mirrored tile arrives with a negative
        // source extent.  Today `blit_w <= 0` drops such a quad entirely; the
        // sampler would draw it, walking the source backwards.  That is a real,
        // user-visible behaviour change — and it is NOT reachable from this
        // task, because those two swaps are immediately followed by
        // `set_is_background_image()` (`background.rs:580`), the only producer
        // of `has_color == 2.0` in the tree, and this task keeps that branch on
        // the crop path.  So the mirrored tile stays dropped:
        let (_, dest_f, tex_f) = BG_CASES[4];
        assert_eq!(Quad::new(true, dest_f, tex_f, 4096, 4096).blit_end(), None);

        // The change is nonetheless real, and this pins what it would do if a
        // later task routes background images through the sampler:
        //   s(p) = 110 + ((p + 0.5 - 0) / 10) * (100 - 110) = 109.5 - p
        let sampled = Quad::new(false, dest_f, tex_f, 4096, 4096);
        assert_eq!(sampled.blit_end(), Some((10, 10)));
        let cols: Vec<i32> = (0..10).map(|x| sampled.texel_x(x)).collect();
        assert_eq!(cols, vec![109, 108, 107, 106, 105, 104, 103, 102, 101, 100]);
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
