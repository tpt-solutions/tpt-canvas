//! Pixel checks for drop shadows on the raster backend.

use tpt_canvas::prelude::*;
use tpt_canvas::Shadow;
use tpt_canvas_raster::RasterBackend;

const W: u32 = 64;
const H: u32 = 64;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut b = PathBuilder::new();
    b.rect(x, y, w, h);
    b.build()
}

/// Alpha at (x, y) of the finished canvas.
fn alpha(ctx: &Context<RasterBackend>, x: u32, y: u32) -> f32 {
    ctx.backend().pixmap().pixel(x, y)[3]
}

#[test]
fn sharp_offset_shadow_appears_at_the_offset_position() {
    let mut ctx = Context::new(RasterBackend::new(W, H));
    ctx.set_shadow(Shadow {
        color: Color::rgba(0.0, 0.0, 0.0, 1.0),
        offset: Vec2::new(10.0, 0.0),
        blur: 0.0,
    });
    ctx.set_fill_paint(Paint::solid(named::RED));
    ctx.fill_path(&rect(10.0, 10.0, 20.0, 20.0));

    // Under the fill, the shadow is covered by the opaque red fill.
    // Only the shadow reaches x in [30, 40).
    assert!(alpha(&ctx, 35, 20) > 0.99, "shadow should be opaque");
    assert_eq!(alpha(&ctx, 45, 20), 0.0, "no shadow past its offset edge");
    assert!(alpha(&ctx, 15, 20) > 0.99, "fill is drawn on top");
}

#[test]
fn blur_spreads_the_shadow_past_its_edge_and_fades_it() {
    let mut ctx = Context::new(RasterBackend::new(W, H));
    ctx.set_shadow(Shadow {
        color: Color::rgba(0.0, 0.0, 0.0, 1.0),
        offset: Vec2::new(20.0, 0.0),
        blur: 6.0,
    });
    ctx.set_fill_paint(Paint::solid(named::RED));
    ctx.fill_path(&rect(5.0, 20.0, 10.0, 10.0));

    // The unblurred shadow would end at x = 35. Blur reaches a little past it,
    // but the far side stays clear.
    let just_outside = alpha(&ctx, 36, 25);
    let far = alpha(&ctx, 50, 25);
    assert!(
        just_outside > 0.0 && just_outside < 1.0,
        "edge should soften: {just_outside}"
    );
    assert_eq!(far, 0.0);
}

#[test]
fn shadow_is_cut_off_by_the_clip() {
    let mut ctx = Context::new(RasterBackend::new(W, H));
    ctx.clip(&rect(0.0, 0.0, 30.0, H as f32));
    ctx.set_shadow(Shadow {
        color: Color::rgba(0.0, 0.0, 0.0, 1.0),
        offset: Vec2::new(20.0, 0.0),
        blur: 4.0,
    });
    ctx.set_fill_paint(Paint::solid(named::RED));
    ctx.fill_path(&rect(5.0, 20.0, 10.0, 10.0));

    // The shadow of the rectangle sits around x = 25..35, and the clip ends at x = 30.
    // The part past x = 30 must be clipped, even though blur would spread it.
    assert_eq!(alpha(&ctx, 32, 25), 0.0, "shadow must respect the clip");
    assert!(alpha(&ctx, 27, 25) > 0.0, "shadow inside the clip is kept");
}

#[test]
fn blurred_shadow_keeps_most_of_its_coverage() {
    let mut ctx = Context::new(RasterBackend::new(W, H));
    ctx.set_shadow(Shadow {
        color: Color::rgba(0.0, 0.0, 0.0, 1.0),
        offset: Vec2::new(0.0, 0.0),
        blur: 4.0,
    });
    // A fully transparent fill leaves only the shadow on the canvas.
    ctx.set_fill_paint(Paint::solid(Color::TRANSPARENT));
    ctx.fill_path(&rect(20.0, 20.0, 10.0, 10.0));

    let pixmap = ctx.backend().pixmap();
    let total: f32 = pixmap.data.iter().map(|p| p[3]).sum();
    // A 10 x 10 opaque square has 100 units of alpha. Blur spreads it but keeps the mass.
    assert!((total - 100.0).abs() < 5.0, "alpha mass was {total}");
    assert!(alpha(&ctx, 25, 25) > 0.9, "centre stays dense");
}
