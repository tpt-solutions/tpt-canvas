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
use crate::{Affine2, Rect};
use tpt_text_stub::ShapedRun;

pub trait CanvasBackend {
    fn fill_path(&mut self, path: &Path, paint: &Paint, rule: FillRule);
    fn stroke_path(&mut self, path: &Path, paint: &Paint, style: &StrokeStyle);
    fn draw_glyphs(&mut self, run: &ShapedRun, paint: &Paint);
    fn draw_image(&mut self, image: &ImageRef, src_rect: Rect, dst_rect: Rect);

    fn push_clip(&mut self, path: &Path);
    fn pop_clip(&mut self);

    fn push_transform(&mut self, transform: &Affine2);
    fn pop_transform(&mut self);
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
}
