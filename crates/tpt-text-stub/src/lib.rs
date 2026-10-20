//! Stub for `tpt-text`: shaped runs and positioned glyphs.
//!
//! tpt-canvas never shapes text; it only receives positioned glyphs and draws them.
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use tpt_math_stub::Vec2;

pub type GlyphId = u32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionedGlyph {
    pub id: GlyphId,
    /// Glyph origin relative to the run origin.
    pub pos: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapedRun {
    pub font_id: u32,
    pub font_size: f32,
    pub glyphs: Vec<PositionedGlyph>,
}

impl ShapedRun {
    pub fn new(font_id: u32, font_size: f32) -> Self {
        ShapedRun {
            font_id,
            font_size,
            glyphs: Vec::new(),
        }
    }

    pub fn push(&mut self, id: GlyphId, pos: Vec2) {
        self.glyphs.push(PositionedGlyph { id, pos });
    }
}
