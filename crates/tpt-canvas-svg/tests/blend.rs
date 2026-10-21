//! Blend modes become `mix-blend-mode` on each path element.

use tpt_canvas::prelude::*;
use tpt_canvas::BlendMode;
use tpt_canvas_svg::SvgBackend;

#[test]
fn blend_mode_is_written_on_each_fill_and_stroke() {
    let mut ctx = Context::new(SvgBackend::new(20.0, 20.0));
    ctx.set_blend_mode(BlendMode::Multiply);
    ctx.fill_rect(0.0, 0.0, 5.0, 5.0);
    ctx.stroke_rect(1.0, 1.0, 3.0, 3.0);
    let backend = ctx.into_backend();
    // SVG can blend natively, so nothing is counted as skipped.
    assert_eq!(backend.unsupported(), 0);
    let svg = backend.finish();
    assert_eq!(svg.matches("mix-blend-mode:multiply").count(), 2, "{svg}");
}

#[test]
fn normal_blending_writes_no_attribute() {
    let mut ctx = Context::new(SvgBackend::new(20.0, 20.0));
    ctx.fill_rect(0.0, 0.0, 5.0, 5.0);
    let svg = ctx.into_backend().finish();
    assert!(!svg.contains("mix-blend-mode"), "{svg}");
}

#[test]
fn restore_stops_the_blend_mode_for_later_shapes() {
    let mut ctx = Context::new(SvgBackend::new(20.0, 20.0));
    ctx.save();
    ctx.set_blend_mode(BlendMode::Screen);
    ctx.fill_rect(0.0, 0.0, 5.0, 5.0);
    ctx.restore();
    ctx.fill_rect(8.0, 8.0, 5.0, 5.0);
    let svg = ctx.into_backend().finish();
    assert_eq!(svg.matches("mix-blend-mode:screen").count(), 1, "{svg}");
}
