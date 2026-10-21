//! Blurred fills become an feGaussianBlur filter sized to the blur spread.

use tpt_canvas::color::named;
use tpt_canvas::{CanvasBackend, Color, FillRule, Paint, PathBuilder};
use tpt_canvas_svg::SvgBackend;

#[test]
fn blurred_fill_emits_a_gaussian_filter_and_is_not_counted() {
    let mut svg = SvgBackend::new(100.0, 100.0);
    let mut b = PathBuilder::new();
    b.rect(10.0, 10.0, 20.0, 20.0);
    svg.fill_path_blurred(
        &b.build(),
        &Paint::Solid(Color::rgba(0.0, 0.0, 0.0, 0.5)),
        FillRule::NonZero,
        2.0,
    );
    assert_eq!(svg.unsupported(), 0);
    let out = svg.finish();
    assert!(
        out.contains(r#"<feGaussianBlur stdDeviation="2"/>"#),
        "{out}"
    );
    // Region: the box 10..30 grown by 3 sigma (6) on each side.
    assert!(
        out.contains(r#"x="4" y="4" width="32" height="32""#),
        "{out}"
    );
    assert!(out.contains(r#"filter="url(#blur1)""#), "{out}");
}

#[test]
fn zero_sigma_is_a_plain_fill() {
    let mut svg = SvgBackend::new(10.0, 10.0);
    let mut b = PathBuilder::new();
    b.rect(1.0, 1.0, 4.0, 4.0);
    svg.fill_path_blurred(
        &b.build(),
        &Paint::Solid(named::RED),
        FillRule::NonZero,
        0.0,
    );
    let out = svg.finish();
    assert!(!out.contains("feGaussianBlur"), "{out}");
    assert!(out.contains("<path"), "{out}");
}
