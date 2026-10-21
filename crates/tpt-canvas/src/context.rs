//! Drawing state and the immediate-mode API (Module C).
//!
//! `Context` tracks the current transform, default styles and clip depth, and
//! forwards every call to a [`CanvasBackend`]. `save`/`restore` mirror the state
//! stack: restoring pops the transforms and clips pushed since the matching save,
//! so the backend's stacks always stay balanced.

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::backend::CanvasBackend;
use crate::image::ImageRef;
use crate::path::{Path, PathBuilder};
use crate::stroke::stroke_outline;
use crate::style::{BlendMode, FillRule, Paint, Shadow, StrokeStyle};
use crate::text::GlyphSource;
use crate::text::ShapedRun;
use crate::{Affine2, Color, Rect, Vec2};

/// Flattening tolerance, in user units, for blurred stroke shadows. The blur hides fine detail.
const STROKE_SHADOW_TOLERANCE: f32 = 0.05;

/// A backend stack push, recorded so `restore` can undo pushes in the order they happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpKind {
    Clip,
    Transform,
    Layer,
}

#[derive(Clone, Debug)]
struct State {
    ctm: Affine2,
    fill: Paint,
    stroke: Paint,
    fill_rule: FillRule,
    stroke_style: StrokeStyle,
    global_alpha: f32,
    shadow: Option<Shadow>,
    blend: BlendMode,
    /// Backend pushes made at this save level, in the order they were made.
    ops: Vec<OpKind>,
}

impl Default for State {
    fn default() -> Self {
        State {
            ctm: Affine2::IDENTITY,
            fill: Paint::Solid(Color::BLACK),
            stroke: Paint::Solid(Color::BLACK),
            fill_rule: FillRule::NonZero,
            stroke_style: StrokeStyle::default(),
            global_alpha: 1.0,
            shadow: None,
            blend: BlendMode::Normal,
            ops: Vec::new(),
        }
    }
}

