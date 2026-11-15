//! Glyph rasterization for `Frame::text` (change `text-rendering`, design D3/D4).
//!
//! The subset font is embedded with `include_bytes!`, so drawing text needs no
//! runtime file access (app-shell: "Bundled rendering font"). A `(char, px)`
//! mask cache makes repeated renders cheap; characters outside the subset fall
//! back to U+25A1 WHITE SQUARE and then to a drawn hollow box — never a panic.

use std::collections::HashMap;

use fontdue::{Font, FontSettings};

use crate::tokens::Color;

/// The subset font, compiled into the binary (design D2).
const FONT_BYTES: &[u8] = include_bytes!("../assets/harbor-font.otf");

/// Replacement glyph for characters outside the subset (design D4): U+25A1
/// WHITE SQUARE, which the subset carries. Noto Sans SC has no U+FFFD —
/// verified against the source cmap — so the square is the visible marker.
pub const REPLACEMENT: char = '\u{25a1}';

/// One rasterized glyph: alpha coverage plus placement relative to the pen.
#[derive(Clone, Debug)]
pub struct GlyphMask {
    pub width: usize,
    pub height: usize,
    /// Left edge offset from the pen (may be negative).
    pub xmin: i32,
    /// Bottom edge offset from the baseline; positive is above it (fontdue's
    /// y-up convention), so a descender glyph has a negative `ymin`.
    pub ymin: i32,
    pub advance: f32,
    /// `width * height` alpha samples, row-major, 0..=255.
    pub coverage: Vec<u8>,
}

impl GlyphMask {
    #[cfg(test)]
    fn any_coverage(&self) -> bool {
        self.coverage.iter().any(|&a| a > 0)
    }

    /// Last-resort glyph when even U+25A1 is missing: a hollow box standing
    /// at a nominal advance (design D4's final fallback).
    fn hollow_box(px: u16) -> Self {
        let side = (((px as f32) * 0.55) as usize).max(4);
        let border = 1.max(px as usize / 16);
        let mut coverage = vec![0u8; side * side];
        for y in 0..side {
            for x in 0..side {
                let edge =
                    x < border || x >= side - border || y < border || y >= side - border;
                if edge {
                    coverage[y * side + x] = 255;
                }
            }
        }
        Self {
            width: side,
            height: side,
            xmin: 0,
            ymin: (px as f32 * 0.15) as i32,
            advance: side as f32 + 1.0,
            coverage,
        }
    }
}

/// One rendered line: RGBA8 pixels (straight alpha), row-major, top-left
/// origin. Empty or fully transparent pixels are still a valid 1x1 buffer so
/// callers can upload it as a texture without special cases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedText {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl RenderedText {
    pub fn has_ink(&self) -> bool {
        self.pixels.chunks(4).any(|px| px[3] > 0)
    }
}

/// Owns the font and the `(char, px)` mask cache (design D3, level one).
pub struct Renderer {
    font: Font,
    masks: HashMap<(char, u16), GlyphMask>,
}

impl Renderer {
    /// Load the embedded font. `None` means the build artifact is corrupt:
    /// callers log once and fall back to placeholder bars — never panic
    /// (design D4).
    pub fn load() -> Option<Self> {
        Self::from_bytes(FONT_BYTES)
    }

