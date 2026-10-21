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
use wasm_bindgen::JsValue;
use web_sys::{CanvasGradient, CanvasRenderingContext2d, CanvasWindingRule, Path2d};

/// A `CanvasBackend` that draws into a browser canvas context.
pub struct Canvas2dBackend {
    ctx: CanvasRenderingContext2d,
    unsupported: u32,
    blend: BlendMode,
}

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
        }
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

impl CanvasBackend for Canvas2dBackend {
    fn set_blend_mode(&mut self, mode: BlendMode) {
        self.blend = mode;
        let _ = self
            .ctx
            .set_global_composite_operation(composite_name(mode));
    }

    fn fill_path(&mut self, path: &Path, paint: &Paint, rule: FillRule) {
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
        self.ctx.fill_with_path_2d_and_winding(&p2, winding);
    }

    fn push_layer(&mut self, _alpha: f32) {
        // Group compositing needs an offscreen canvas, which is not wired up yet.
        self.unsupported += 1;
    }

    fn pop_layer(&mut self) {}

    fn fill_path_blurred(&mut self, _path: &Path, _paint: &Paint, _rule: FillRule, _sigma: f32) {
        // Canvas2D can blur with `filter`, but this adapter does not use it yet. Counted until then.
        self.unsupported += 1;
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
        if matches!(paint, Paint::Pattern { .. }) {
            self.unsupported += 1;
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

    fn draw_image(&mut self, _image: &ImageRef, _src_rect: Rect, _dst_rect: Rect) {
        self.unsupported += 1;
    }

    fn push_clip(&mut self, path: &Path) {
        self.ctx.save();
        if let Some(p2) = Self::path2d(path) {
            self.ctx.clip_with_path_2d(&p2);
        }
    }

    fn pop_clip(&mut self) {
        self.ctx.restore();
    }

    fn push_transform(&mut self, t: &Affine2) {
        self.ctx.save();
        let _ = self.ctx.transform(
            t.a as f64, t.b as f64, t.c as f64, t.d as f64, t.e as f64, t.f as f64,
        );
    }

    fn pop_transform(&mut self) {
        self.ctx.restore();
    }
}

/// The `globalCompositeOperation` value for a blend mode. Canvas calls normal blending `source-over`.
fn composite_name(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "source-over",
        other => other.css_name(),
    }
}
