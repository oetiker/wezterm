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
//!
//! Nothing calls this yet: Task 7 routes the blit loop through it.  The
//! `dead_code` allows below are for that gap and come off with the wiring.

/// First destination pixel covered by a quad edge, under GL's rule that a pixel
/// is covered when its centre lies inside the primitive.
///
/// Pixel `x` is covered when `edge0 <= x + 0.5`, so the first covered pixel is
/// `ceil(edge - 0.5)`.  This replaces the truncating `as i32` that displaced
/// sub-pixel-positioned quads a whole pixel left/up (M1).
#[allow(dead_code)]
#[inline]
pub fn cover_start(edge: f32) -> i32 {
    (edge - 0.5).ceil() as i32
}

/// First destination pixel *past* the quad, same rule.
///
/// Delegates rather than repeating the expression: the two names exist because
/// the call sites mean different things, but there is only one coverage rule
/// and a second copy of it would be free to drift.
#[allow(dead_code)]
#[inline]
pub fn cover_end(edge: f32) -> i32 {
    cover_start(edge)
}

/// Maps destination pixels to atlas texels along one axis.
///
/// `src_limit` is the atlas dimension in texels; sampling clamps to
/// `[0, src_limit - 1]`, because `src_limit` itself is one texel outside it.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Axis {
    src_origin: f32,
    src_extent: f32,
    dest_origin: f32,
    dest_extent: f32,
    src_limit: i32,
}

#[allow(dead_code)]
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
        // NaN has no nearest edge, and `NaN as i32` is 0 by saturation anyway;
        // pick 0 explicitly.  Infinities deliberately fall through: saturation
        // takes them to `i32::MIN`/`i32::MAX` and the clamp then yields the near
        // or far edge texel, which is exactly what Clamp means.
        if idx.is_nan() {
            return 0;
        }
        (idx as i32).clamp(0, (self.src_limit - 1).max(0))
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
