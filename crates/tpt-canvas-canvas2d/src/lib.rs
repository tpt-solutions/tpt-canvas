//! Canvas2D backend for tpt-canvas.
//!
//! Each call maps onto a `CanvasRenderingContext2D`. Transform and clip pushes
//! become `save()` plus the operation, and pops become `restore()`, so the browser's
//! own state stack stays balanced. The crate is empty on non-wasm targets.
//!
//! Limitations: canvas gradients support only pad spreading. Repeat and reflect are
//! drawn as pad and counted by `Canvas2dBackend::unsupported`. Pattern paints,
//! glyphs and images are not drawn yet, and are counted too.
#![warn(missing_docs)]
#![cfg(target_arch = "wasm32")]

use tpt_canvas::path::{Path, PathCommand};
use tpt_canvas::text::ShapedRun;
use tpt_canvas::{
    Affine2, BlendMode, CanvasBackend, Color, FillRule, ImageRef, LineCap, LineJoin, Paint, Rect,
    SpreadMode, StrokeStyle,
};
use wasm_bindgen::{Clamped, JsCast, JsValue};
use web_sys::{
    CanvasGradient, CanvasPattern, CanvasRenderingContext2d, CanvasWindingRule, HtmlCanvasElement,
    ImageData, Path2d,
};

/// A `CanvasBackend` that draws into a browser canvas context.
pub struct Canvas2dBackend {
    ctx: CanvasRenderingContext2d,
    unsupported: u32,
    blend: BlendMode,
    /// Registered images: an offscreen canvas holding each one, with its size.
    images: Vec<(tpt_canvas::ImageId, HtmlCanvasElement, u32, u32)>,
    /// Canvas patterns made from registered images, made on first use.
    patterns: Vec<(tpt_canvas::ImageId, CanvasPattern)>,
    /// Clips and transforms in force, oldest first. A layer replays them onto its canvas.
    state: Vec<StateEntry>,
    /// Open layers, innermost last.
    layers: Vec<LayerFrame>,
}

/// A clip or transform that is in force, recorded so a layer can replay it.
#[derive(Clone)]
enum StateEntry {
    /// A clip. Its path is not kept: clips are re-applied when a layer is composited.
    Clip,
    Transform(Affine2),
}

/// An open layer: the context it will be composited onto, its canvas, and its alpha.
struct LayerFrame {
    parent: CanvasRenderingContext2d,
    canvas: HtmlCanvasElement,
    alpha: f32,
}

/// Flattening tolerance, in user units, for the outline of a stroke drawn with a pattern.
const STROKE_PATTERN_TOLERANCE: f32 = 0.05;

/// A paint resolved into something the context can take directly.
enum Style {
    Color(String),
    Gradient(CanvasGradient),
}

impl Canvas2dBackend {
    /// Wraps a 2D rendering context obtained from a canvas element.
    pub fn new(ctx: CanvasRenderingContext2d) -> Self {
        Canvas2dBackend {
            ctx,
            unsupported: 0,
            blend: BlendMode::Normal,
            images: Vec::new(),
            patterns: Vec::new(),
            state: Vec::new(),
            layers: Vec::new(),
        }
    }

    /// Registers an image under `id`. `rgba` is straight-alpha RGBA8, row by row, with
    /// `width * height * 4` bytes. The pixels are copied into an offscreen canvas once, so each
    /// `draw_image` is a plain `drawImage`. Replaces any image already registered under `id`.
    pub fn add_image(
        &mut self,
        id: tpt_canvas::ImageId,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<(), JsValue> {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or_else(|| JsValue::from_str("no document"))?;
        let canvas: HtmlCanvasElement = document.create_element("canvas")?.dyn_into()?;
        canvas.set_width(width);
        canvas.set_height(height);
        let ctx: CanvasRenderingContext2d = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("no 2d context"))?
            .dyn_into()?;
        let data = ImageData::new_with_u8_clamped_array_and_sh(Clamped(rgba), width, height)?;
        ctx.put_image_data(&data, 0.0, 0.0)?;
        self.patterns.retain(|(k, _)| *k != id);
        match self.images.iter_mut().find(|(k, ..)| *k == id) {
            Some(slot) => *slot = (id, canvas, width, height),
            None => self.images.push((id, canvas, width, height)),
        }
        Ok(())
    }

    /// Number of calls skipped because they are not supported yet (pattern paints,
    /// glyph runs and images).
    pub fn unsupported(&self) -> u32 {
        self.unsupported
    }

