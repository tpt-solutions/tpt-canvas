//! Retained mode: a display list records backend calls and replays them later.
//!
//! A `DisplayList` is itself a [`CanvasBackend`], so `Context` can record into it
//! directly. Replaying into any backend yields the same output as drawing
//! immediately; the core test suite checks this for a sample scene.

mod codec;
pub mod diff;

pub use codec::DisplayDecodeError;

use alloc::vec::Vec;

use crate::backend::CanvasBackend;
use crate::image::ImageRef;
use crate::path::Path;
use crate::style::{BlendMode, FillRule, Paint, StrokeStyle};
use crate::text::ShapedRun;
use crate::{Affine2, Rect};

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
    /// Clears a path to transparent.
    ClearPath(Path),
    /// Fills a path as a blurred shadow with standard deviation `sigma`.
    FillPathBlurred {
        /// The path to fill.
        path: Path,
        /// The paint to fill with.
        paint: Paint,
        /// The winding rule.
        rule: FillRule,
        /// Gaussian standard deviation in user units.
        sigma: f32,
    },
    /// Pushes a clip.
    PushClip(Path),
    /// Pops a clip.
    PopClip,
    /// Pushes a transform.
    PushTransform(Affine2),
    /// Pops a transform.
    PopTransform,
    /// Starts a group with the given alpha.
    PushLayer(f32),
    /// Ends the most recent group.
    PopLayer,
    /// Sets the blend mode for later fills and strokes.
    SetBlendMode(BlendMode),
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
                DrawOp::ClearPath(path) => backend.clear_path(path),
                DrawOp::FillPathBlurred {
                    path,
                    paint,
                    rule,
                    sigma,
                } => backend.fill_path_blurred(path, paint, *rule, *sigma),
                DrawOp::PushClip(path) => backend.push_clip(path),
                DrawOp::PopClip => backend.pop_clip(),
                DrawOp::PushTransform(m) => backend.push_transform(m),
                DrawOp::PopTransform => backend.pop_transform(),
                DrawOp::PushLayer(alpha) => backend.push_layer(*alpha),
                DrawOp::PopLayer => backend.pop_layer(),
                DrawOp::SetBlendMode(mode) => backend.set_blend_mode(*mode),
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
    fn clear_path(&mut self, path: &Path) {
        self.ops.push(DrawOp::ClearPath(path.clone()));
    }
    fn fill_path_blurred(&mut self, path: &Path, paint: &Paint, rule: FillRule, sigma: f32) {
        self.ops.push(DrawOp::FillPathBlurred {
            path: path.clone(),
            paint: paint.clone(),
            rule,
            sigma,
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
    fn push_layer(&mut self, alpha: f32) {
        self.ops.push(DrawOp::PushLayer(alpha));
    }
    fn pop_layer(&mut self) {
        self.ops.push(DrawOp::PopLayer);
    }
    fn set_blend_mode(&mut self, mode: BlendMode) {
        self.ops.push(DrawOp::SetBlendMode(mode));
    }
}
