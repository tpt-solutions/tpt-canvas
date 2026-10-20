//! Calls the SVG backend writes as placeholders must be counted.

use tpt_canvas::text::ShapedRun;
use tpt_canvas::{
    CanvasBackend, Color, FillRule, ImageId, ImageRef, Paint, PathBuilder, Rect, StrokeStyle,
};
use tpt_canvas_svg::SvgBackend;

#[test]
fn placeholder_output_is_counted() {
    let mut s = SvgBackend::new(8.0, 8.0);
    let mut b = PathBuilder::new();
    b.rect(0.0, 0.0, 4.0, 4.0);
    let path = b.build();

    let pattern = Paint::Pattern {
        image: ImageId(1),
        transform: tpt_canvas::Affine2::IDENTITY,
    };
    s.fill_path(&path, &pattern, FillRule::NonZero);
    s.draw_glyphs(&ShapedRun::new(1, 12.0), &Paint::Solid(Color::BLACK));
    s.draw_image(
        &ImageRef {
            id: ImageId(1),
            width: 1,
            height: 1,
        },
        Rect::new(0.0, 0.0, 1.0, 1.0),
        Rect::new(0.0, 0.0, 1.0, 1.0),
    );
    assert_eq!(s.unsupported(), 3);

    s.fill_path(&path, &Paint::Solid(Color::BLACK), FillRule::NonZero);
    s.stroke_path(&path, &Paint::Solid(Color::BLACK), &StrokeStyle::default());
    assert_eq!(s.unsupported(), 3);
}

#[test]
fn clear_path_is_counted_because_svg_cannot_erase() {
    let mut s = SvgBackend::new(8.0, 8.0);
    let mut b = PathBuilder::new();
    b.rect(0.0, 0.0, 4.0, 4.0);
    s.clear_path(&b.build());
    assert_eq!(s.unsupported(), 1);
}

#[test]
fn layer_becomes_a_group_with_opacity() {
    let mut s = SvgBackend::new(8.0, 8.0);
    s.push_layer(0.5);
    let mut b = PathBuilder::new();
    b.rect(0.0, 0.0, 4.0, 4.0);
    s.fill_path(&b.build(), &Paint::Solid(Color::BLACK), FillRule::NonZero);
    s.pop_layer();
    let svg = s.finish();
    assert!(svg.contains(r#"<g opacity="0.5">"#), "{svg}");
    assert_eq!(svg.matches("<g").count(), svg.matches("</g>").count());
}
