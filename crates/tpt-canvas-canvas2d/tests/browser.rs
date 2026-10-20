//! Browser tests: draw through `Canvas2dBackend` and read pixels back with `getImageData`.
//!
//! Run with: `wasm-pack test --headless --chrome crates/tpt-canvas-canvas2d`

#![cfg(target_arch = "wasm32")]

use tpt_canvas::path::Path;
use tpt_canvas::text::{GlyphId, ShapedRun};
use tpt_canvas::{
    Affine2, Color, ColorSpace, Context, FillRule, GlyphSource, Paint, PathBuilder, SpreadMode,
    Stop, StrokeStyle, Vec2,
};
use tpt_canvas_canvas2d::Canvas2dBackend;
use wasm_bindgen::{Clamped, JsCast};
use wasm_bindgen_test::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

wasm_bindgen_test_configure!(run_in_browser);

const W: u32 = 64;
const H: u32 = 64;

/// A fresh canvas element attached to the page, returned with its 2D context.
fn canvas() -> CanvasRenderingContext2d {
    let document = web_sys::window().unwrap().document().unwrap();
    let el: HtmlCanvasElement = document
        .create_element("canvas")
        .unwrap()
        .dyn_into()
        .unwrap();
    el.set_width(W);
    el.set_height(H);
    document.body().unwrap().append_child(&el).unwrap();
    el.get_context("2d").unwrap().unwrap().dyn_into().unwrap()
}

/// RGBA of the pixel at `(x, y)`, read back from the canvas.
fn pixel(ctx: &CanvasRenderingContext2d, x: u32, y: u32) -> [u8; 4] {
    let data = ctx.get_image_data(x as f64, y as f64, 1.0, 1.0).unwrap();
    let px: Clamped<Vec<u8>> = data.data();
    [px[0], px[1], px[2], px[3]]
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut b = PathBuilder::new();
    b.rect(x, y, w, h);
    b.build()
}

fn red() -> Paint {
    Paint::Solid(Color::rgb(1.0, 0.0, 0.0))
}

#[wasm_bindgen_test]
fn solid_fill_paints_the_path() {
    let c = canvas();
    let mut ctx = Context::new(Canvas2dBackend::new(c.clone()));
    ctx.set_fill_paint(red());
    ctx.fill_path(&rect(8.0, 8.0, 20.0, 20.0));
    assert_eq!(pixel(&c, 15, 15), [255, 0, 0, 255]);
    assert_eq!(pixel(&c, 40, 40)[3], 0);
}

#[wasm_bindgen_test]
fn even_odd_leaves_the_hole_empty() {
    let c = canvas();
    let mut ctx = Context::new(Canvas2dBackend::new(c.clone()));
    let mut b = PathBuilder::new();
    b.rect(0.0, 0.0, 40.0, 40.0);
    b.rect(10.0, 10.0, 20.0, 20.0);
    ctx.set_fill_paint(red());
    ctx.set_fill_rule(FillRule::EvenOdd);
    ctx.fill_path(&b.build());
    assert_eq!(pixel(&c, 20, 20)[3], 0, "hole");
    assert_eq!(pixel(&c, 5, 5)[3], 255, "ring");
}

#[wasm_bindgen_test]
fn clip_limits_the_fill() {
    let c = canvas();
    let mut ctx = Context::new(Canvas2dBackend::new(c.clone()));
    ctx.set_fill_paint(red());
    ctx.save();
    ctx.clip(&rect(0.0, 0.0, 30.0, 64.0));
    ctx.fill_path(&rect(0.0, 0.0, 64.0, 64.0));
    ctx.restore();
    assert_eq!(pixel(&c, 10, 10)[3], 255);
    assert_eq!(pixel(&c, 50, 10)[3], 0);
}

#[wasm_bindgen_test]
fn transform_moves_the_fill() {
    let c = canvas();
    let mut ctx = Context::new(Canvas2dBackend::new(c.clone()));
    ctx.set_fill_paint(red());
    ctx.save();
    ctx.transform(Affine2::translate(30.0, 0.0));
    ctx.fill_path(&rect(0.0, 0.0, 10.0, 10.0));
    ctx.restore();
    assert_eq!(pixel(&c, 35, 5)[3], 255);
    assert_eq!(pixel(&c, 5, 5)[3], 0);
}

#[wasm_bindgen_test]
fn linear_gradient_runs_dark_to_light() {
    let c = canvas();
    let mut ctx = Context::new(Canvas2dBackend::new(c.clone()));
    ctx.set_fill_paint(Paint::LinearGradient {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(64.0, 0.0),
        stops: vec![
            Stop {
                offset: 0.0,
                color: Color::BLACK,
            },
            Stop {
                offset: 1.0,
                color: Color::WHITE,
            },
        ],
        spread: SpreadMode::Pad,
        space: ColorSpace::Srgb,
    });
    ctx.fill_path(&rect(0.0, 0.0, 64.0, 64.0));
    let left = pixel(&c, 2, 32)[0];
    let right = pixel(&c, 61, 32)[0];
    assert!(left < 40 && right > 215, "left {left} right {right}");
}

#[wasm_bindgen_test]
fn stroke_draws_a_line() {
    let c = canvas();
    let mut ctx = Context::new(Canvas2dBackend::new(c.clone()));
    let mut b = PathBuilder::new();
    b.move_to(4.0, 32.0);
    b.line_to(60.0, 32.0);
    ctx.set_stroke_paint(red());
    ctx.set_stroke_style(StrokeStyle {
        width: 4.0,
        ..StrokeStyle::default()
    });
    ctx.stroke_path(&b.build());
    assert_eq!(pixel(&c, 30, 32), [255, 0, 0, 255]);
    assert_eq!(pixel(&c, 30, 20)[3], 0);
}

/// One glyph: a square in a 1000-unit em.
struct SquareFont;

impl GlyphSource for SquareFont {
    fn units_per_em(&self, _font_id: u32) -> Option<f32> {
        Some(1000.0)
    }

    fn outline(&self, _font_id: u32, glyph: GlyphId) -> Option<Path> {
        (glyph == 1).then(|| rect(0.0, 0.0, 1000.0, 1000.0))
    }
}

#[wasm_bindgen_test]
fn glyph_run_is_drawn_through_the_outline_seam() {
    let c = canvas();
    let mut ctx = Context::new(Canvas2dBackend::new(c.clone()));
    ctx.set_glyph_source(Box::new(SquareFont));
    ctx.set_fill_paint(red());
    let mut run = ShapedRun::new(1, 20.0);
    run.push(1, Vec2::new(10.0, 40.0));
    ctx.draw_glyphs(&run);
    // The square covers x 10..30 and y 20..40, flipped about the baseline.
    assert_eq!(pixel(&c, 15, 30), [255, 0, 0, 255]);
    assert_eq!(pixel(&c, 15, 45)[3], 0);
}

#[wasm_bindgen_test]
fn clear_path_makes_pixels_transparent_and_leaves_the_rest() {
    let c = canvas();
    let mut ctx = Context::new(Canvas2dBackend::new(c.clone()));
    ctx.set_fill_paint(red());
    ctx.fill_path(&rect(0.0, 0.0, 64.0, 64.0));
    ctx.clear_rect(8.0, 8.0, 20.0, 20.0);
    assert_eq!(pixel(&c, 15, 15), [0, 0, 0, 0]);
    assert_eq!(pixel(&c, 40, 40), [255, 0, 0, 255]);
}
