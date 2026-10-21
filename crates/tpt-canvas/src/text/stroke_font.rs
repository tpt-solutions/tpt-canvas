//! A built-in stroke font, so examples can label charts before `tpt-text` exists.
//!
//! Each glyph is a few polylines on a 5 x 7 grid, and its outline is those lines stroked
//! with round caps and joins. The font covers digits, capital letters (lowercase maps to
//! capitals), and a few punctuation marks. It is for labels, not for body text.

use alloc::vec::Vec;

use crate::path::{Path, PathBuilder};
use crate::stroke::stroke_outline;
use crate::style::{LineCap, LineJoin, StrokeStyle};
use crate::text::{GlyphId, GlyphSource, ShapedRun};
use crate::Vec2;

/// The font id that [`StrokeFont`] answers for. Any other id is unknown.
pub const STROKE_FONT_ID: u32 = 0;

/// Units per em for the built-in font. Glyph outlines are in these units, y up.
pub const UNITS_PER_EM: f32 = 100.0;

/// Size of one grid cell in font units.
const CELL: f32 = 14.0;

/// Horizontal advance of one glyph, in font units. Glyphs are 4 cells wide with a gap.
const ADVANCE: f32 = 6.0 * CELL;

/// Stroke width in font units.
const STROKE: f32 = 12.0;

/// Flattening tolerance for the stroke outlines, in font units.
const TOLERANCE: f32 = 0.05;

/// The built-in stroke font. It implements [`GlyphSource`], so it works with
/// [`Context::set_glyph_source`](crate::Context::set_glyph_source) and `draw_glyphs`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StrokeFont;

impl StrokeFont {
    /// Lays out `text` on one baseline, starting at the origin, at `font_size` user units.
    ///
    /// Characters the font does not cover get no outline but still take up an advance, so
    /// columns stay aligned.
    pub fn layout(text: &str, font_size: f32) -> ShapedRun {
        let mut run = ShapedRun::new(STROKE_FONT_ID, font_size);
        let step = ADVANCE * font_size / UNITS_PER_EM;
        for (i, c) in text.chars().enumerate() {
            run.push(c as GlyphId, Vec2::new(i as f32 * step, 0.0));
        }
        run
    }

    /// The width of `text` in user units, from the start of the first glyph to the right
    /// edge of the last one.
    pub fn width(text: &str, font_size: f32) -> f32 {
        let chars = text.chars().count() as f32;
        let scale = font_size / UNITS_PER_EM;
        (chars - 1.0).max(0.0) * ADVANCE * scale + (4.0 * CELL) * scale
    }
}

impl GlyphSource for StrokeFont {
    fn units_per_em(&self, font_id: u32) -> Option<f32> {
        (font_id == STROKE_FONT_ID).then_some(UNITS_PER_EM)
    }

    fn outline(&self, font_id: u32, glyph: GlyphId) -> Option<Path> {
        if font_id != STROKE_FONT_ID {
            return None;
        }
        let c = char::from_u32(glyph)?.to_ascii_uppercase();
        let strokes = glyph_strokes(c)?;
        if strokes.is_empty() {
            return None;
        }
        let mut lines = PathBuilder::new();
        for polyline in strokes {
            for (i, &(x, y)) in polyline.iter().enumerate() {
                let (x, y) = (x * CELL, y * CELL);
                if i == 0 {
                    lines.move_to(x, y);
                } else {
                    lines.line_to(x, y);
                }
            }
        }
        let style = StrokeStyle {
            width: STROKE,
            cap: LineCap::Round,
            join: LineJoin::Round,
            ..StrokeStyle::default()
        };
        // Each stroke outline is a closed polygon that fills with the nonzero rule, so
        // overlapping strokes merge into one glyph.
        let polygons = stroke_outline(&lines.build().flatten(TOLERANCE), &style, TOLERANCE);
        let mut out = PathBuilder::new();
        for poly in polygons.iter().filter(|p| p.len() >= 3) {
            out.move_to(poly[0].x, poly[0].y);
            for p in &poly[1..] {
                out.line_to(p.x, p.y);
            }
            out.close();
        }
        Some(out.build())
    }
}