/// The paint to hand a backend: `paint` itself when `alpha` is 1, otherwise a copy with its alpha scaled.
fn faded(paint: &Paint, alpha: f32) -> Cow<'_, Paint> {
    if alpha >= 1.0 {
        Cow::Borrowed(paint)
    } else {
        Cow::Owned(paint.with_opacity(alpha))
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
        next.ops = Vec::new();
        let outer = core::mem::replace(&mut self.state, next);
        self.stack.push(outer);
    }

    /// Undoes everything since the matching `save`. Extra restores are ignored.
    pub fn restore(&mut self) {
        let Some(outer) = self.stack.pop() else {
            return;
        };
        // Undo in reverse push order so the backend's clip and transform stacks
        // unwind exactly as they were built.
        while let Some(op) = self.state.ops.pop() {
            match op {
                OpKind::Clip => self.backend.pop_clip(),
                OpKind::Transform => self.backend.pop_transform(),
                OpKind::Layer => self.backend.pop_layer(),
            }
        }
        let inner_blend = self.state.blend;
        self.state = outer;
        // Only tell the backend when the mode changes, so unchanged state adds no calls.
        if self.state.blend != inner_blend {
            self.backend.set_blend_mode(self.state.blend);
        }
    }

    /// Concatenates `m` onto the current transform.
    pub fn transform(&mut self, m: Affine2) {
        self.state.ctm = self.state.ctm * m;
        self.backend.push_transform(&m);
        self.state.ops.push(OpKind::Transform);
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
        self.state.ops.push(OpKind::Transform);
    }

    /// Intersects the current clip with `path`, in the current user space.
    pub fn clip(&mut self, path: &Path) {
        self.backend.push_clip(path);
        self.state.ops.push(OpKind::Clip);
    }

    /// Starts a group: everything drawn until the matching [`pop_layer`](Self::pop_layer)
    /// is composited as one unit with `alpha` (clamped to `0..=1`). Unlike global alpha,
    /// overlapping shapes inside the group are not double-faded. `restore` also closes it.
    pub fn push_layer(&mut self, alpha: f32) {
        let alpha = if alpha.is_finite() {
            alpha.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.backend.push_layer(alpha);
        self.state.ops.push(OpKind::Layer);
    }

    /// Ends the most recent group. It only closes a group that is the latest thing pushed at
    /// this save level, so close it before pushing clips or transforms that come after it.
    /// Otherwise nothing happens; `restore` still closes it.
    pub fn pop_layer(&mut self) {
        if self.state.ops.last() == Some(&OpKind::Layer) {
            self.state.ops.pop();
            self.backend.pop_layer();
        }
    }

    /// Sets the global alpha: a factor on the alpha of every fill, stroke and glyph
    /// drawn after it. Clamped to `0..=1`. Non-finite values are ignored.
    ///
    /// Images are not drawn yet, so this does not affect them. Like the other state,
    /// it is restored by `restore`.
    pub fn set_global_alpha(&mut self, alpha: f32) {
        if alpha.is_finite() {
            self.state.global_alpha = alpha.clamp(0.0, 1.0);
        }
    }

    /// The current global alpha.
    pub fn global_alpha(&self) -> f32 {
        self.state.global_alpha
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

    /// Sets the drop shadow used by fills. It is saved and restored with the other state.
    /// Strokes get the same shadow, blurred through their outline.
    pub fn set_shadow(&mut self, shadow: Shadow) {
        self.state.shadow = Some(shadow);
    }

    /// Sets how later fills and strokes combine with the canvas. Restored by `restore`.
    /// Group layers and `clear_rect` ignore it.
    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        if self.state.blend != mode {
            self.state.blend = mode;
            self.backend.set_blend_mode(mode);
        }
    }

    /// The current blend mode.
    pub fn blend_mode(&self) -> BlendMode {
        self.state.blend
    }

    /// Removes the drop shadow.
    pub fn clear_shadow(&mut self) {
        self.state.shadow = None;
    }

    /// Fills `path` with the current fill paint and rule. A shadow, if set, is drawn first.
    pub fn fill_path(&mut self, path: &Path) {
        if let Some(shadow) = self.state.shadow {
            self.draw_shadow(path, shadow);
        }
        let paint = faded(&self.state.fill, self.state.global_alpha);
        self.backend.fill_path(path, &paint, self.state.fill_rule);
    }

    /// Draws the shadow of a stroke. Unblurred, it is the stroke moved by the offset. Blurred, the
    /// stroke outline is flattened to polygons at `STROKE_SHADOW_TOLERANCE` and blurred as a fill.
    fn draw_stroke_shadow(&mut self, path: &Path, shadow: Shadow) {
        if shadow.color.a <= 0.0 || (shadow.offset == Vec2::ZERO && shadow.blur <= 0.0) {
            return;
        }
        let moved = path.transform(&Affine2::translate(shadow.offset.x, shadow.offset.y));
        let color = Paint::Solid(shadow.color);
        let paint = faded(&color, self.state.global_alpha);
        let style = self.state.stroke_style.clone();
        if shadow.blur > 0.0 {
            let polygons = stroke_outline(
                &moved.flatten(STROKE_SHADOW_TOLERANCE),
                &style,
                STROKE_SHADOW_TOLERANCE,
            );
            let mut outline = PathBuilder::new();
            for poly in polygons.iter().filter(|p| p.len() >= 3) {
                outline.move_to(poly[0].x, poly[0].y);
                for p in &poly[1..] {
                    outline.line_to(p.x, p.y);
                }
                outline.close();
            }
            self.backend.fill_path_blurred(
                &outline.build(),
                &paint,
                FillRule::NonZero,
                shadow.blur / 2.0,
            );
        } else {
            self.backend.stroke_path(&moved, &paint, &style);
        }
    }

    /// Draws the shadow of a fill: the path moved by the offset, painted in the shadow color.
    fn draw_shadow(&mut self, path: &Path, shadow: Shadow) {
        if shadow.color.a <= 0.0 || (shadow.offset == Vec2::ZERO && shadow.blur <= 0.0) {
            return;
        }
        let moved = path.transform(&Affine2::translate(shadow.offset.x, shadow.offset.y));
        let color = Paint::Solid(shadow.color);
        let paint = faded(&color, self.state.global_alpha);
        let rule = self.state.fill_rule;
        if shadow.blur > 0.0 {
            // The canvas blur size is twice the Gaussian standard deviation.
            self.backend
                .fill_path_blurred(&moved, &paint, rule, shadow.blur / 2.0);
        } else {
            self.backend.fill_path(&moved, &paint, rule);
        }
    }

    /// Strokes `path` with the current stroke paint and style. A shadow, if set, is drawn first.
    pub fn stroke_path(&mut self, path: &Path) {
        if let Some(shadow) = self.state.shadow {
            self.draw_stroke_shadow(path, shadow);
        }
        let paint = faded(&self.state.stroke, self.state.global_alpha);
        self.backend
            .stroke_path(path, &paint, &self.state.stroke_style);
    }

    /// Draws a glyph run with the current fill paint.
    ///
    /// With a [`GlyphSource`] set, each glyph is filled as a path, so every backend draws
    /// it. Without one, the run is passed to the backend's own `draw_glyphs`.
    pub fn draw_glyphs(&mut self, run: &ShapedRun) {
        let Some(source) = self.glyphs.as_deref() else {
            let paint = faded(&self.state.fill, self.state.global_alpha);
            self.backend.draw_glyphs(run, &paint);
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

    /// Fills the rectangle `(x, y, w, h)` with the current fill paint.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let mut b = PathBuilder::new();
        b.rect(x, y, w, h);
        self.fill_path(&b.build());
    }

    /// Sets the rectangle `(x, y, w, h)` back to transparent. Like canvas `clearRect`, the
    /// fill paint and global alpha do not apply, but the transform and clip do.
    pub fn clear_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let mut b = PathBuilder::new();
        b.rect(x, y, w, h);
        self.backend.clear_path(&b.build());
    }

    /// Strokes the rectangle `(x, y, w, h)` with the current stroke paint and style.
    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let mut b = PathBuilder::new();
        b.rect(x, y, w, h);
        self.stroke_path(&b.build());
    }

    /// Fills the circle at `(cx, cy)` with radius `r` with the current fill paint.
    pub fn fill_circle(&mut self, cx: f32, cy: f32, r: f32) {
        let mut b = PathBuilder::new();
        b.circle(cx, cy, r);
        self.fill_path(&b.build());
    }

    /// Draws part of an image into a destination rectangle.
    pub fn draw_image(&mut self, image: &ImageRef, src_rect: Rect, dst_rect: Rect) {
        self.backend.draw_image(image, src_rect, dst_rect);
    }
}
