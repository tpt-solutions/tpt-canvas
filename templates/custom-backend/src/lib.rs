//! A starting point for your own `CanvasBackend`.
//!
//! This backend records each call as a name and counts the ones it cannot draw. Replace the
//! bodies with your output: a file format, a GPU command stream, or a display surface.
//!
//! Rules from the trait docs:
//! - Count what you skip or approximate in `unsupported`. Never drop a call silently.
//! - `push_*` and `pop_*` must balance, and `pop_*` undoes the most recent `push_*`.
//! - Backends that cannot erase must count `clear_path`, not skip it.

use tpt_canvas::path::Path;
use tpt_canvas::text::ShapedRun;
use tpt_canvas::{Affine2, BlendMode, CanvasBackend, FillRule, ImageRef, Paint, Rect, StrokeStyle};

/// A backend that records the calls it receives.
#[derive(Debug, Default)]
pub struct Recorder {
    /// One entry per call, in order, such as `"fill_path"`.
    pub calls: Vec<&'static str>,
    /// Calls this backend cannot draw.
    pub unsupported: u32,
}

impl CanvasBackend for Recorder {
    fn fill_path(&mut self, _path: &Path, _paint: &Paint, _rule: FillRule) {
        self.calls.push("fill_path");
    }

    fn stroke_path(&mut self, _path: &Path, _paint: &Paint, _style: &StrokeStyle) {
        self.calls.push("stroke_path");
    }

    fn draw_glyphs(&mut self, _run: &ShapedRun, _paint: &Paint) {
        self.calls.push("draw_glyphs");
        self.unsupported += 1;
    }

    fn draw_image(&mut self, _image: &ImageRef, _src_rect: Rect, _dst_rect: Rect) {
        self.calls.push("draw_image");
        self.unsupported += 1;
    }

    fn clear_path(&mut self, _path: &Path) {
        self.calls.push("clear_path");
        self.unsupported += 1;
    }

    fn fill_path_blurred(&mut self, _path: &Path, _paint: &Paint, _rule: FillRule, _sigma: f32) {
        self.calls.push("fill_path_blurred");
        self.unsupported += 1;
    }

    fn set_blend_mode(&mut self, _mode: BlendMode) {
        self.calls.push("set_blend_mode");
    }

    fn push_clip(&mut self, _path: &Path) {
        self.calls.push("push_clip");
    }

    fn pop_clip(&mut self) {
        self.calls.push("pop_clip");
    }

    fn push_transform(&mut self, _transform: &Affine2) {
        self.calls.push("push_transform");
    }

    fn pop_transform(&mut self) {
        self.calls.push("pop_transform");
    }

    fn push_layer(&mut self, _alpha: f32) {
        self.calls.push("push_layer");
        self.unsupported += 1;
    }

    fn pop_layer(&mut self) {
        self.calls.push("pop_layer");
        self.unsupported += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_canvas::prelude::*;

    #[test]
    fn records_calls_in_order() {
        let mut ctx = Context::new(Recorder::default());
        ctx.save();
        ctx.translate(2.0, 3.0);
        ctx.fill_rect(0.0, 0.0, 4.0, 4.0);
        ctx.restore();
        let rec = ctx.into_backend();
        assert_eq!(rec.calls, ["push_transform", "fill_path", "pop_transform"]);
        assert_eq!(rec.unsupported, 0);
    }

    #[test]
    fn counts_what_it_cannot_draw() {
        let mut ctx = Context::new(Recorder::default());
        ctx.clear_rect(0.0, 0.0, 4.0, 4.0);
        assert_eq!(ctx.into_backend().unsupported, 1);
    }
}