/// Polylines for one glyph, on a grid of x in 0..=4 and y in 0..=6, y up.
fn glyph_strokes(c: char) -> Option<Vec<Vec<(f32, f32)>>> {
    // Each stroke is a list of grid points. They are small integers, so the table stays
    // readable.
    type Stroke = &'static [(u8, u8)];
    let strokes: &[Stroke] = match c {
        ' ' => &[],
        '0' => &[&[
            (0, 1),
            (0, 5),
            (1, 6),
            (3, 6),
            (4, 5),
            (4, 1),
            (3, 0),
            (1, 0),
            (0, 1),
        ]],
        '1' => &[&[(1, 5), (2, 6), (2, 0)], &[(1, 0), (3, 0)]],
        '2' => &[&[(0, 5), (1, 6), (3, 6), (4, 5), (4, 4), (0, 0), (4, 0)]],
        '3' => &[
            &[(0, 5), (1, 6), (3, 6), (4, 5), (4, 4), (3, 3), (1, 3)],
            &[(3, 3), (4, 2), (4, 1), (3, 0), (1, 0), (0, 1)],
        ],
        '4' => &[&[(3, 0), (3, 6), (0, 2), (4, 2)]],
        '5' => &[&[
            (4, 6),
            (0, 6),
            (0, 3),
            (3, 3),
            (4, 2),
            (4, 1),
            (3, 0),
            (1, 0),
            (0, 1),
        ]],
        '6' => &[&[
            (4, 5),
            (3, 6),
            (1, 6),
            (0, 5),
            (0, 1),
            (1, 0),
            (3, 0),
            (4, 1),
            (4, 3),
            (3, 4),
            (1, 4),
            (0, 3),
        ]],
        '7' => &[&[(0, 6), (4, 6), (1, 0)]],
        '8' => &[&[
            (1, 3),
            (0, 4),
            (0, 5),
            (1, 6),
            (3, 6),
            (4, 5),
            (4, 4),
            (3, 3),
            (1, 3),
            (0, 2),
            (0, 1),
            (1, 0),
            (3, 0),
            (4, 1),
            (4, 2),
            (3, 3),
        ]],
        '9' => &[&[
            (4, 3),
            (3, 4),
            (1, 4),
            (0, 5),
            (1, 6),
            (3, 6),
            (4, 5),
            (4, 1),
            (3, 0),
            (1, 0),
        ]],
        'A' => &[&[(0, 0), (2, 6), (4, 0)], &[(1, 3), (3, 3)]],
        'B' => &[
            &[(0, 0), (0, 6), (3, 6), (4, 5), (4, 4), (3, 3), (0, 3)],
            &[(3, 3), (4, 2), (4, 1), (3, 0), (0, 0)],
        ],
        'C' => &[&[
            (4, 5),
            (3, 6),
            (1, 6),
            (0, 5),
            (0, 1),
            (1, 0),
            (3, 0),
            (4, 1),
        ]],
        'D' => &[&[(0, 0), (0, 6), (2, 6), (4, 4), (4, 2), (2, 0), (0, 0)]],
        'E' => &[&[(4, 6), (0, 6), (0, 0), (4, 0)], &[(0, 3), (3, 3)]],
        'F' => &[&[(4, 6), (0, 6), (0, 0)], &[(0, 3), (3, 3)]],
        'G' => &[&[
            (4, 5),
            (3, 6),
            (1, 6),
            (0, 5),
            (0, 1),
            (1, 0),
            (3, 0),
            (4, 1),
            (4, 3),
            (2, 3),
        ]],
        'H' => &[&[(0, 0), (0, 6)], &[(4, 0), (4, 6)], &[(0, 3), (4, 3)]],
        'I' => &[&[(1, 6), (3, 6)], &[(2, 6), (2, 0)], &[(1, 0), (3, 0)]],
        'J' => &[&[(4, 6), (4, 1), (3, 0), (1, 0), (0, 1)]],
        'K' => &[&[(0, 0), (0, 6)], &[(4, 6), (0, 2)], &[(1, 3), (4, 0)]],
        'L' => &[&[(0, 6), (0, 0), (4, 0)]],
        'M' => &[&[(0, 0), (0, 6), (2, 3), (4, 6), (4, 0)]],
        'N' => &[&[(0, 0), (0, 6), (4, 0), (4, 6)]],
        'O' => &[&[
            (1, 0),
            (0, 1),
            (0, 5),
            (1, 6),
            (3, 6),
            (4, 5),
            (4, 1),
            (3, 0),
            (1, 0),
        ]],
        'P' => &[&[(0, 0), (0, 6), (3, 6), (4, 5), (4, 4), (3, 3), (0, 3)]],
        'Q' => &[
            &[
                (1, 0),
                (0, 1),
                (0, 5),
                (1, 6),
                (3, 6),
                (4, 5),
                (4, 1),
                (3, 0),
                (1, 0),
            ],
            &[(2, 2), (4, 0)],
        ],
        'R' => &[
            &[(0, 0), (0, 6), (3, 6), (4, 5), (4, 4), (3, 3), (0, 3)],
            &[(2, 3), (4, 0)],
        ],
        'S' => &[&[
            (4, 5),
            (3, 6),
            (1, 6),
            (0, 5),
            (0, 4),
            (1, 3),
            (3, 3),
            (4, 2),
            (4, 1),
            (3, 0),
            (1, 0),
            (0, 1),
        ]],
        'T' => &[&[(0, 6), (4, 6)], &[(2, 6), (2, 0)]],
        'U' => &[&[(0, 6), (0, 1), (1, 0), (3, 0), (4, 1), (4, 6)]],
        'V' => &[&[(0, 6), (2, 0), (4, 6)]],
        'W' => &[&[(0, 6), (1, 0), (2, 3), (3, 0), (4, 6)]],
        'X' => &[&[(0, 0), (4, 6)], &[(0, 6), (4, 0)]],
        'Y' => &[&[(0, 6), (2, 3), (4, 6)], &[(2, 3), (2, 0)]],
        'Z' => &[&[(0, 6), (4, 6), (0, 0), (4, 0)]],
        '.' => &[&[(2, 0), (2, 1)]],
        '-' => &[&[(0, 3), (4, 3)]],
        '+' => &[&[(0, 3), (4, 3)], &[(2, 1), (2, 5)]],
        '=' => &[&[(0, 2), (4, 2)], &[(0, 4), (4, 4)]],
        ':' => &[&[(2, 1), (2, 2)], &[(2, 4), (2, 5)]],
        '/' => &[&[(0, 0), (4, 6)]],
        '!' => &[&[(2, 6), (2, 2)], &[(2, 0), (2, 1)]],
        _ => return None,
    };
    Some(
        strokes
            .iter()
            .map(|s| {
                s.iter()
                    .map(|&(x, y)| (x as f32, y as f32))
                    .collect::<Vec<_>>()
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covered_characters_have_outlines_and_others_do_not() {
        let font = StrokeFont;
        for c in "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ.-+=:/!".chars() {
            assert!(
                font.outline(STROKE_FONT_ID, c as GlyphId).is_some(),
                "no outline for {c}"
            );
        }
        assert!(font.outline(STROKE_FONT_ID, '@' as GlyphId).is_none());
        assert!(font.outline(STROKE_FONT_ID, ' ' as GlyphId).is_none());
    }

    #[test]
    fn lowercase_uses_the_capital_outline() {
        let font = StrokeFont;
        assert_eq!(
            font.outline(STROKE_FONT_ID, 'a' as GlyphId),
            font.outline(STROKE_FONT_ID, 'A' as GlyphId)
        );
    }

    #[test]
    fn other_fonts_are_unknown() {
        let font = StrokeFont;
        assert_eq!(font.units_per_em(STROKE_FONT_ID), Some(UNITS_PER_EM));
        assert_eq!(font.units_per_em(7), None);
        assert!(font.outline(7, 'A' as GlyphId).is_none());
    }

    #[test]
    fn layout_spaces_glyphs_by_the_advance() {
        let run = StrokeFont::layout("AB", 100.0);
        assert_eq!(run.glyphs.len(), 2);
        assert_eq!(run.glyphs[1].pos.x, ADVANCE);
        assert_eq!(run.font_id, STROKE_FONT_ID);
    }

    #[test]
    fn width_matches_the_layout_extent() {
        let w = StrokeFont::width("ABC", 100.0);
        assert_eq!(w, 2.0 * ADVANCE + 4.0 * CELL);
    }
}
