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
