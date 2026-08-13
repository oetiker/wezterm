# Wez's Terminal

<img height="128" alt="WezTerm Icon" src="https://raw.githubusercontent.com/wezterm/wezterm/main/assets/icon/wezterm-icon.svg" align="left"> *A GPU-accelerated cross-platform terminal emulator and multiplexer written by <a href="https://github.com/wez">@wez</a> and implemented in <a href="https://www.rust-lang.org/">Rust</a>*

User facing docs and guide at: https://wezterm.org/

![Screenshot](docs/screenshots/two.png)

*Screenshot of wezterm on macOS, running vim*

---

## About this fork

This is [@oetiker](https://github.com/oetiker)'s fork of wezterm. It exists to
run wezterm well on machines with **no usable GPU** — remote X11 sessions, VNC,
VMs, and thin clients — and to drop the vendored C font stack in favour of a
pure-Rust one.

**These changes are not proposed for upstream.** They are large, they remove
code other people depend on, and they trade breadth for one particular
deployment. Upstream wezterm is the right choice for almost everyone; this fork
is for the case where it is not.

Three substantial differences from upstream:

### 1. A pure-Rust font stack

The vendored C dependencies are **gone** — `deps/freetype`, `deps/harfbuzz`,
`deps/fontconfig` and `deps/cairo` were removed along with their wrappers, about
**270,000 lines** of vendored C and its build plumbing. In their place:

| role | upstream | this fork |
|---|---|---|
| rasterizer | FreeType | [skrifa](https://crates.io/crates/skrifa) (+ [zeno](https://crates.io/crates/zeno), [tiny-skia](https://crates.io/crates/tiny-skia) for COLR) |
| shaper | HarfBuzz | [harfrust](https://crates.io/crates/harfrust) |
| font discovery | FontConfig | [fontdb](https://crates.io/crates/fontdb) |

The build no longer needs a C toolchain or system font libraries. Hinting, COLR
colour fonts, synthetic bold/italic and subpixel positioning are all still
supported.

The `font_rasterizer`, `font_shaper` and `font_locator` config keys are still
accepted so old configs keep loading, but the removed backends no longer exist:
selecting `FreeType`, `Harfbuzz` or `FontConfig` logs a warning and falls
through to the Rust implementation.

### 2. `front_end = "PureCpu"` — a real software renderer

A new front end that rasterizes and blits on the CPU with **no OpenGL context at
all**. This is distinct from upstream's existing `Software` option, which is
still OpenGL — it just forces a software GL implementation such as llvmpipe:

| `front_end` | what it does |
|---|---|
| `OpenGL` | GPU via OpenGL (upstream default) |
| `WebGpu` | GPU via wgpu |
| `Software` | **OpenGL**, forced onto a software rasterizer (llvmpipe) |
| `PureCpu` | **no GL at all** — CPU rasterization, X11 `PutImage` presentation |

It is built to match the GPU output rather than approximate it: the same
nearest-neighbour atlas sampling the shader uses, per-channel subpixel
antialiasing matching the GL dual-source blend, and dirty-region tracking with an
idle skip so an idle window costs almost nothing.

Where it is measurably better: on a GPU-less box, **llvmpipe-backed OpenGL cost
30–120× the CPU of this renderer** in the same scenes (llvmpipe is
multi-threaded and exceeded 100% of a core; PureCpu stayed in the low single
digits). Numbers are from one machine and one X server — treat the ratio as the
result, not the percentages.

### 3. X11 fixes

- **OSC 52 clipboard** no longer silently does nothing when another window
  copied more recently ([PR #8043](https://github.com/wezterm/wezterm/pull/8043),
  offered upstream).
- **Large frames** are chunked so `PutImage` cannot exceed the X11 maximum
  request length.
- **The framebuffer is re-presented on `Expose`**, so switching virtual desktops
  no longer leaves a blank window.

## How the PureCpu renderer was verified

The CPU renderer was reviewed against the OpenGL backend pixel by pixel, and the
findings were then repaired and re-measured. That work is written up in
[`docs/purecpu-review/`](docs/purecpu-review/):

- [`README.md`](docs/purecpu-review/README.md) — what it does now, and what still differs
- [`parity-matrix.md`](docs/purecpu-review/parity-matrix.md) — every feature, its verdict, and the evidence
- [`findings.md`](docs/purecpu-review/findings.md) — each defect, its disposition, and the commit that fixed it
- [`noise-floor.md`](docs/purecpu-review/noise-floor.md) — the measurement calibration

The harness that produced those numbers is in
[`tools/purecpu-parity/`](tools/purecpu-parity/) and is re-runnable.

**Known gaps, stated plainly.** Background images, window opacity and
blur/HSB tint were never examined against the GPU path. PureCpu caps its glyph
atlas at 8192px where OpenGL allows 16384, so a very wide sixel is downscaled one
step further than on the GPU. Animated-image ink that overflows its cell is not
repainted incrementally. A static image costs ~0.6 ms per frame while any
animation holds the frame timer open.

> Subpixel antialiasing needs **two** config keys — the global
> `freetype_render_target` *and* the per-font `freetype_load_target` inside a
> `wezterm.font { ... }` table. Setting only the global one gives you dual-source
> blending over a grayscale atlas. See `tools/purecpu-parity/gen-config.sh` for a
> worked example.

---

## Installation

https://wezterm.org/installation

## Getting help

This is a spare time project, so please bear with me.  There are a couple of channels for support:

* You can use the [GitHub issue tracker](https://github.com/wezterm/wezterm/issues) to see if someone else has a similar issue, or to file a new one.
* Start or join a thread in our [GitHub Discussions](https://github.com/wezterm/wezterm/discussions); if you have general
  questions or want to chat with other wezterm users, you're welcome here!
* There is a [Matrix room via Element.io](https://matrix.to/#/#wezterm:matrix.org)
  for (potentially!) real time discussions.

The GitHub Discussions and Element/Gitter rooms are better suited for questions
than bug reports, but don't be afraid to use whichever you are most comfortable
using and we'll work it out.

## Supporting the Project

If you use and like WezTerm, please consider sponsoring it: your support helps
to cover the fees required to maintain the project and to validate the time
spent working on it!

[Read more about sponsoring](https://wezterm.org/sponsor.html).

* [![Sponsor WezTerm](https://img.shields.io/github/sponsors/wez?label=Sponsor%20WezTerm&logo=github&style=for-the-badge)](https://github.com/sponsors/wez)
* [Patreon](https://patreon.com/WezFurlong)
* [Ko-Fi](https://ko-fi.com/wezfurlong)
* [Liberapay](https://liberapay.com/wez)
