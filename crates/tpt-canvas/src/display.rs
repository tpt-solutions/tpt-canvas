//! Retained mode: a display list records backend calls and replays them later.
//!
//! A `DisplayList` is itself a [`CanvasBackend`], so `Context` can record into it
//! directly. Replaying into any backend yields the same output as drawing
//! immediately; the core test suite checks this for a sample scene.

mod codec;

pub use codec::DisplayDecodeError;

use alloc::vec::Vec;

use crate::backend::CanvasBackend;
use crate::image::ImageRef;
use crate::path::Path;
use crate::style::{FillRule, Paint, StrokeStyle};
use crate::{Affine2, Rect};
use tpt_text_stub::ShapedRun;

/// One recorded backend call.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawOp {
    /// Fills a path.
    FillPath {
        /// The path to fill.
        path: Path,
        /// The paint to fill with.
        paint: Paint,
        /// The winding rule.
        rule: FillRule,
    },
    /// Strokes a path.
    StrokePath {
        /// The path to stroke.
        path: Path,
        /// The paint to stroke with.
        paint: Paint,
        /// The stroke geometry.
        style: StrokeStyle,
    },
    /// Draws a glyph run.
    DrawGlyphs {
        /// The run to draw.
        run: ShapedRun,
        /// The paint to draw with.
        paint: Paint,
    },
    /// Draws an image region.
    DrawImage {
        /// The image.
        image: ImageRef,
        /// The source region.
        src: Rect,
        /// The destination rectangle.
        dst: Rect,
    },
    /// Pushes a clip.
    PushClip(Path),
    /// Pops a clip.
    PopClip,
    /// Pushes a transform.
    PushTransform(Affine2),
    /// Pops a transform.
    PopTransform,
}

#[derive(Clone, Debug, Default, PartialEq)]
/// A recorded sequence of backend calls. It is a backend itself, so it can record and replay.
pub struct DisplayList {
    ops: Vec<DrawOp>,
}

impl DisplayList {
    /// Creates an empty display list.
    pub fn new() -> Self {
        DisplayList::default()
    }

    fn push_op(&mut self, op: DrawOp) {
        self.ops.push(op);
    }

    /// The recorded operations, in order.
    pub fn ops(&self) -> &[DrawOp] {
        &self.ops
    }

    /// The number of recorded operations.
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Whether no operations are recorded.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Re-issues every recorded call, in order, against `backend`.
    pub fn replay<B: CanvasBackend + ?Sized>(&self, backend: &mut B) {
        for op in &self.ops {
            match op {
                DrawOp::FillPath { path, paint, rule } => backend.fill_path(path, paint, *rule),
                DrawOp::StrokePath { path, paint, style } => {
                    backend.stroke_path(path, paint, style)
                }
                DrawOp::DrawGlyphs { run, paint } => backend.draw_glyphs(run, paint),
                DrawOp::DrawImage { image, src, dst } => backend.draw_image(image, *src, *dst),
                DrawOp::PushClip(path) => backend.push_clip(path),
                DrawOp::PopClip => backend.pop_clip(),
                DrawOp::PushTransform(m) => backend.push_transform(m),
                DrawOp::PopTransform => backend.pop_transform(),
            }
        }
    }
}

impl CanvasBackend for DisplayList {
    fn fill_path(&mut self, path: &Path, paint: &Paint, rule: FillRule) {
        self.ops.push(DrawOp::FillPath {
            path: path.clone(),
            paint: paint.clone(),
            rule,
        });
    }
    fn stroke_path(&mut self, path: &Path, paint: &Paint, style: &StrokeStyle) {
        self.ops.push(DrawOp::StrokePath {
            path: path.clone(),
            paint: paint.clone(),
            style: style.clone(),
        });
    }
    fn draw_glyphs(&mut self, run: &ShapedRun, paint: &Paint) {
        self.ops.push(DrawOp::DrawGlyphs {
            run: run.clone(),
            paint: paint.clone(),
        });
    }
    fn draw_image(&mut self, image: &ImageRef, src_rect: Rect, dst_rect: Rect) {
        self.ops.push(DrawOp::DrawImage {
            image: *image,
            src: src_rect,
            dst: dst_rect,
        });
    }
    fn push_clip(&mut self, path: &Path) {
        self.ops.push(DrawOp::PushClip(path.clone()));
    }
    fn pop_clip(&mut self) {
        self.ops.push(DrawOp::PopClip);
    }
    fn push_transform(&mut self, transform: &Affine2) {
        self.ops.push(DrawOp::PushTransform(*transform));
    }
    fn pop_transform(&mut self) {
        self.ops.push(DrawOp::PopTransform);
    }
}
