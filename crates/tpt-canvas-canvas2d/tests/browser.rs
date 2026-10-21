//! Browser tests: draw through `Canvas2dBackend` and read pixels back with `getImageData`.
//!
//! Run with: `wasm-pack test --headless --chrome crates/tpt-canvas-canvas2d`

#![cfg(target_arch = "wasm32")]

use tpt_canvas::path::Path;
use tpt_canvas::text::{GlyphId, ShapedRun};
use tpt_canvas::CanvasBackend;
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

#[wasm_bindgen_test]
fn multiply_blend_darkens_the_backdrop() {
    let c = canvas();
    let mut ctx = Context::new(Canvas2dBackend::new(c.clone()));
    ctx.set_fill_paint(red());
    ctx.fill_rect(0.0, 0.0, 64.0, 64.0);
    ctx.set_blend_mode(tpt_canvas::BlendMode::Multiply);
    ctx.set_fill_paint(Paint::Solid(Color::rgb(0.5, 0.5, 0.5)));
    ctx.fill_rect(8.0, 8.0, 20.0, 20.0);
    // Red times grey: the green and blue channels drop to zero, red halves.
    let p = pixel(&c, 15, 15);
    assert!((p[0] as i32 - 128).abs() <= 2, "red was {}", p[0]);
    assert_eq!(p[1], 0);
    assert_eq!(p[2], 0);
}

#[wasm_bindgen_test]
fn blend_mode_survives_clear_rect() {
    // `clear_rect` switches the composite to destination-out and back. The fill after it
    // must still multiply, not fall back to source-over.
    let c = canvas();
    let mut ctx = Context::new(Canvas2dBackend::new(c.clone()));
    ctx.set_fill_paint(red());
    ctx.fill_rect(0.0, 0.0, 64.0, 64.0);
    ctx.set_blend_mode(tpt_canvas::BlendMode::Multiply);
    ctx.clear_rect(0.0, 0.0, 4.0, 4.0);
    ctx.set_fill_paint(Paint::Solid(Color::rgb(0.5, 0.5, 0.5)));
    ctx.fill_rect(40.0, 40.0, 20.0, 20.0);
    let p = pixel(&c, 50, 50);
    assert!((p[0] as i32 - 128).abs() <= 2, "red was {}", p[0]);
    assert_eq!(p[1], 0, "green should be multiplied away, not greyed in");
}

#[wasm_bindgen_test]
fn blurred_fill_spreads_past_the_edge_and_fades() {
    // `fill_path_blurred` is what a blurred shadow uses. Sigma 3 spreads a 24-pixel square.
    let c = canvas();
    let mut backend = Canvas2dBackend::new(c.clone());
    backend.fill_path_blurred(
        &rect(20.0, 20.0, 24.0, 24.0),
        &red(),
        FillRule::NonZero,
        3.0,
    );
    assert_eq!(pixel(&c, 32, 32), [255, 0, 0, 255], "centre is opaque");
    let edge = pixel(&c, 46, 32);
    assert!(
        edge[3] > 0 && edge[3] < 255,
        "edge should be partly covered: {edge:?}"
    );
    assert_eq!(pixel(&c, 60, 32)[3], 0, "far away stays clear");
    assert_eq!(backend.unsupported(), 0, "blur is drawn, not counted");
}
