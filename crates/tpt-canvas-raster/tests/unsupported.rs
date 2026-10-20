//! Calls the raster backend cannot draw yet must be counted, not dropped silently.

use tpt_canvas::text::ShapedRun;
use tpt_canvas::{
    CanvasBackend, Color, FillRule, ImageId, ImageRef, Paint, Path, PathBuilder, Rect, StrokeStyle,
};
use tpt_canvas_raster::RasterBackend;

fn square() -> Path {
    let mut b = PathBuilder::new();
    b.rect(0.0, 0.0, 4.0, 4.0);
    b.build()
}

#[test]
fn skipped_calls_are_counted() {
    let mut r = RasterBackend::new(8, 8);
    assert_eq!(r.unsupported(), 0);

    let pattern = Paint::Pattern {
        image: ImageId(1),
        transform: tpt_canvas::Affine2::IDENTITY,
    };
    r.fill_path(&square(), &pattern, FillRule::NonZero);
    r.stroke_path(&square(), &pattern, &StrokeStyle::default());
    r.draw_glyphs(&ShapedRun::new(1, 12.0), &Paint::Solid(Color::BLACK));
    r.draw_image(
        &ImageRef {
            id: ImageId(1),
            width: 1,
            height: 1,
        },
        Rect::new(0.0, 0.0, 1.0, 1.0),
        Rect::new(0.0, 0.0, 1.0, 1.0),
    );
    assert_eq!(r.unsupported(), 4);

    // Solid fills are drawn, so they are not counted.
    r.fill_path(&square(), &Paint::Solid(Color::BLACK), FillRule::NonZero);
    assert_eq!(r.unsupported(), 4);
}
