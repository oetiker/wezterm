#!/usr/bin/env bash
# corpus/scaled-glyph.sh — text that forces wezterm's glyph.scale != 1 path
# (wezterm-gui/src/glyphcache.rs, the "physically scaling" log::trace! at the
# scale != 1.0 branch around line 858).
#
# (Task 1) Answering spec §10's open question: does any font on this machine
# actually reach that path, or would a re-run against it be a null result
# indistinguishable from parity (the mistake that already produced one false
# `parity` verdict on inline images)?
#
# `fc-list` on this machine turns up exactly one candidate:
#   /usr/share/fonts/truetype/noto/NotoColorEmoji.ttf: Noto Color Emoji:style=Regular
# `fc-list :scalable=false family` is empty — there is no bitmap-only fallback
# font (e.g. unifont, terminus, misc-fixed) installed here.
#
# Confirmed at runtime, not assumed from the font's name/format: built
# WEZTERM_BIN=/scratch/oetiker/wezterm-builds/wezterm-gui-rebased, ran it with
# WEZTERM_LOG=trace against a corpus printing 😀🎉🚀🔥, and grepped the log.
# It fired for every glyph:
#
#   TRACE wezterm_gui::glyphcache > physically scaling GlyphInfo { only_char:
#   Some('😀'), ... } by 0.1588235294117647 bcos 136x128 > 9.6x21.12.
#   aspect=1.0625
#
# i.e. Noto Color Emoji's fixed 136x128 bitmap strike is scaled down to fit a
# 9.6x21.12px cell at this harness's 12pt font_size, so glyph.scale=0.159 !=
# 1.0. This required no source instrumentation — wezterm-gui/src/glyphcache.rs
# already logs this at trace level; WEZTERM_LOG=trace was enough. No temporary
# logging was added or needed.
#
# So: yes, a glyph.scale != 1 font exists on this machine. This corpus is safe
# to use for the scaled-glyph / bitmap-glyph matrix row.
set -euo pipefail
printf 'scaled glyphs (Noto Color Emoji, forces glyph.scale != 1):\n'
printf '\xf0\x9f\x98\x80 \xf0\x9f\x8e\x89 \xf0\x9f\x9a\x80 \xf0\x9f\x94\xa5\n'
exec sleep 600
