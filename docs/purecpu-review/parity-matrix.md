# PureCpu Parity Matrix

Backend under test: `front_end = "PureCpu"`
Reference: `front_end = "OpenGL"` (Mesa llvmpipe 4.5)

Verdict vocabulary: `parity`, `degraded`, `missing`, `known gap`.
Method: `by-reading`, `needs-measurement`, `out-of-scope`.

Rows marked `needs-measurement` have their behaviour columns pre-filled from
static reading where that reading is suggestive but not conclusive; later tasks
overwrite Verdict/Evidence with measured results.

| Feature | GPU behaviour | PureCpu behaviour | Method | Verdict | Evidence |
|---|---|---|---|---|---|
| Text glyphs (monochrome) | Coverage mask sampled from the one atlas with a nearest sampler, tinted by `fg_color` mixed with `alt_color` by `mix_value`, `foreground_text_hsb` applied, output converted to sRGB (`glyph-frag.glsl:145-160`) | Same algorithm reimplemented per-pixel; `has_color==0.0` branch tints `fg` by atlas alpha and applies `foreground_text_hsb`, then linear→sRGB (`purecpu.rs:412-429`, `455-460`) | needs-measurement | | |
| Inline image: sixel | Image is decoded into the same glyph atlas; each covered cell gets a quad whose texture rect is the cell's fraction of the sprite and whose destination is exactly one cell — the GPU rescales source to destination (`render/mod.rs:468-518`) | Textured quads are blitted 1:1: `blit_w = tex_w.min(dest_w)`, `blit_h = tex_h.min(dest_h)`, source stepped in lockstep with destination. No scaling exists in the rasteriser, so whenever the image's per-cell source region is not exactly `cell_width x cell_height` the image is cropped (or leaves gaps), never resampled (`purecpu.rs:344-360`) | needs-measurement | degraded | Regenerate native-size sub-case: `FUZZ=1 ./compare-case.sh images-native "$PWD/corpus/images-native.sh"` — `out/images-native-gl.png` vs `out/images-native-cpu.png`, region crop `64x64+0+55`: AE=0, PAE=0 (bit-identical parity at native size, reproduced in fix round 2). Two larger sizes, both measured through `compare-case.sh` (not a side probe), both fail the plan's `PAE<=257` gate: **(1)** 300x300 sixel, still requested at native size (`FUZZ=1 ./compare-case.sh images "$PWD/corpus/images.sh"`, region crop `300x300+0+341`): AE=3300, **PAE=1542 (6 LSB, fails gate)** — not a crop, a one-row displacement of the gradient band boundary (row 135: `gl srgb(140,0,114)` vs `cpu srgb(142,0,112)`, i.e. GL steps to the next band one scan row earlier than PureCpu; absent at 64x64, so it is source-stepping rounding that only shows up once the image spans enough rows for a rounding remainder to accumulate). **(2)** `FUZZ=1 ./compare-case.sh wide-sixel "$PWD/corpus/wide-sixel.sh"` (17000x64 gradient, `PARITY_SETTLE_WARMUP=20` — see fix-round-2 report for why): whole-frame AE=64932, body PAE=13107 (51 LSB, fails gate); the visible 1000x64 strip alone (region crop `1000x64+0+55`) is **AE=64000 of 64000 pixels — every pixel in the strip differs** — by a uniform ~2-3/255 colour offset (`gl srgb(122,0,132)` vs `cpu srgb(124,0,130)` sampled across the whole strip width), consistent with GL sampling a `paint.rs:72-76` `AllowImage::Scale(2)`-downscaled sprite (confirmed via GL's own log: `Not enough texture space (... max 16384); will retry render with Scale(2)`; PureCpu's log has no such line, since `renderstate.rs:78-118`'s `Self::PureCpu` arm has no texture-size cap and never enters this fallback) while PureCpu blits the full-resolution source. So sixel is degraded from at least two independent, unrelated mechanisms once an image departs from a small native size: row-stepping rounding (visible already at 300x300) and the `AllowImage::Scale` asymmetry (visible once the atlas must grow past `GL_MAX_TEXTURE_SIZE`). See Task 4 fix-round-2 report |
| Inline image: iTerm2 OSC 1337 | Same path as sixel (`populate_image_quad`, `render/mod.rs:441-521`); `AllowImage::Scale(n)` may store a downscaled sprite in the atlas, widening the source/destination ratio | Same 1:1 crop as sixel; a downscaled sprite makes the mismatch larger | needs-measurement | degraded | Regenerate native-size sub-case: `FUZZ=1 ./compare-case.sh images-native "$PWD/corpus/images-native.sh"` — `out/images-native-gl.png` vs `out/images-native-cpu.png`, region crop `64x64+0+143`: AE=0, PAE=0 (bit-identical) — parity only holds when the requested display size equals the sprite's native size. Requested at non-native size (200x132px, and again at 20x4 cells): `FUZZ=1 ./compare-case.sh images "$PWD/corpus/images.sh"`, region crop `200x200+0+77` (merged view of both non-native blocks after terminal scroll): AE=39360, PAE=61423 (0.937 — near-maximal). Visual: OpenGL draws a solid stretched gradient block; PureCpu draws a dotted grid of tiny cropped fragments, one small top-left tile per covered cell, background showing through the rest of every cell — exactly the `purecpu.rs:346-347` crop-instead-of-scale defect at full severity, reachable from an ordinary escape sequence. (The "cpu ink stddev is ~59% of gl's" claim in fix-round-1 did not survive splitting the merged region into its two sub-blocks and should not be cited as supporting evidence — the AE/PAE numbers and the visual pattern carry the verdict.) See Task 4 fix-round-1 and fix-round-2 reports |
| Inline image: animated GIF | Frame advance happens inside `cached_image`, i.e. as a side effect of running the paint pass; the GPU path repaints on the scheduled animation timer so frames advance (`glyphcache.rs:929-970`, `render/mod.rs:468-473`) | `do_paint_purecpu` returns before `paint_impl` whenever no terminal row is dirty and no cursor-blink transition occurred, so the paint pass — and therefore frame advance — does not run for a still screen (`termwindow/mod.rs:1436-1447`). Additionally the same 1:1 crop applies to each frame | needs-measurement | degraded | The corpus GIF loops forever (`identify -verbose` reports `Iterations: 0`; `glyphcache.rs:955-960` wraps frame index unconditionally, so loop metadata doesn't matter — it animates for as long as paints happen). Measured with `sample-case.sh`, which samples a live region repeatedly instead of waiting to settle: `./sample-case.sh gifanim "$PWD/corpus/gif-loop.sh" "4x4+14+69" 12 0.5`. Over 12 samples at 0.5s intervals (6s): `gl` cycles `srgb(255,255,0)`/`srgb(0,128,0)` (yellow/green, both GIF frames observed); `cpu` reports `srgb(0,128,0)` for all 12 samples — frozen on one frame for the entire window. Confirms the `termwindow/mod.rs:1436-1447` idle-skip prediction directly: PureCpu does not advance GIF frames on an otherwise-idle screen. See Task 4 fix-round-1 report |
| Fancy tab bar | Elements rendered through `box_model`; glyph quads are sized `texture.coords.size * glyph.scale`, so a fallback/bitmap glyph with `scale != 1` is rescaled by the GPU (`box_model.rs:889-914`) | Solid-colour and 1:1 glyph quads are correct; any `glyph.scale != 1` quad is cropped rather than scaled (`purecpu.rs:346-347`) | needs-measurement | | |
| Retro tab bar | Rendered as ordinary cells plus `util_sprites.white_space` / `filled_box`, both allocated in the same atlas (`render/tab_bar.rs:49-50`, `utilsprites.rs:159-162`) | Same quads; solid-colour quads take the `IS_SOLID_COLOR` fill path, glyphs the 1:1 path (`purecpu.rs:300-341`) | needs-measurement | | |
| Window buttons | Built as `box_model` Elements (polys + glyphs) drawn from the same atlas (`render/window_buttons.rs`) | Same; poly sprites are generated at their final pixel size so the 1:1 blit is exact | needs-measurement | | |
| Rounded corners | `poly_quad` rasterises the corner poly into the atlas at exactly the requested corner size and marks it `IS_GRAY_SCALE` (`render/mod.rs:290-328`, `box_model.rs:1038-1092`) | `has_color==4.0` branch: `fg` tinted by atlas alpha; source and destination sizes are equal by construction, so the 1:1 blit is exact (`purecpu.rs:406-411`) | needs-measurement | | |
| Split dividers | `filled_rectangle` → `IS_SOLID_COLOR` quad, texture ignored by the shader (`render/split.rs:32,55`, `render/mod.rs:266-287`, `glyph-frag.glsl:115-118`) | `IS_SOLID_COLOR` fill path, colour-only, size-independent (`purecpu.rs:300-341`) | needs-measurement | | |
| Cursor (static) | Block/bar/underline drawn as a poly or solid quad at exact pixel size | Same quads; 1:1 blit is exact | needs-measurement | | |
| Cursor blink easing | `ColorEase::intensity_continuous()` is evaluated on the CPU every animation frame and delivered as `fg_color_mix` / `cursor_border_mix`, giving a continuous eased fade (`render/mod.rs:680-696`) | `do_paint_purecpu` quantises the eased intensity to a boolean (`i < 0.5`) and only repaints on a phase change or cycle end; between transitions the paint pass is skipped entirely (`termwindow/mod.rs:1380-1412`, `1436-1447`). The rasteriser itself handles `mix_value` correctly (`purecpu.rs:252-255`), so the loss is in the repaint scheduling, not the blend | by-reading | degraded | `termwindow/mod.rs:1387-1394` (quantisation to on/off), `termwindow/mod.rs:1440-1447` (paint skipped between transitions) |
| Blinking text attribute | `blink_state` / `rapid_blink_state` intensity is applied CPU-side to the cell foreground each paint, and `update_next_frame_time` schedules the next frame (`render/screen_line.rs:797-825`) | The colour computation is identical when a paint happens, but the dirty-rect pass only ever marks changed terminal rows and the cursor cell; nothing marks cells carrying the blink attribute, so the early exit at `termwindow/mod.rs:1440-1447` suppresses the repaint on an otherwise idle screen | needs-measurement | | |
| Visual bell | `get_intensity_if_bell_target_ringing` returns a continuously varying mix that is applied to the cell/cursor background each paint (`render/mod.rs:233-258`, `535-565`) | Same colour computation, but a ringing bell changes no line seqno and sets no dirty rect, so the same early exit applies (`termwindow/mod.rs:1436-1447`) | needs-measurement | | |
| Double-width / double-height lines (DECDWL/DECDHL) | Glyph destination is scaled by `width_scale` / `height_scale` while the atlas source stays at base size; the GPU rescales (`render/screen_line.rs:50-63`, `632-647`) | 1:1 blit clips to the smaller of source and destination, so the glyph is drawn at base size in the top-left of the enlarged cell (`purecpu.rs:346-347`) | by-reading | degraded | `render/screen_line.rs:636-647` vs `purecpu.rs:346-347` |
| Scaled fallback / bitmap glyphs (`glyph.scale != 1`) | Destination is `sprite size * glyph.scale`; the GPU rescales (`render/screen_line.rs:380-393`, `box_model.rs:902-913`). `scale` is set below 1 for oversized or unscaled bitmap glyphs such as colour-emoji fonts (`glyphcache.rs:749-798`) | 1:1 blit crops the sprite to the destination rectangle instead of downscaling | by-reading | degraded | `glyphcache.rs:749-798`, `box_model.rs:902-913` vs `purecpu.rs:346-347` |
| Subpixel antialiased text (`freetype_render_target = "HorizontalLcd"`) | Layer 1 is drawn with dual-source blending using the per-channel `colorMask` output (`render/draw.rs:172-193`, `244`, `260-266`; `glyph-frag.glsl:14-17`, `147-152`) | The rasteriser has a single scalar alpha and no per-channel mask; `blend_over` blends one alpha for all channels (`purecpu.rs:529-543`) | by-reading | degraded | `render/draw.rs:181-193` vs `purecpu.rs:529-543` |
| Window background image sampling filter | `IS_BG_IMAGE` quads are sampled through `atlas_linear_sampler` (bilinear) rather than the nearest sampler (`glyph-frag.glsl:119-127`, `render/draw.rs:220-223`) | `has_color==2.0` branch does the same 1:1 nearest crop as every other textured quad (`purecpu.rs:394-399`) | out-of-scope | known gap | user decision, not investigated |
| window_background_image | n/a | n/a | out-of-scope | known gap | user decision, not investigated |
| window_background_opacity | n/a | n/a | out-of-scope | known gap | user decision, not investigated |
| text_background_opacity | n/a | n/a | out-of-scope | known gap | user decision, not investigated |
| Background blur / HSB tint | n/a | n/a | out-of-scope | known gap | user decision, not investigated |

## Settled by reading: the single-texture question

The hypothesis was that `call_draw_purecpu` binds one texture and therefore
cannot draw content that the GPU path sources from some other texture.

**The hypothesis is false, and for a good reason: the GPU path has only one
texture too.** `call_draw_glium` binds exactly one `SrgbTexture2d` — the glyph
cache atlas (`render/draw.rs:157`) — and exposes it twice, as
`atlas_nearest_sampler` and `atlas_linear_sampler` (`render/draw.rs:215-223`),
two samplers over the same texture. Every `Sprite` in the GUI is allocated from
the single `GlyphCache::atlas` (`glyphcache.rs:562`): glyphs
(`glyphcache.rs:873`), custom block/poly glyphs (`glyphcache.rs:1340`), solid
colour swatches (`glyphcache.rs:1132`), inline images and animation frames
(`glyphcache.rs:922`, `978`, `1071`), and the util sprites
(`utilsprites.rs:159-162`). `GlyphCache::new_in_memory` exists but is only used
by the `ls-fonts` CLI and unit tests, never by a window.

So no content class is invisible under PureCpu because of texture binding. The
real structural gap is one level down, in sampling: the GPU interpolates texture
coordinates across each quad and therefore rescales whenever source and
destination sizes differ, while `call_draw_purecpu` blits 1:1 and clips
(`purecpu.rs:344-360`). Content whose quads rely on that rescale — inline
images, double-width/height lines, scaled fallback glyphs — is drawn from the
right texture but at the wrong size.

The second structural gap is not in the rasteriser at all but in
`do_paint_purecpu`'s idle skip (`termwindow/mod.rs:1436-1447`): time-driven
animation that changes no line seqno and marks no dirty rect never reaches the
paint pass.

## `Software` versus `PureCpu`

`FrontEndSelection::Software` is consumed in exactly one place:
`window/src/configuration.rs:11`, which makes `prefer_swrast()` true. That flag
only steers EGL/WGL configuration selection towards a software rasteriser
(`window/src/egl.rs:434-478`, `window/src/os/windows/wgl.rs:95`). Everything
above that — glium context, shaders, quad generation, `call_draw_glium` — is the
`OpenGL` front end unchanged. `Software` is therefore "OpenGL, but insist on
llvmpipe"; on this GPU-less machine it is behaviourally identical to `OpenGL`.

`PureCpu` skips OpenGL entirely: no GL context is created
(`termwindow/mod.rs:853-856`), the atlas is a plain `ImageTexture` in system
memory (`renderstate.rs:110-115`), and quads are rasterised by hand into a
`Vec<u8>` framebuffer that is pushed to the window with
`present_software_frame_region`. What PureCpu adds over `Software` is the
removal of the GL/Mesa stack and the ability to repaint only dirty regions; what
it gives up is the shader, the samplers, and per-frame animation repaints.
