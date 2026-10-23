//! Text for tpt-canvas, from [tpt-text](https://github.com/tpt-solutions/tpt-text).
//!
//! tpt-text shapes and lays out paragraphs. [`TextRuns::from_layout`] turns a laid-out
//! paragraph into the runs `tpt-canvas` draws, and [`TextRuns::glyph_source`] supplies the
//! outlines those runs refer to. Drawing then goes through `Context::draw_glyphs`, as for any
//! other glyph source.
//!
//! Glyph positions follow tpt-text's own SVG output: the pen starts at each line's offset,
//! each glyph sits at the pen plus its offset on the line's baseline, and the pen advances by
//! the glyph's advance.
#![warn(missing_docs)]

use std::sync::Arc;

use tpt_canvas::text::{GlyphId, GlyphSource, ShapedRun};
use tpt_canvas::{CanvasBackend, Context, Path, PathBuilder, Vec2};
use tpt_text::{FontFace, Fx, ParagraphLayout, PathSegment};

/// The laid-out text of a paragraph, ready to draw.
pub struct TextRuns {
    faces: Arc<Vec<Arc<FontFace>>>,
    runs: Vec<ShapedRun>,
}

impl TextRuns {
    /// Converts a laid-out paragraph into one `ShapedRun` per run of each line.
    ///
    /// Each distinct font becomes a font id, in the order the runs first use it, so the ids
    /// index the faces that [`glyph_source`](Self::glyph_source) answers for.
    pub fn from_layout(layout: &ParagraphLayout) -> TextRuns {
        let mut faces: Vec<Arc<FontFace>> = Vec::new();
        let mut runs = Vec::new();
        for line in &layout.lines {
            let mut pen = line.x_offset;
            for run in &line.runs {
                let font_id = face_id(&mut faces, &run.font);
                let mut out = ShapedRun::new(font_id, run.style.size.to_f32());
                for g in &run.glyphs {
                    let x = pen + g.x_offset;
                    let y = line.baseline_y + g.y_offset;
                    out.push(g.glyph_id, Vec2::new(x.to_f32(), y.to_f32()));
                    pen += g.x_advance;
                }
                runs.push(out);
            }
        }
        TextRuns {
            faces: Arc::new(faces),
            runs,
        }
    }

    /// The runs, in line order, each positioned on the paragraph's coordinates.
    pub fn runs(&self) -> &[ShapedRun] {
        &self.runs
    }

    /// A glyph source that answers for the faces these runs use. Install it with
    /// `Context::set_glyph_source` before drawing the runs.
    pub fn glyph_source(&self) -> Box<dyn GlyphSource> {
        Box::new(FaceOutlines {
            faces: self.faces.clone(),
        })
    }

    /// Draws every run through `ctx`, at the positions the layout gave them.
    ///
    /// This replaces the context's glyph source with this text's.
    pub fn draw<B: CanvasBackend>(&self, ctx: &mut Context<B>) {
        ctx.set_glyph_source(self.glyph_source());
        for run in &self.runs {
            ctx.draw_glyphs(run);
        }
    }
}

/// The id of `face` in `faces`, adding it on first use.
fn face_id(faces: &mut Vec<Arc<FontFace>>, face: &Arc<FontFace>) -> u32 {
    let index = match faces.iter().position(|f| Arc::ptr_eq(f, face)) {
        Some(i) => i,
        None => {
            faces.push(face.clone());
            faces.len() - 1
        }
    };
    index as u32
}

/// Outlines for the faces of a [`TextRuns`].
struct FaceOutlines {
    faces: Arc<Vec<Arc<FontFace>>>,
}

impl GlyphSource for FaceOutlines {
    fn units_per_em(&self, font_id: u32) -> Option<f32> {
        self.faces
            .get(font_id as usize)
            .map(|f| f32::from(f.units_per_em()))
    }

    fn outline(&self, font_id: u32, glyph: GlyphId) -> Option<Path> {
        let face = self.faces.get(font_id as usize)?;
        // An outline at one em is in font units. tpt-text gives it y down, but a
        // `GlyphSource` is y up, so the y coordinates are negated.
        let size = Fx::from_int(i32::from(face.units_per_em()));
        let segments = face.glyph_outline(glyph, size);
        if segments.is_empty() {
            return None;
        }
        let mut b = PathBuilder::new();
        for seg in segments {
            match seg {
                PathSegment::MoveTo(x, y) => b.move_to(x.to_f32(), -y.to_f32()),
                PathSegment::LineTo(x, y) => b.line_to(x.to_f32(), -y.to_f32()),
                PathSegment::QuadTo(x1, y1, x, y) => {
                    b.quad_to(x1.to_f32(), -y1.to_f32(), x.to_f32(), -y.to_f32());
                }
                PathSegment::CubicTo(x1, y1, x2, y2, x, y) => b.cubic_to(
                    x1.to_f32(),
                    -y1.to_f32(),
                    x2.to_f32(),
                    -y2.to_f32(),
                    x.to_f32(),
                    -y.to_f32(),
                ),
                PathSegment::Close => b.close(),
            }
        }
        Some(b.build())
    }
}

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_examples {}
