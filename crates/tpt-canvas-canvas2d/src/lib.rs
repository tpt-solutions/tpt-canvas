//! Canvas2D backend for tpt-canvas.
//!
//! Each call maps onto a `CanvasRenderingContext2D`. Transform and clip pushes
//! become `save()` plus the operation, and pops become `restore()`, so the browser's
//! own state stack stays balanced. The crate is empty on non-wasm targets.
//!
//! Limitations: canvas gradients support only pad spreading (repeat and reflect are
//! drawn as pad), per-stop alpha is applied through the color string, and pattern
//! paints, glyphs and images are not drawn yet.
#![cfg(target_arch = "wasm32")]

use tpt_canvas::path::{Path, PathCommand};
use tpt_canvas::{
    Affine2, CanvasBackend, Color, FillRule, ImageRef, LineCap, LineJoin, Paint, Rect, StrokeStyle,
};
use tpt_text_stub::ShapedRun;
use wasm_bindgen::JsValue;
use web_sys::{CanvasGradient, CanvasRenderingContext2d, CanvasWindingRule, Path2d};

pub struct Canvas2dBackend {
    ctx: CanvasRenderingContext2d,
}

/// A paint resolved into something the context can take directly.
enum Style {
    Color(String),
    Gradient(CanvasGradient),
}

impl Canvas2dBackend {
    pub fn new(ctx: CanvasRenderingContext2d) -> Self {
        Canvas2dBackend { ctx }
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
    fn fill_path(&mut self, path: &Path, paint: &Paint, rule: FillRule) {
        let (Some(p2), Some(style)) = (Self::path2d(path), self.style(paint)) else {
            return;
        };
        let winding = match rule {
            FillRule::NonZero => CanvasWindingRule::Nonzero,
            FillRule::EvenOdd => CanvasWindingRule::Evenodd,
        };
        self.set_fill(&style);
        let _ = self.ctx.fill_with_path_2d_and_winding(&p2, winding);
    }

    fn stroke_path(&mut self, path: &Path, paint: &Paint, style: &StrokeStyle) {
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
        let _ = self.ctx.stroke_with_path(&p2);
    }

    fn draw_glyphs(&mut self, _run: &ShapedRun, _paint: &Paint) {}

    fn draw_image(&mut self, _image: &ImageRef, _src_rect: Rect, _dst_rect: Rect) {}

    fn push_clip(&mut self, path: &Path) {
        self.ctx.save();
        if let Some(p2) = Self::path2d(path) {
            self.ctx.clip_with_path_2d(&p2);
        }
    }

    fn pop_clip(&mut self) {
        let _ = self.ctx.restore();
    }

    fn push_transform(&mut self, t: &Affine2) {
        self.ctx.save();
        let _ = self.ctx.transform(
            t.a as f64, t.b as f64, t.c as f64, t.d as f64, t.e as f64, t.f as f64,
        );
    }

    fn pop_transform(&mut self) {
        let _ = self.ctx.restore();
    }
}