    /// Parse a font from raw bytes (also the test seam for the corrupt-font
    /// path).
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let settings = FontSettings {
            collection_index: 0,
            scale: 40.0,
            load_substitutions: false,
        };
        Font::from_bytes(bytes, settings).ok().map(|font| Self {
            font,
            masks: HashMap::new(),
        })
    }

    /// The mask for `ch` at `px`, substituting [`REPLACEMENT`] when the subset
    /// does not cover `ch` and the hollow box when even that is missing.
    fn mask(&mut self, ch: char, px: u16) -> GlyphMask {
        let covered = self.font.lookup_glyph_index(ch) != 0;
        let key_ch = if covered { ch } else { REPLACEMENT };
        if let Some(hit) = self.masks.get(&(key_ch, px)) {
            return hit.clone();
        }
        let mask = if self.font.lookup_glyph_index(key_ch) == 0 {
            GlyphMask::hollow_box(px)
        } else {
            let (m, coverage) = self.font.rasterize(key_ch, px as f32);
            GlyphMask {
                width: m.width,
                height: m.height,
                xmin: m.xmin,
                ymin: m.ymin,
                advance: m.advance_width,
                coverage,
            }
        };
        self.masks.insert((key_ch, px), mask.clone());
        mask
    }

    /// Rasterize `text` at `px` into an RGBA buffer — the pure core of the
    /// draw path (no SDL, no I/O): glyphs composite into a transparent line
    /// box whose top-left is the caller's `(x, y)` (design D3: the same box
    /// the placeholder bars occupied).
    ///
    /// Placement is anchored to this font's OS/2 typo metrics, not its hhea
    /// line box: Noto Sans SC's hhea reads 1160/-288 (1.448 em, sized for
    /// full ideographs), which would drop labels far below the bar they
    /// replace, while the typo metrics are 880/-120 — exactly 1.0 em. The
    /// baseline therefore sits at 0.88·px with 0.25·px of descender slack
    /// below (the deepest ink in the subset is `g` at -0.250 em; `中` tops
    /// out at 0.840 em, inside the box).
    pub fn render_to_buffer(&mut self, text: &str, px: u16, color: Color) -> RenderedText {
        let px = px.max(1);
        let baseline = (px as f32 * 0.88).round();
        let height = baseline as usize + (px as f32 * 0.25).ceil() as usize;

        // Pass 1: lay the glyphs out and size the line box.
        let mut pen = 0.0f32;
        let mut right = 0.0f32;
        let mut placements = Vec::new();
        for ch in text.chars() {
            let mask = self.mask(ch, px);
            right = right.max(pen + mask.xmin as f32 + mask.width as f32);
            let x0 = pen;
            pen += mask.advance;
            placements.push((mask, x0));
        }
        let width = right.ceil().max(1.0) as usize;
        let mut pixels = vec![0u8; width * height * 4];
        if placements.is_empty() {
            return RenderedText {
                width,
                height,
                pixels,
            };
        }

        // Pass 2: composite (color is constant across the string, so a plain
        // write is a correct "over" onto the transparent buffer).
        let alpha_of = |coverage: u8| ((coverage as u32 * color.a as u32) / 255) as u8;
        for (mask, x0) in placements {
            let left = (x0 + mask.xmin as f32).round() as i32;
            // Bottom edge sits at `baseline - ymin` rows (y-up ymin), and the
            // bitmap occupies the `height` rows ending there.
            let bottom = (baseline - mask.ymin as f32).round() as i32;
            let top = bottom - mask.height as i32 + 1;
            for gy in 0..mask.height {
                let row = top + gy as i32;
                if row < 0 || row as usize >= height {
                    continue;
                }
                for gx in 0..mask.width {
                    let col = left + gx as i32;
                    if col < 0 || col as usize >= width {
                        continue;
                    }
                    let cov = mask.coverage[gy * mask.width + gx];
                    if cov == 0 {
                        continue;
                    }
                    let at = ((row as usize * width) + col as usize) * 4;
                    pixels[at] = color.r;
                    pixels[at + 1] = color.g;
                    pixels[at + 2] = color.b;
                    pixels[at + 3] = alpha_of(cov);
                }
            }
        }
        RenderedText {
            width,
            height,
            pixels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::rgb;

    fn renderer() -> Renderer {
        Renderer::load().expect("the embedded font must parse")
    }

    #[test]
    fn masks_nonempty_for_latin_and_hanzi() {
        let mut r = renderer();
        let a = r.mask('A', 24);
        assert!(a.width > 0 && a.height > 0 && a.any_coverage(), "A: {a:?}");
        let han = r.mask('\u{4e2d}', 24);
        assert!(han.width > 0 && han.height > 0 && han.any_coverage(), "中: {han:?}");
    }

    #[test]
    fn mask_empty_for_space() {
        let mut r = renderer();
        let space = r.mask(' ', 24);
        assert!(!space.any_coverage(), "space must carry no ink: {space:?}");
        assert!(space.advance > 0.0, "space still advances the pen");
    }

    #[test]
    fn mask_cache_hits_by_char_and_size() {
        let mut r = renderer();
        let first = r.mask('A', 24);
        let second = r.mask('A', 24);
        assert_eq!(r.masks.len(), 1);
        assert_eq!(first.coverage, second.coverage);
        r.mask('A', 32);
        assert_eq!(r.masks.len(), 2, "a different px is a different entry");
    }

    #[test]
    fn render_known_string_has_ink() {
        let mut r = renderer();
        let out = r.render_to_buffer("Hi", 24, rgb(255, 255, 255));
        assert!(out.width > 0 && out.height > 0);
        assert!(out.has_ink(), "Hi must put pixels on the buffer");
        assert_eq!(out.pixels.len(), out.width * out.height * 4);
        // Ink is white with full alpha where coverage is solid.
        let opaque = out
            .pixels
            .chunks(4)
            .filter(|px| px[3] == 255)
            .next()
            .expect("at least one fully covered pixel");
        assert_eq!(&opaque[..3], &[255, 255, 255]);
    }

    #[test]
    fn render_empty_text_is_a_blank_buffer() {
        let mut r = renderer();
        let out = r.render_to_buffer("", 24, rgb(255, 255, 255));
        assert!(!out.has_ink());
        assert_eq!(out.pixels.len(), out.width * out.height * 4);
    }

    #[test]
    fn uncovered_char_renders_replacement_square() {
        let mut r = renderer();
        // An emoji outside the subset: must fall back, not panic or vanish.
        let out = r.render_to_buffer("\u{1f3ac}", 24, rgb(255, 255, 255));
        assert!(out.has_ink(), "replacement glyph must be visible");
        // The substitution is cached under the replacement char, not the emoji.
        assert!(r.masks.contains_key(&(REPLACEMENT, 24)));
        assert!(!r.masks.contains_key(&('\u{1f3ac}', 24)));
    }

    #[test]
    fn corrupt_font_bytes_fail_soft() {
        assert!(Renderer::from_bytes(b"this is not a font").is_none());
        assert!(Renderer::from_bytes(b"").is_none());
    }

    #[test]
    fn line_box_keeps_text_within_the_bar_box() {
        let mut r = renderer();
        // The placeholder bar occupied [y, y + px); the line box must not run
        // far past it for ordinary caps text.
        let out = r.render_to_buffer("Browser", 32, rgb(255, 255, 255));
        assert!(
            out.height <= 32 + 8,
            "line box {} should hug the {} px bar",
            out.height,
            32
        );
        assert!(out.has_ink());
    }

    /// Placement math: ink must start in the upper half of the box (the bar's
    /// top was the label's top). Using the font's 1.448 em hhea line box
    /// instead of the typo metrics would sink `Browser` to the bottom — the
    /// regression this test pins.
    #[test]
    fn ink_starts_near_the_top_of_the_box() {
        let mut r = renderer();
        let out = r.render_to_buffer("Browser", 32, rgb(255, 255, 255));
        let first_ink_row = out
            .pixels
            .chunks(4)
            .position(|px| px[3] > 0)
            .map(|linear| linear / out.width)
            .expect("ink must exist");
        assert!(
            first_ink_row < out.height / 2,
            "ink starts at row {first_ink_row} of {} — text sank in its box",
            out.height
        );
    }

    /// Every string literal of `i18n.rs`, comments skipped — the test-side
    /// twin of `ci/subset_font.py`'s scanner (the file holds only plain
    /// double-quoted literals and lifetimes, no raw strings or escapes).
    fn i18n_string_literals(src: &str) -> Vec<String> {
        let chars: Vec<char> = src.chars().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '/' && chars.get(i + 1) == Some(&'/') {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            } else if chars[i] == '"' {
                let mut buf = String::new();
                i += 1;
                while i < chars.len() {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        buf.push(chars[i + 1]);
                        i += 2;
                        continue;
                    }
                    if chars[i] == '"' {
                        break;
                    }
                    buf.push(chars[i]);
                    i += 1;
                }
                out.push(buf);
            } else {
                i += 1;
            }
        }
        out
    }

    /// Task 3.1's gate: the embedded subset must cover the whole i18n
    /// dictionary (both languages). When this fails, a dictionary string
    /// gained a character the subset lacks — re-run `python3
    /// ci/subset_font.py` to regenerate the font.
    #[test]
    fn embedded_subset_covers_the_i18n_dictionary() {
        let r = renderer();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../harbor-platform/src/i18n.rs");
        let src = std::fs::read_to_string(&path).expect("i18n.rs must be readable");
        let mut missing: Vec<char> = Vec::new();
        for literal in i18n_string_literals(&src) {
            for ch in literal.chars() {
                if ch.is_control() || ch == REPLACEMENT {
                    continue;
                }
                if r.font.lookup_glyph_index(ch) == 0 {
                    missing.push(ch);
                }
            }
        }
        missing.sort_unstable();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "i18n characters missing from the embedded subset {missing:?} \
             - re-run `python3 ci/subset_font.py` to regenerate the font"
        );
    }
}
