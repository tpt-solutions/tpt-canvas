//! The rendering abstraction (Module D).
//!
//! Geometry arrives in user space. Backends keep their own transform stack:
//! `push_transform` concatenates a matrix onto the current transform, and
//! `pop_transform` undoes the most recent push. Clips follow the same rule with
//! `push_clip` and `pop_clip`, and a clip is interpreted under the transform that
//! is current when it is pushed.

use crate::image::ImageRef;
use crate::path::Path;
use crate::style::{FillRule, Paint, StrokeStyle};
use crate::text::ShapedRun;
use crate::{Affine2, Rect};

/// The rendering abstraction. Geometry arrives in user space; backends keep their own transform and clip stacks.
pub trait CanvasBackend {
    /// Fills a path with `paint` under the winding rule `rule`.
    fn fill_path(&mut self, path: &Path, paint: &Paint, rule: FillRule);
    /// Strokes a path with `paint` and `style`.
    fn stroke_path(&mut self, path: &Path, paint: &Paint, style: &StrokeStyle);
    /// Draws a positioned glyph run.
    fn draw_glyphs(&mut self, run: &ShapedRun, paint: &Paint);
    /// Draws the part `src_rect` of an image into `dst_rect`.
    fn draw_image(&mut self, image: &ImageRef, src_rect: Rect, dst_rect: Rect);
    /// Sets the pixels inside `path` back to transparent, ignoring paint and global alpha.
    ///
    /// The current transform and clip apply, as for a fill. Backends whose output cannot
    /// be erased (for example a vector file that only appends) must count the call
    /// rather than skip it silently.
    fn clear_path(&mut self, path: &Path);

    /// Intersects the current clip with `path`. Must be balanced by `pop_clip`.
    fn push_clip(&mut self, path: &Path);
    /// Undoes the most recent `push_clip`.
    fn pop_clip(&mut self);

    /// Concatenates `transform` onto the current transform. Must be balanced by `pop_transform`.
    fn push_transform(&mut self, transform: &Affine2);
    /// Undoes the most recent `push_transform`.
    fn pop_transform(&mut self);

    /// Starts a group whose drawing is composited onto the content below with `alpha` (0..=1),
    /// as one unit. Must be balanced by `pop_layer`. Layers do not reset the transform or clip.
    ///
    /// Backends that cannot composite a group must count the call rather than skip it.
    fn push_layer(&mut self, alpha: f32);
    /// Ends the most recent `push_layer` and composites its group.
    fn pop_layer(&mut self);
}

/// Forwarding impl so `Context` can hold `&mut dyn CanvasBackend`.
impl<B: CanvasBackend + ?Sized> CanvasBackend for &mut B {
    fn fill_path(&mut self, path: &Path, paint: &Paint, rule: FillRule) {
        (**self).fill_path(path, paint, rule)
    }
    fn stroke_path(&mut self, path: &Path, paint: &Paint, style: &StrokeStyle) {
        (**self).stroke_path(path, paint, style)
    }
    fn draw_glyphs(&mut self, run: &ShapedRun, paint: &Paint) {
        (**self).draw_glyphs(run, paint)
    }
    fn draw_image(&mut self, image: &ImageRef, src_rect: Rect, dst_rect: Rect) {
        (**self).draw_image(image, src_rect, dst_rect)
    }
    fn clear_path(&mut self, path: &Path) {
        (**self).clear_path(path)
    }
    fn push_clip(&mut self, path: &Path) {
        (**self).push_clip(path)
    }
    fn pop_clip(&mut self) {
        (**self).pop_clip()
    }
    fn push_transform(&mut self, transform: &Affine2) {
        (**self).push_transform(transform)
    }
    fn pop_transform(&mut self) {
        (**self).pop_transform()
    }
    fn push_layer(&mut self, alpha: f32) {
        (**self).push_layer(alpha)
    }
    fn pop_layer(&mut self) {
        (**self).pop_layer()
    }
}
