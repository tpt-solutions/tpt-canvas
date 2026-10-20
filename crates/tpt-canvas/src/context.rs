//! Drawing state and the immediate-mode API (Module C).
//!
//! `Context` tracks the current transform, default styles and clip depth, and
//! forwards every call to a [`CanvasBackend`]. `save`/`restore` mirror the state
//! stack: restoring pops the transforms and clips pushed since the matching save,
//! so the backend's stacks always stay balanced.

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::backend::CanvasBackend;
use crate::image::ImageRef;
use crate::path::Path;
use crate::style::{FillRule, Paint, StrokeStyle};
use crate::text::GlyphSource;
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

/// Drawing state and the immediate-mode API. Forwards calls to a backend, tracking transforms and clips so `restore` can undo them.
pub struct Context<B: CanvasBackend> {
    backend: B,
    state: State,
    stack: Vec<State>,
    glyphs: Option<Box<dyn GlyphSource>>,
}

impl<B: CanvasBackend> Context<B> {
    /// Creates a context with the default state (identity transform, black fill and stroke).
    pub fn new(backend: B) -> Self {
        Context {
            backend,
            state: State::default(),
            stack: Vec::new(),
            glyphs: None,
        }
    }

    /// Returns the wrapped backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Consumes the context and returns the wrapped backend.
    pub fn into_backend(self) -> B {
        self.backend
    }

    /// Current user-to-device-independent transform (the product of all pushes).
    pub fn current_transform(&self) -> Affine2 {
        self.state.ctm
    }

    /// Pushes the drawing state. Pair with `restore`.
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

    /// Translates the user space by `(tx, ty)`.
    pub fn translate(&mut self, tx: f32, ty: f32) {
        self.transform(Affine2::translate(tx, ty));
    }

    /// Rotates the user space by `theta` radians.
    pub fn rotate(&mut self, theta: f32) {
        self.transform(Affine2::rotate(theta));
    }

    /// Scales the user space by `(sx, sy)`.
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

    /// Sets the paint used by `fill_path` and `draw_glyphs`.
    pub fn set_fill_paint(&mut self, paint: Paint) {
        self.state.fill = paint;
    }

    /// Sets the paint used by `stroke_path`.
    pub fn set_stroke_paint(&mut self, paint: Paint) {
        self.state.stroke = paint;
    }

    /// Sets the winding rule used by `fill_path`.
    pub fn set_fill_rule(&mut self, rule: FillRule) {
        self.state.fill_rule = rule;
    }

    /// Sets the stroke geometry used by `stroke_path`.
    pub fn set_stroke_style(&mut self, style: StrokeStyle) {
        self.state.stroke_style = style;
    }

    /// Fills `path` with the current fill paint and rule.
    pub fn fill_path(&mut self, path: &Path) {
        self.backend
            .fill_path(path, &self.state.fill, self.state.fill_rule);
    }

    /// Strokes `path` with the current stroke paint and style.
    pub fn stroke_path(&mut self, path: &Path) {
        self.backend
            .stroke_path(path, &self.state.stroke, &self.state.stroke_style);
    }

    /// Draws `run` with the current fill paint.
    /// Draws a glyph run with the current fill paint.
    ///
    /// With a [`GlyphSource`] set, each glyph is filled as a path, so every backend draws
    /// it. Without one, the run is passed to the backend's own `draw_glyphs`.
    pub fn draw_glyphs(&mut self, run: &ShapedRun) {
        let Some(source) = self.glyphs.as_deref() else {
            self.backend.draw_glyphs(run, &self.state.fill);
            return;
        };
        let Some(upem) = source.units_per_em(run.font_id).filter(|u| *u > 0.0) else {
            return;
        };
        let scale = run.font_size / upem;
        let mut outlines = Vec::with_capacity(run.glyphs.len());
        for g in &run.glyphs {
            if let Some(outline) = source.outline(run.font_id, g.id) {
                // Font units are y-up; the canvas is y-down, so flip y while scaling.
                let m = Affine2::translate(g.pos.x, g.pos.y) * Affine2::scale(scale, -scale);
                outlines.push((m, outline));
            }
        }
        for (m, outline) in outlines {
            self.save();
            self.transform(m);
            self.fill_path(&outline);
            self.restore();
        }
    }

    /// Sets the source used by [`draw_glyphs`](Self::draw_glyphs) to turn glyphs into paths.
    pub fn set_glyph_source(&mut self, source: Box<dyn GlyphSource>) {
        self.glyphs = Some(source);
    }

    /// Draws part of an image into a destination rectangle.
    pub fn draw_image(&mut self, image: &ImageRef, src_rect: Rect, dst_rect: Rect) {
        self.backend.draw_image(image, src_rect, dst_rect);
    }
}
