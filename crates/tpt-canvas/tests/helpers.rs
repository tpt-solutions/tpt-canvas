//! Colour, paint and shape helpers.

use tpt_canvas::color::{named, ColorExt};
use tpt_canvas::prelude::*;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn hex_parses_short_long_and_alpha_forms() {
    assert_eq!(
        Color::from_hex("#ff8000"),
        Some(Color::from_rgb8(255, 128, 0))
    );
    assert_eq!(Color::from_hex("f80"), Some(Color::from_rgb8(255, 136, 0)));
    assert_eq!(
        Color::from_hex("#00000080"),
        Some(Color::from_rgba8(0, 0, 0, 128))
    );
    assert_eq!(Color::from_hex("#fff"), Some(named::WHITE));
}

#[test]
fn hex_rejects_bad_input() {
    assert_eq!(Color::from_hex(""), None);
    assert_eq!(Color::from_hex("#12345"), None);
    assert_eq!(Color::from_hex("#ggg"), None);
    assert_eq!(Color::from_hex("#héé"), None);
}

#[test]
fn mix_and_lighten_darken_behave_at_the_ends() {
    let c = Color::rgba(0.2, 0.4, 0.6, 0.5);
    assert_eq!(c.mix(named::WHITE, 0.0), c);
    assert_eq!(c.mix(named::WHITE, 1.0), Color::rgba(1.0, 1.0, 1.0, 1.0));
    let light = c.lighten(0.5);
    assert!(close(light.r, 0.6) && close(light.a, 0.5));
    let dark = c.darken(0.5);
    assert!(close(dark.g, 0.2) && close(dark.a, 0.5));
}

#[test]
fn named_colors_match_their_hex_values() {
    assert_eq!(named::RED, Color::from_hex("#ff0000").unwrap());
    assert_eq!(named::BLUE, Color::from_hex("#0000ff").unwrap());
    let gray = Color::from_hex("#808080").unwrap();
    assert!(close(named::GRAY.r, gray.r) && close(named::GRAY.b, gray.b));
}

#[test]
fn paint_linear_sorts_stops_and_defaults_to_pad() {
    let p = Paint::linear(
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        &[(1.0, named::BLUE), (0.0, named::RED)],
    );
    match p {
        Paint::LinearGradient {
            stops,
            spread,
            space,
            ..
        } => {
            assert_eq!(stops[0].offset, 0.0);
            assert_eq!(stops[1].offset, 1.0);
            assert_eq!(spread, SpreadMode::Pad);
            assert_eq!(space, ColorSpace::Srgb);
        }
        _ => panic!("expected a linear gradient"),
    }
}

#[test]
fn paint_radial_builds_a_radial_gradient() {
    let p = Paint::radial(
        Vec2::new(5.0, 5.0),
        3.0,
        &[(0.0, named::WHITE), (1.0, named::BLACK)],
    );
    assert!(matches!(p, Paint::RadialGradient { radius, .. } if radius == 3.0));
}

#[test]
fn context_shape_helpers_record_one_fill_or_stroke_each() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.fill_rect(0.0, 0.0, 4.0, 4.0);
    ctx.stroke_rect(0.0, 0.0, 4.0, 4.0);
    ctx.fill_circle(2.0, 2.0, 1.0);
    let ops = ctx.into_backend();
    let fills = ops
        .ops()
        .iter()
        .filter(|o| matches!(o, DrawOp::FillPath { .. }))
        .count();
    let strokes = ops
        .ops()
        .iter()
        .filter(|o| matches!(o, DrawOp::StrokePath { .. }))
        .count();
    assert_eq!((fills, strokes), (2, 1));
}