    fn path2d(path: &Path) -> Option<Path2d> {
        let p2 = Path2d::new().ok()?;
        for cmd in path.commands() {
            match *cmd {
                PathCommand::MoveTo(p) => p2.move_to(p.x as f64, p.y as f64),
                PathCommand::LineTo(p) => p2.line_to(p.x as f64, p.y as f64),
                PathCommand::QuadTo(c, p) => {
                    p2.quadratic_curve_to(c.x as f64, c.y as f64, p.x as f64, p.y as f64)
                }
                PathCommand::CubicTo(c1, c2, p) => p2.bezier_curve_to(
                    c1.x as f64,
                    c1.y as f64,
                    c2.x as f64,
                    c2.y as f64,
                    p.x as f64,
                    p.y as f64,
                ),
                PathCommand::Close => p2.close_path(),
            }
        }
        Some(p2)
    }

    /// Resolves `paint` for the context, or `None` when the paint is not drawable here.
    fn style(&self, paint: &Paint) -> Option<Style> {
        match paint {
            Paint::Solid(c) => Some(Style::Color(css_color(*c))),
            Paint::LinearGradient {
                start, end, stops, ..
            } => {
                let g = self.ctx.create_linear_gradient(
                    start.x as f64,
                    start.y as f64,
                    end.x as f64,
                    end.y as f64,
                );
                for s in stops {
                    let _ = g.add_color_stop(s.offset, &css_color(s.color));
                }
                Some(Style::Gradient(g))
            }
            Paint::RadialGradient {
                center,
                radius,
                stops,
                ..
            } => {
                let g = self
                    .ctx
                    .create_radial_gradient(
                        center.x as f64,
                        center.y as f64,
                        0.0,
                        center.x as f64,
                        center.y as f64,
                        *radius as f64,
                    )
                    .ok()?;
                for s in stops {
                    let _ = g.add_color_stop(s.offset, &css_color(s.color));
                }
                Some(Style::Gradient(g))
            }
            Paint::Pattern { .. } => None,
        }
    }

    fn set_fill(&self, style: &Style) {
        match style {
            Style::Color(c) => self.ctx.set_fill_style_str(c),
            Style::Gradient(g) => self.ctx.set_fill_style_canvas_gradient(g),
        }
    }

    fn set_stroke(&self, style: &Style) {
        match style {
            Style::Color(c) => self.ctx.set_stroke_style_str(c),
            Style::Gradient(g) => self.ctx.set_stroke_style_canvas_gradient(g),
        }
    }
}

/// Whether `paint` is a gradient with a spread that canvas cannot draw (anything but `Pad`).
fn non_pad_spread(paint: &Paint) -> bool {
    match paint {
        Paint::LinearGradient { spread, .. } | Paint::RadialGradient { spread, .. } => {
            *spread != SpreadMode::Pad
        }
        _ => false,
    }
}

fn css_color(c: Color) -> String {
    let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    format!(
        "rgba({},{},{},{})",
        to8(c.r),
        to8(c.g),
        to8(c.b),
        c.a.clamp(0.0, 1.0)
    )
}

impl Canvas2dBackend {
    /// The canvas pattern for a registered image, repeating in image pixels. Made on first use.
    fn pattern_for(&mut self, id: tpt_canvas::ImageId) -> Option<CanvasPattern> {
        if let Some((_, pattern)) = self.patterns.iter().find(|(k, _)| *k == id) {
            return Some(pattern.clone());
        }
        let canvas = self.images.iter().find(|(k, ..)| *k == id)?.1.clone();
        let pattern = self
            .ctx
            .create_pattern_with_html_canvas_element(&canvas, "repeat")
            .ok()??;
        self.patterns.push((id, pattern.clone()));
        Some(pattern)
    }

    /// Fills `path` (in user space) with a pattern. The fill runs in image space: the path is
    /// mapped by the inverse of `transform`, and the context is transformed by `transform`
    /// while it is filled. Returns false when the image is missing or the transform has no
    /// inverse.
    fn fill_pattern(
        &mut self,
        path: &Path,
        id: tpt_canvas::ImageId,
        transform: &Affine2,
        rule: FillRule,
    ) -> bool {
        let Some(pattern) = self.pattern_for(id) else {
            return false;
        };
        let Some(inverse) = transform.invert() else {
            return false;
        };
        let Some(p2) = Self::path2d(&path.transform(&inverse)) else {
            return false;
        };
        let winding = match rule {
            FillRule::NonZero => CanvasWindingRule::Nonzero,
            FillRule::EvenOdd => CanvasWindingRule::Evenodd,
        };
        self.ctx.save();
        self.ctx
            .transform(
                transform.a as f64,
                transform.b as f64,
                transform.c as f64,
                transform.d as f64,
                transform.e as f64,
                transform.f as f64,
            )
            .ok();
        // Nearest-neighbour sampling, as the raster backend does.
        self.ctx.set_image_smoothing_enabled(false);
        self.ctx.set_fill_style_canvas_pattern(&pattern);
        self.ctx.fill_with_path_2d_and_winding(&p2, winding);
        self.ctx.restore();
        true
    }
}

