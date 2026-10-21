//! Drop shadows: the shadow is drawn before the fill, moved by the offset, and blurred by
//! `fill_path_blurred` with a standard deviation of half the blur size.

use tpt_canvas::prelude::*;
use tpt_canvas::{DisplayList, DrawOp, Shadow, StrokeStyle};

fn shadow(blur: f32, offset: (f32, f32)) -> Shadow {
    Shadow {
        color: Color::rgba(0.0, 0.0, 0.0, 0.5),
        offset: Vec2::new(offset.0, offset.1),
        blur,
    }
}

fn rect() -> Path {
    let mut b = PathBuilder::new();
    b.rect(10.0, 10.0, 20.0, 20.0);
    b.build()
}

#[test]
fn offset_only_shadow_is_a_moved_fill_before_the_real_fill() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.set_shadow(shadow(0.0, (4.0, 6.0)));
    ctx.fill_path(&rect());
    let ops = ctx.into_backend();
    let ops = ops.ops();

    assert_eq!(ops.len(), 2, "shadow then fill: {ops:?}");
    match &ops[0] {
        DrawOp::FillPath { path, paint, .. } => {
            assert_eq!(*paint, Paint::Solid(Color::rgba(0.0, 0.0, 0.0, 0.5)));
            assert_ne!(*path, rect(), "shadow path must be moved");
            assert_eq!(path.bounding_box().x, 14.0);
            assert_eq!(path.bounding_box().y, 16.0);
        }
        other => panic!("expected the shadow fill first, got {other:?}"),
    }
    assert!(matches!(&ops[1], DrawOp::FillPath { .. }));
}

#[test]
fn blurred_shadow_uses_half_the_blur_size_as_sigma() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.set_shadow(shadow(8.0, (0.0, 0.0)));
    ctx.fill_path(&rect());
    let ops = ctx.into_backend();

    match &ops.ops()[0] {
        DrawOp::FillPathBlurred { sigma, .. } => assert_eq!(*sigma, 4.0),
        other => panic!("expected a blurred shadow, got {other:?}"),
    }
}

#[test]
fn shadow_is_restored_with_the_state() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.save();
    ctx.set_shadow(shadow(0.0, (1.0, 1.0)));
    ctx.restore();
    ctx.fill_path(&rect());
    // After restore there is no shadow, so only the fill is recorded.
    assert_eq!(ctx.into_backend().ops().len(), 1);
}

#[test]
fn clear_shadow_stops_shadows() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.set_shadow(shadow(2.0, (1.0, 1.0)));
    ctx.clear_shadow();
    ctx.fill_path(&rect());
    assert_eq!(ctx.into_backend().ops().len(), 1);
}

#[test]
fn transparent_or_empty_shadow_draws_nothing() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.set_shadow(Shadow {
        color: Color::rgba(0.0, 0.0, 0.0, 0.0),
        offset: Vec2::new(5.0, 5.0),
        blur: 4.0,
    });
    ctx.set_shadow(shadow(0.0, (0.0, 0.0)));
    ctx.fill_path(&rect());
    assert_eq!(ctx.into_backend().ops().len(), 1);
}

#[test]
fn blurred_shadow_survives_serialization() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.set_shadow(shadow(6.0, (2.0, 3.0)));
    ctx.fill_path(&rect());
    let list = ctx.into_backend();
    let decoded = DisplayList::deserialize(&list.serialize()).expect("round trip");
    assert_eq!(decoded.ops(), list.ops());
}

fn stroke_style() -> StrokeStyle {
    StrokeStyle {
        width: 2.0,
        ..StrokeStyle::default()
    }
}

#[test]
fn stroke_shadow_is_the_stroke_moved_by_the_offset() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.set_stroke_style(stroke_style());
    ctx.set_shadow(shadow(0.0, (3.0, 0.0)));
    ctx.stroke_path(&rect());
    let ops = ctx.into_backend();
    assert_eq!(ops.ops().len(), 2, "{:?}", ops.ops());
    match &ops.ops()[0] {
        DrawOp::StrokePath { path, paint, .. } => {
            assert_eq!(*paint, Paint::Solid(Color::rgba(0.0, 0.0, 0.0, 0.5)));
            assert_eq!(path.bounding_box().x, 13.0);
        }
        other => panic!("expected the shadow stroke first, got {other:?}"),
    }
}

#[test]
fn blurred_stroke_shadow_is_a_blurred_fill_of_the_outline() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.set_stroke_style(stroke_style());
    ctx.set_shadow(shadow(6.0, (0.0, 0.0)));
    ctx.stroke_path(&rect());
    let ops = ctx.into_backend();
    match &ops.ops()[0] {
        DrawOp::FillPathBlurred { path, sigma, .. } => {
            assert_eq!(*sigma, 3.0);
            // The outline surrounds the rectangle, so it reaches beyond its 10..30 box.
            let bb = path.bounding_box();
            assert!(bb.x < 10.0 && bb.x + bb.w > 30.0, "outline box {bb:?}");
        }
        other => panic!("expected a blurred outline fill, got {other:?}"),
    }
}
