//! Blend modes on the raster backend, checked against the separable formulas.

use tpt_canvas::prelude::*;
use tpt_canvas::BlendMode;
use tpt_canvas_raster::RasterBackend;

const TOL: f32 = 0.01;

/// Fills an opaque `backdrop` grey, then draws an opaque `source` grey in `mode`,
/// and returns the red channel of the middle pixel.
fn composite(backdrop: f32, source: f32, mode: BlendMode) -> f32 {
    let mut ctx = Context::new(RasterBackend::new(8, 8));
    ctx.set_fill_paint(Paint::solid(Color::rgb(backdrop, backdrop, backdrop)));
    ctx.fill_rect(0.0, 0.0, 8.0, 8.0);
    ctx.set_blend_mode(mode);
    ctx.set_fill_paint(Paint::solid(Color::rgb(source, source, source)));
    ctx.fill_rect(0.0, 0.0, 8.0, 8.0);
    ctx.backend().pixmap().pixel(4, 4)[0]
}

fn assert_close(got: f32, want: f32, what: &str) {
    assert!((got - want).abs() < TOL, "{what}: got {got}, want {want}");
}

#[test]
fn multiply_darkens_by_the_product() {
    assert_close(composite(0.5, 0.5, BlendMode::Multiply), 0.25, "multiply");
    assert_close(
        composite(1.0, 0.4, BlendMode::Multiply),
        0.4,
        "multiply on white",
    );
}

#[test]
fn screen_lightens_by_the_inverse_product() {
    assert_close(composite(0.5, 0.5, BlendMode::Screen), 0.75, "screen");
    assert_close(
        composite(0.0, 0.4, BlendMode::Screen),
        0.4,
        "screen on black",
    );
}

#[test]
fn darken_and_lighten_take_the_extreme() {
    assert_close(composite(0.25, 0.75, BlendMode::Darken), 0.25, "darken");
    assert_close(composite(0.25, 0.75, BlendMode::Lighten), 0.75, "lighten");
}

#[test]
fn difference_and_exclusion() {
    assert_close(
        composite(0.8, 0.3, BlendMode::Difference),
        0.5,
        "difference",
    );
    assert_close(composite(0.5, 0.5, BlendMode::Exclusion), 0.5, "exclusion");
    assert_close(
        composite(0.2, 0.6, BlendMode::Exclusion),
        0.2 + 0.6 - 2.0 * 0.2 * 0.6,
        "exclusion",
    );
}

#[test]
fn overlay_depends_on_the_backdrop() {
    // Dark backdrop (<= 0.5): multiply(source, 2 * backdrop).
    assert_close(
        composite(0.25, 0.75, BlendMode::Overlay),
        0.75 * 0.5,
        "overlay, dark backdrop",
    );
    // Light backdrop (> 0.5): screen(source, 2 * backdrop - 1).
    let s = 2.0 * 0.75 - 1.0;
    assert_close(
        composite(0.75, 0.5, BlendMode::Overlay),
        0.5 + s - 0.5 * s,
        "overlay, light backdrop",
    );
}

#[test]
fn partial_source_alpha_mixes_with_the_normal_result() {
    // Half-transparent multiply over white: half the multiplied colour, half the white.
    let mut ctx = Context::new(RasterBackend::new(4, 4));
    ctx.set_fill_paint(Paint::solid(Color::WHITE));
    ctx.fill_rect(0.0, 0.0, 4.0, 4.0);
    ctx.set_blend_mode(BlendMode::Multiply);
    ctx.set_fill_paint(Paint::solid(Color::rgba(0.5, 0.5, 0.5, 0.5)));
    ctx.fill_rect(0.0, 0.0, 4.0, 4.0);
    assert_close(
        ctx.backend().pixmap().pixel(2, 2)[0],
        0.75,
        "half-alpha multiply",
    );
}

#[test]
fn restore_puts_the_outer_blend_mode_back() {
    let mut ctx = Context::new(RasterBackend::new(4, 4));
    ctx.save();
    ctx.set_blend_mode(BlendMode::Screen);
    assert_eq!(ctx.blend_mode(), BlendMode::Screen);
    ctx.restore();
    assert_eq!(ctx.blend_mode(), BlendMode::Normal);
}

#[test]
fn normal_mode_is_unchanged_source_over() {
    assert_close(composite(0.5, 0.25, BlendMode::Normal), 0.25, "normal");
}
