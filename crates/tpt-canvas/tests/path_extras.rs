//! Hit tests, SVG path parsing, the `path!` macro, and global alpha.

use tpt_canvas::prelude::*;
use tpt_canvas::{path, Path, PathCommand, SvgPathError};

fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut b = PathBuilder::new();
    b.rect(x, y, w, h);
    b.build()
}

#[test]
fn contains_point_nonzero_and_even_odd_differ_on_nested_subpaths() {
    // Two same-direction squares: the inner one has winding 2.
    let mut b = PathBuilder::new();
    b.rect(0.0, 0.0, 10.0, 10.0);
    b.rect(2.0, 2.0, 6.0, 6.0);
    let p = b.build();
    let inside_ring = Vec2::new(1.0, 1.0);
    let inside_hole = Vec2::new(5.0, 5.0);
    assert!(p.contains_point(inside_ring, FillRule::NonZero));
    assert!(p.contains_point(inside_ring, FillRule::EvenOdd));
    assert!(p.contains_point(inside_hole, FillRule::NonZero));
    assert!(!p.contains_point(inside_hole, FillRule::EvenOdd));
    assert!(!p.contains_point(Vec2::new(20.0, 5.0), FillRule::NonZero));
}

#[test]
fn contains_point_handles_curves() {
    let mut b = PathBuilder::new();
    b.circle(0.0, 0.0, 10.0);
    let c = b.build();
    assert!(c.contains_point(Vec2::new(9.0, 0.0), FillRule::NonZero));
    assert!(!c.contains_point(Vec2::new(11.0, 0.0), FillRule::NonZero));
}

#[test]
fn stroke_contains_point_follows_width_and_butt_caps() {
    let line = path! { M 0 0 L 10 0 };
    let style = StrokeStyle {
        width: 2.0,
        ..StrokeStyle::default()
    };
    assert!(line.stroke_contains_point(Vec2::new(5.0, 0.5), &style));
    assert!(!line.stroke_contains_point(Vec2::new(5.0, 2.0), &style));
    // Butt caps stop at the endpoint.
    assert!(!line.stroke_contains_point(Vec2::new(-0.5, 0.0), &style));
}

#[test]
fn svg_relative_and_absolute_forms_agree() {
    let abs = Path::from_svg_d("M 0 0 L 10 0 L 10 10 Z").unwrap();
    let rel = Path::from_svg_d("m0 0 l10 0 0 10 z").unwrap();
    assert_eq!(abs, rel);
}

#[test]
fn svg_implicit_lineto_after_moveto() {
    let p = Path::from_svg_d("M0 0 10 10 20 0").unwrap();
    assert_eq!(p.commands().len(), 3);
    assert!(matches!(p.commands()[1], PathCommand::LineTo(_)));
}

#[test]
fn svg_horizontal_and_vertical_lines() {
    let p = Path::from_svg_d("M 0 0 H 10 V 5 h -10").unwrap();
    let expect = Path::from_svg_d("M 0 0 L 10 0 L 10 5 L 0 5").unwrap();
    assert_eq!(p, expect);
}

#[test]
fn svg_smooth_cubic_reflects_the_previous_control_point() {
    let s = Path::from_svg_d("M0 0 C0 10 10 10 10 0 S 20 -10 20 0").unwrap();
    let explicit = Path::from_svg_d("M0 0 C0 10 10 10 10 0 C 10 -10 20 -10 20 0").unwrap();
    assert_eq!(s, explicit);
}

#[test]
fn svg_smooth_quad_reflects_the_previous_control_point() {
    let t = Path::from_svg_d("M0 0 Q5 10 10 0 T20 0").unwrap();
    let explicit = Path::from_svg_d("M0 0 Q5 10 10 0 Q15 -10 20 0").unwrap();
    assert_eq!(t, explicit);
}

#[test]
fn svg_arc_matches_the_builder() {
    let p = Path::from_svg_d("M0 0 A5 5 0 0 1 10 0").unwrap();
    let mut b = PathBuilder::new();
    b.move_to(0.0, 0.0);
    b.elliptical_arc(5.0, 5.0, 0.0, false, true, 10.0, 0.0);
    assert_eq!(p, b.build());
}

#[test]
fn svg_compact_arc_flags_parse() {
    // Flags written without separators: "0110" is large=0 sweep=1 then x=10 y=0.
    let compact = Path::from_svg_d("M0 0 A5 5 0 0110 0").unwrap();
    let spaced = Path::from_svg_d("M0 0 A5 5 0 0 1 10 0").unwrap();
    assert_eq!(compact, spaced);
}

#[test]
fn svg_rejects_bad_input_with_an_offset() {
    assert_eq!(Path::from_svg_d("L 10 10"), Err(SvgPathError { offset: 0 }));
    assert!(Path::from_svg_d("M 0 0 Z 5 5").is_err());
    assert!(Path::from_svg_d("M 0 0 L").is_err());
    assert!(Path::from_svg_d("M0 0 A1 1 0 2 0 1 1").is_err());
    assert!(Path::from_svg_d("M 0 0 X 1 1").is_err());
}

#[test]
fn svg_decimals_and_exponents() {
    let p = Path::from_svg_d("M.5.5 L1e1-2.5").unwrap();
    let expect = Path::from_svg_d("M0.5 0.5 L10 -2.5").unwrap();
    assert_eq!(p, expect);
}

#[test]
fn path_macro_builds_the_same_path() {
    let m = path! { M 0 0 L 10 0 L 10 10 Z };
    assert_eq!(m, Path::from_svg_d("M 0 0 L 10 0 L 10 10 Z").unwrap());
    let negative = path! { M -5 0 L 5 0 };
    assert_eq!(negative, Path::from_svg_d("M -5 0 L 5 0").unwrap());
}

#[test]
fn global_alpha_scales_fills_and_is_restored() {
    let mut ctx = Context::new(DisplayList::new());
    let square = rect(0.0, 0.0, 4.0, 4.0);
    ctx.save();
    ctx.set_global_alpha(0.5);
    ctx.fill_path(&square);
    ctx.restore();
    ctx.fill_path(&square);
    assert_eq!(ctx.global_alpha(), 1.0);

    let list = ctx.into_backend();
    let alphas: Vec<f32> = list
        .ops()
        .iter()
        .filter_map(|op| match op {
            DrawOp::FillPath {
                paint: Paint::Solid(c),
                ..
            } => Some(c.a),
            _ => None,
        })
        .collect();
    assert_eq!(alphas, [0.5, 1.0]);
}

#[test]
fn global_alpha_clamps_and_ignores_nan() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.set_global_alpha(3.0);
    assert_eq!(ctx.global_alpha(), 1.0);
    ctx.set_global_alpha(-1.0);
    assert_eq!(ctx.global_alpha(), 0.0);
    ctx.set_global_alpha(0.25);
    ctx.set_global_alpha(f32::NAN);
    assert_eq!(ctx.global_alpha(), 0.25);
}
