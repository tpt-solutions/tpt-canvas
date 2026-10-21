//! Glyph outlines: the seam between shaped text and drawing.
//!
//! `tpt-text` shapes text into [`ShapedRun`]s. It does not render them. A
//! [`GlyphSource`] supplies each glyph's outline as a [`Path`] in font units, with y
//! pointing up. [`Context::draw_glyphs`](crate::Context::draw_glyphs) then turns each
//! glyph into an ordinary fill, so every backend draws text without knowing about fonts.
//!
//! Until `tpt-text` exists, applications and tests supply their own source.

use crate::path::Path;

mod base;
mod stroke_font;
pub use base::*;
pub use stroke_font::{StrokeFont, STROKE_FONT_ID, UNITS_PER_EM};

/// Supplies glyph outlines for the fonts a [`ShapedRun`] refers to.
pub trait GlyphSource {
    /// Units per em for `font_id`, or `None` if the font is unknown.
    fn units_per_em(&self, font_id: u32) -> Option<f32>;

    /// Outline of `glyph` in font units (y up, origin at the glyph origin), or `None`
    /// if the font has no outline for it. Glyphs without an outline are skipped.
    fn outline(&self, font_id: u32, glyph: GlyphId) -> Option<Path>;
}
