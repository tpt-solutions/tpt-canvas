//! Blend mode bookkeeping in the context and the display list.

use tpt_canvas::prelude::*;
use tpt_canvas::{BlendMode, DisplayList, DrawOp};

const ALL: [BlendMode; 12] = [
    BlendMode::Normal,
    BlendMode::Multiply,
    BlendMode::Screen,
    BlendMode::Overlay,
    BlendMode::Darken,
    BlendMode::Lighten,
    BlendMode::Difference,
    BlendMode::Exclusion,
    BlendMode::Hue,
    BlendMode::Saturation,
    BlendMode::Color,
    BlendMode::Luminosity,
];

fn square() -> Path {
    let mut b = PathBuilder::new();
    b.rect(0.0, 0.0, 4.0, 4.0);
    b.build()
}

#[test]
fn only_changes_reach_the_backend() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.set_blend_mode(BlendMode::Multiply);
    ctx.set_blend_mode(BlendMode::Multiply); // no change, not recorded again
    ctx.fill_path(&square());
    assert_eq!(
        ctx.into_backend().ops(),
        &[
            DrawOp::SetBlendMode(BlendMode::Multiply),
            DrawOp::FillPath {
                path: square(),
                paint: Paint::Solid(Color::BLACK),
                rule: FillRule::NonZero,
            },
        ]
    );
}

#[test]
fn restore_sends_the_outer_mode_only_when_it_differs() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.save();
    ctx.set_blend_mode(BlendMode::Screen);
    ctx.restore();
    let ops = ctx.into_backend();
    assert_eq!(
        ops.ops().last(),
        Some(&DrawOp::SetBlendMode(BlendMode::Normal))
    );

    // Saving and restoring without a change adds nothing.
    let mut ctx = Context::new(DisplayList::new());
    ctx.save();
    ctx.restore();
    assert!(ctx.into_backend().ops().is_empty());
}

#[test]
fn every_mode_survives_serialization() {
    let mut ctx = Context::new(DisplayList::new());
    for mode in ALL {
        ctx.set_blend_mode(mode);
        ctx.fill_path(&square());
    }
    let list = ctx.into_backend();
    let decoded = DisplayList::deserialize(&list.serialize()).expect("round trip");
    assert_eq!(decoded.ops(), list.ops());
}

#[test]
fn css_names_match_the_spec_keywords() {
    assert_eq!(BlendMode::Multiply.css_name(), "multiply");
    assert_eq!(BlendMode::Exclusion.css_name(), "exclusion");
    assert_eq!(BlendMode::Normal.css_name(), "normal");
}
