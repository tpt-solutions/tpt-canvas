//! Context state stack, transform bookkeeping, and immediate/retained equivalence.

use tpt_canvas::text::ShapedRun;
use tpt_canvas::{
    Affine2, CanvasBackend, Color, Context, DisplayList, DrawOp, FillRule, ImageId, ImageRef,
    Paint, Path, PathBuilder, Rect, StrokeStyle, Vec2,
};

fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut b = PathBuilder::new();
    b.rect(x, y, w, h);
    b.build()
}

/// Draws a small scene using only the public Context API.
fn draw_scene<B: CanvasBackend>(ctx: &mut Context<B>) {
    ctx.set_fill_paint(Paint::Solid(Color::rgb(1.0, 0.0, 0.0)));
    ctx.save();
    ctx.translate(10.0, 10.0);
    ctx.clip(&rect_path(0.0, 0.0, 50.0, 50.0));
    ctx.rotate(0.5);
    ctx.fill_path(&rect_path(0.0, 0.0, 20.0, 20.0));
    ctx.save();
    ctx.scale(2.0, 0.5);
    ctx.set_stroke_style(StrokeStyle {
        width: 3.0,
        ..StrokeStyle::default()
    });
    ctx.stroke_path(&rect_path(1.0, 1.0, 4.0, 4.0));
    ctx.restore();
    ctx.restore();
    ctx.set_fill_rule(FillRule::EvenOdd);
    ctx.fill_path(&rect_path(0.0, 0.0, 5.0, 5.0));
    ctx.draw_image(
        &ImageRef {
            id: ImageId(3),
            width: 4,
            height: 4,
        },
        Rect::new(0.0, 0.0, 4.0, 4.0),
        Rect::new(0.0, 0.0, 8.0, 8.0),
    );
    let mut run = ShapedRun::new(1, 12.0);
    run.push(7, Vec2::new(0.0, 0.0));
    ctx.draw_glyphs(&run);
}

#[test]
fn restore_pops_every_transform_and_clip_it_pushed() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.save();
    ctx.translate(1.0, 2.0);
    ctx.clip(&rect_path(0.0, 0.0, 1.0, 1.0));
    ctx.rotate(0.3);
    ctx.restore();
    let list = ctx.into_backend();
    let pushes = list
        .ops()
        .iter()
        .filter(|o| matches!(o, DrawOp::PushTransform(_) | DrawOp::PushClip(_)))
        .count();
    let pops = list
        .ops()
        .iter()
        .filter(|o| matches!(o, DrawOp::PopTransform | DrawOp::PopClip))
        .count();
    assert_eq!(pushes, 3);
    assert_eq!(pops, 3);
}

#[test]
fn restore_without_save_is_a_no_op() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.restore();
    assert!(ctx.backend().is_empty());
}

#[test]
fn current_transform_tracks_concatenation_and_restore() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.translate(5.0, 0.0);
    ctx.save();
    ctx.scale(2.0, 2.0);
    let p = ctx.current_transform().apply(Vec2::new(1.0, 1.0));
    assert_eq!(p, Vec2::new(7.0, 2.0));
    ctx.restore();
    assert_eq!(
        ctx.current_transform().apply(Vec2::new(1.0, 1.0)),
        Vec2::new(6.0, 1.0)
    );
}

#[test]
fn set_transform_replaces_the_current_matrix() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.translate(100.0, 100.0);
    ctx.rotate(1.0);
    let target = Affine2::new(2.0, 0.0, 0.0, 3.0, 4.0, 5.0);
    ctx.set_transform(target);
    assert_eq!(ctx.current_transform(), target);

    // Replaying the recorded pushes onto an identity stack must give the target too.
    let mut stack = Affine2::IDENTITY;
    for op in ctx.backend().ops() {
        match op {
            DrawOp::PushTransform(m) => stack = stack * *m,
            DrawOp::PopTransform => {}
            _ => {}
        }
    }
    let probe = Vec2::new(0.25, -0.75);
    assert!(stack.apply(probe).distance(target.apply(probe)) < 1e-4);
}

#[test]
fn retained_replay_matches_immediate_mode() {
    // Immediate: draw straight into a recording list.
    let mut direct = Context::new(DisplayList::new());
    draw_scene(&mut direct);
    let direct = direct.into_backend();

    // Retained: record the same scene, then replay it into a second list.
    let mut recorded = Context::new(DisplayList::new());
    draw_scene(&mut recorded);
    let recorded = recorded.into_backend();
    let mut replayed = DisplayList::new();
    recorded.replay(&mut replayed);

    assert_eq!(direct, replayed);
    assert!(!direct.is_empty());
}

#[test]
fn backend_can_be_borrowed_as_dyn() {
    let mut list = DisplayList::new();
    {
        let dyn_backend: &mut dyn CanvasBackend = &mut list;
        let mut ctx = Context::new(dyn_backend);
        draw_scene(&mut ctx);
    }
    assert!(list.len() > 5);
}

#[test]
fn restore_pops_in_reverse_push_order() {
    // Pushes are transform, clip, transform. Pops must mirror that order, not group by kind.
    let mut ctx = Context::new(DisplayList::new());
    ctx.save();
    ctx.translate(1.0, 2.0);
    ctx.clip(&rect_path(0.0, 0.0, 1.0, 1.0));
    ctx.rotate(0.3);
    ctx.restore();
    let list = ctx.into_backend();
    let pops: Vec<&DrawOp> = list
        .ops()
        .iter()
        .filter(|o| matches!(o, DrawOp::PopTransform | DrawOp::PopClip))
        .collect();
    assert!(matches!(
        pops.as_slice(),
        [DrawOp::PopTransform, DrawOp::PopClip, DrawOp::PopTransform]
    ));
}
