//! Pattern paints in SVG: each registered image becomes a `<pattern>` tiled over user space.

use tpt_canvas::{Affine2, CanvasBackend, Color, FillRule, ImageId, Paint, PathBuilder};
use tpt_canvas_raster::Pixmap;
use tpt_canvas_svg::SvgBackend;

fn two_by_two() -> Pixmap {
    let rgba = [
        255, 0, 0, 255, 0, 255, 0, 255, //
        0, 0, 255, 255, 255, 255, 0, 255,
    ];
    Pixmap::from_rgba8(2, 2, &rgba).unwrap()
}

fn rect() -> tpt_canvas::path::Path {
    let mut b = PathBuilder::new();
    b.rect(0.0, 0.0, 50.0, 50.0);
    b.build()
}

#[test]
fn pattern_fill_writes_a_tiled_pattern_of_the_image() {
    let mut svg = SvgBackend::new(100.0, 100.0);
    svg.add_image(ImageId(1), two_by_two());
    let paint = Paint::Pattern {
        image: ImageId(1),
        transform: Affine2::scale(10.0, 10.0),
    };
    svg.fill_path(&rect(), &paint, FillRule::NonZero);
    assert_eq!(svg.unsupported(), 0);
    let doc = svg.finish();
    assert!(
        doc.contains(r#"<pattern id="pt1" patternUnits="userSpaceOnUse" width="2" height="2""#),
        "{doc}"
    );
    assert!(doc.contains("data:image/png;base64,"), "tile image missing");
    assert!(
        doc.contains(r#"fill="url(#pt1)""#),
        "fill does not use the pattern"
    );
}

#[test]
fn same_pattern_is_defined_once() {
    let mut svg = SvgBackend::new(100.0, 100.0);
    svg.add_image(ImageId(1), two_by_two());
    let paint = Paint::Pattern {
        image: ImageId(1),
        transform: Affine2::IDENTITY,
    };
    svg.fill_path(&rect(), &paint, FillRule::NonZero);
    svg.fill_path(&rect(), &paint, FillRule::NonZero);
    let doc = svg.finish();
    assert_eq!(doc.matches("<pattern ").count(), 1);
    assert_eq!(doc.matches(r#"fill="url(#pt1)""#).count(), 2);
}

#[test]
fn unregistered_pattern_is_counted_and_draws_nothing() {
    let mut svg = SvgBackend::new(100.0, 100.0);
    let paint = Paint::Pattern {
        image: ImageId(4),
        transform: Affine2::IDENTITY,
    };
    svg.fill_path(&rect(), &paint, FillRule::NonZero);
    assert_eq!(svg.unsupported(), 1);
    let doc = svg.finish();
    assert!(doc.contains(r#"fill="none""#), "{doc}");
    assert!(!doc.contains("<pattern "));
}

#[test]
fn solid_fills_are_unaffected_by_patterns() {
    let mut svg = SvgBackend::new(100.0, 100.0);
    svg.fill_path(
        &rect(),
        &Paint::Solid(Color::rgb(1.0, 0.0, 0.0)),
        FillRule::NonZero,
    );
    assert!(!svg.finish().contains("<pattern"));
}