impl CanvasBackend for Canvas2dBackend {
    fn set_blend_mode(&mut self, mode: BlendMode) {
        self.blend = mode;
        let _ = self
            .ctx
            .set_global_composite_operation(composite_name(mode));
    }

    fn fill_path(&mut self, path: &Path, paint: &Paint, rule: FillRule) {
        if let Paint::Pattern { image, transform } = paint {
            if !self.fill_pattern(path, *image, transform, rule) {
                self.unsupported += 1;
            }
            return;
        }
        if non_pad_spread(paint) {
            self.unsupported += 1;
        }
        let (Some(p2), Some(style)) = (Self::path2d(path), self.style(paint)) else {
            return;
        };
        let winding = match rule {
            FillRule::NonZero => CanvasWindingRule::Nonzero,
            FillRule::EvenOdd => CanvasWindingRule::Evenodd,
        };
        self.set_fill(&style);
        self.ctx.fill_with_path_2d_and_winding(&p2, winding);
    }

    fn push_layer(&mut self, alpha: f32) {
        let (width, height) = match self.ctx.canvas() {
            Some(c) => (c.width(), c.height()),
            None => {
                self.unsupported += 1;
                return;
            }
        };
        let Ok((canvas, ctx)) = offscreen(width, height) else {
            self.unsupported += 1;
            return;
        };
        // The layer starts with the transforms in force, because its drawing is in those
        // coordinates. The clips in force are not replayed: the layer is composited under the
        // same clips, so they still apply to what it draws.
        for entry in &self.state {
            if let StateEntry::Transform(t) = entry {
                ctx.save();
                let _ = ctx.transform(
                    t.a as f64, t.b as f64, t.c as f64, t.d as f64, t.e as f64, t.f as f64,
                );
            }
        }
        self.layers.push(LayerFrame {
            parent: self.ctx.clone(),
            canvas,
            alpha: alpha.clamp(0.0, 1.0),
        });
        self.ctx = ctx;
    }

    fn pop_layer(&mut self) {
        let Some(frame) = self.layers.pop() else {
            return;
        };
        self.ctx = frame.parent;
        // Composite with the identity transform, so the layer lands where it was drawn.
        // The parent's clip is still in force, which is what a group should respect.
        self.ctx.save();
        let _ = self.ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        self.ctx.set_global_alpha(frame.alpha as f64);
        let _ = self
            .ctx
            .set_global_composite_operation(composite_name(self.blend));
        let _ = self
            .ctx
            .draw_image_with_html_canvas_element(&frame.canvas, 0.0, 0.0);
        self.ctx.restore();
    }

    fn fill_path_blurred(&mut self, path: &Path, paint: &Paint, rule: FillRule, sigma: f32) {
        if !sigma.is_finite() || sigma <= 0.0 {
            return self.fill_path(path, paint, rule);
        }
        if matches!(paint, Paint::Pattern { .. }) {
            self.unsupported += 1;
            return;
        }
        if non_pad_spread(paint) {
            self.unsupported += 1;
        }
        let (Some(p2), Some(style)) = (Self::path2d(path), self.style(paint)) else {
            return;
        };
        let winding = match rule {
            FillRule::NonZero => CanvasWindingRule::Nonzero,
            FillRule::EvenOdd => CanvasWindingRule::Evenodd,
        };
        self.set_fill(&style);
        // A CSS blur radius is a standard deviation, the same convention as the other backends.
        self.ctx.set_filter(&format!("blur({sigma}px)"));
        self.ctx.fill_with_path_2d_and_winding(&p2, winding);
        self.ctx.set_filter("none");
    }

    fn clear_path(&mut self, path: &Path) {
        let Some(p2) = Self::path2d(path) else {
            return;
        };
        // `destination-out` with opaque black removes whatever is underneath, which is
        // what canvas clearRect does. The composite is put back straight away so the
        // next fill blends as usual.
        let _ = self.ctx.set_global_composite_operation("destination-out");
        self.ctx.set_fill_style_str("rgba(0,0,0,1)");
        self.ctx.fill_with_path_2d(&p2);
        let _ = self
            .ctx
            .set_global_composite_operation(composite_name(self.blend));
    }

