//! Drawing state and the immediate-mode API (Module C).
//!
//! `Context` tracks the current transform, default styles and clip depth, and
//! forwards every call to a [`CanvasBackend`]. `save`/`restore` mirror the state
//! stack: restoring pops the transforms and clips pushed since the matching save,
//! so the backend's stacks always stay balanced.

use alloc::vec::Vec;

use crate::backend::CanvasBackend;
use crate::image::ImageRef;
use crate::path::Path;
use crate::style::{FillRule, Paint, StrokeStyle};
use crate::{Affine2, Color, Rect};
use tpt_text_stub::ShapedRun;

#[derive(Clone, Debug)]
struct State {
    ctm: Affine2,
    fill: Paint,
    stroke: Paint,
    fill_rule: FillRule,
    stroke_style: StrokeStyle,
    /// Transform pushes made at this save level.
    transform_ops: u32,
    /// Clip pushes made at this save level.
    clip_ops: u32,
}

impl Default for State {
    fn default() -> Self {
        State {
            ctm: Affine2::IDENTITY,
            fill: Paint::Solid(Color::BLACK),
            stroke: Paint::Solid(Color::BLACK),
            fill_rule: FillRule::NonZero,
            stroke_style: StrokeStyle::default(),
            transform_ops: 0,
            clip_ops: 0,
        }
    }
}

pub struct Context<B: CanvasBackend> {
    backend: B,
    state: State,
    stack: Vec<State>,
}

impl<B: CanvasBackend> Context<B> {
    pub fn new(backend: B) -> Self {
        Context {
            backend,
            state: State::default(),
            stack: Vec::new(),
        }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn into_backend(self) -> B {
        self.backend
    }

    /// Current user-to-device-independent transform (the product of all pushes).
    pub fn current_transform(&self) -> Affine2 {
        self.state.ctm
    }

    pub fn save(&mut self) {
        let mut next = self.state.clone();
        next.transform_ops = 0;
        next.clip_ops = 0;
        let outer = core::mem::replace(&mut self.state, next);
        self.stack.push(outer);
    }

    /// Undoes everything since the matching `save`. Extra restores are ignored.
    pub fn restore(&mut self) {
        let Some(outer) = self.stack.pop() else {
            return;
        };
        for _ in 0..self.state.clip_ops {
            self.backend.pop_clip();
        }
        for _ in 0..self.state.transform_ops {
            self.backend.pop_transform();
        }
        self.state = outer;
    }

    /// Concatenates `m` onto the current transform.
    pub fn transform(&mut self, m: Affine2) {
        self.state.ctm = self.state.ctm * m;
        self.backend.push_transform(&m);
        self.state.transform_ops += 1;
    }

    pub fn translate(&mut self, tx: f32, ty: f32) {
        self.transform(Affine2::translate(tx, ty));
    }

    pub fn rotate(&mut self, theta: f32) {
        self.transform(Affine2::rotate(theta));
    }

    pub fn scale(&mut self, sx: f32, sy: f32) {
        self.transform(Affine2::scale(sx, sy));
    }

    /// Replaces the current transform with `m`. Does nothing if the current
    /// transform is singular, because the needed correction cannot be formed.
    pub fn set_transform(&mut self, m: Affine2) {
        let Some(inv) = self.state.ctm.invert() else {
            return;
        };
        let delta = inv * m;
        self.backend.push_transform(&delta);
        self.state.ctm = m;
        self.state.transform_ops += 1;
    }

    /// Intersects the current clip with `path`, in the current user space.
    pub fn clip(&mut self, path: &Path) {
        self.backend.push_clip(path);
        self.state.clip_ops += 1;
    }

    pub fn set_fill_paint(&mut self, paint: Paint) {
        self.state.fill = paint;
    }

    pub fn set_stroke_paint(&mut self, paint: Paint) {
        self.state.stroke = paint;
    }

    pub fn set_fill_rule(&mut self, rule: FillRule) {
        self.state.fill_rule = rule;
    }

    pub fn set_stroke_style(&mut self, style: StrokeStyle) {
        self.state.stroke_style = style;
    }

    pub fn fill_path(&mut self, path: &Path) {
        self.backend
            .fill_path(path, &self.state.fill, self.state.fill_rule);
    }

    pub fn stroke_path(&mut self, path: &Path) {
        self.backend
            .stroke_path(path, &self.state.stroke, &self.state.stroke_style);
    }

    pub fn draw_glyphs(&mut self, run: &ShapedRun) {
        self.backend.draw_glyphs(run, &self.state.fill);
    }

    pub fn draw_image(&mut self, image: &ImageRef, src_rect: Rect, dst_rect: Rect) {
        self.backend.draw_image(image, src_rect, dst_rect);
    }
}