    fn stroke_path(&mut self, path: &Path, paint: &Paint, style: &StrokeStyle) {
        if let Paint::Pattern { image, transform } = paint {
            // Stroke the outline as polygons in user space, then fill those with the pattern.
            let outline = tpt_canvas::stroke::stroke_outline(
                &path.flatten(STROKE_PATTERN_TOLERANCE),
                style,
                STROKE_PATTERN_TOLERANCE,
            );
            let mut builder = tpt_canvas::PathBuilder::new();
            for poly in outline.iter().filter(|p| p.len() >= 3) {
                builder.move_to(poly[0].x, poly[0].y);
                for p in &poly[1..] {
                    builder.line_to(p.x, p.y);
                }
                builder.close();
            }
            if !self.fill_pattern(&builder.build(), *image, transform, FillRule::NonZero) {
                self.unsupported += 1;
            }
            return;
        }
        if non_pad_spread(paint) {
            self.unsupported += 1;
        }
        let (Some(p2), Some(resolved)) = (Self::path2d(path), self.style(paint)) else {
            return;
        };
        self.set_stroke(&resolved);
        self.ctx.set_line_width(style.width as f64);
        self.ctx.set_miter_limit(style.miter_limit as f64);
        self.ctx.set_line_cap(match style.cap {
            LineCap::Butt => "butt",
            LineCap::Round => "round",
            LineCap::Square => "square",
        });
        self.ctx.set_line_join(match style.join {
            LineJoin::Miter => "miter",
            LineJoin::Round => "round",
            LineJoin::Bevel => "bevel",
        });
        let dashes = js_sys::Array::new();
        for d in &style.dash {
            dashes.push(&JsValue::from_f64(*d as f64));
        }
        let _ = self.ctx.set_line_dash(&dashes);
        self.ctx.set_line_dash_offset(style.dash_offset as f64);
        self.ctx.stroke_with_path(&p2);
    }

    fn draw_glyphs(&mut self, _run: &ShapedRun, _paint: &Paint) {
        self.unsupported += 1;
    }

    fn draw_image(&mut self, image: &ImageRef, src_rect: Rect, dst_rect: Rect) {
        if dst_rect.w == 0.0 || dst_rect.h == 0.0 {
            return;
        }
        let Some((_, canvas, width, height)) = self.images.iter().find(|(k, ..)| *k == image.id)
        else {
            self.unsupported += 1;
            return;
        };
        // The source rectangle is snapped outwards to whole pixels, as in the other backends.
        let (pw, ph) = (*width as f64, *height as f64);
        let sx = (src_rect.x as f64).floor().clamp(0.0, pw);
        let sy = (src_rect.y as f64).floor().clamp(0.0, ph);
        let sx1 = ((src_rect.x + src_rect.w) as f64).ceil().clamp(0.0, pw);
        let sy1 = ((src_rect.y + src_rect.h) as f64).ceil().clamp(0.0, ph);
        if sx1 <= sx || sy1 <= sy {
            return;
        }
        // Nearest-neighbour sampling, as the raster backend does.
        self.ctx.set_image_smoothing_enabled(false);
        let _ = self
            .ctx
            .draw_image_with_html_canvas_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                canvas,
                sx,
                sy,
                sx1 - sx,
                sy1 - sy,
                dst_rect.x as f64,
                dst_rect.y as f64,
                dst_rect.w as f64,
                dst_rect.h as f64,
            );
        self.ctx.set_image_smoothing_enabled(true);
    }

    fn push_clip(&mut self, path: &Path) {
        self.state.push(StateEntry::Clip);
        self.ctx.save();
        if let Some(p2) = Self::path2d(path) {
            self.ctx.clip_with_path_2d(&p2);
        }
    }

    fn pop_clip(&mut self) {
        self.state.pop();
        self.ctx.restore();
    }

    fn push_transform(&mut self, t: &Affine2) {
        self.state.push(StateEntry::Transform(*t));
        self.ctx.save();
        let _ = self.ctx.transform(
            t.a as f64, t.b as f64, t.c as f64, t.d as f64, t.e as f64, t.f as f64,
        );
    }

    fn pop_transform(&mut self) {
        self.state.pop();
        self.ctx.restore();
    }
}

/// A transparent offscreen canvas of the given size, with its 2D context.
fn offscreen(
    width: u32,
    height: u32,
) -> Result<(HtmlCanvasElement, CanvasRenderingContext2d), JsValue> {
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let canvas: HtmlCanvasElement = document.create_element("canvas")?.dyn_into()?;
    canvas.set_width(width);
    canvas.set_height(height);
    let ctx: CanvasRenderingContext2d = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("no 2d context"))?
        .dyn_into()?;
    Ok((canvas, ctx))
}

/// The `globalCompositeOperation` value for a blend mode. Canvas calls normal blending `source-over`.
fn composite_name(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "source-over",
        other => other.css_name(),
    }
}

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_examples {}
