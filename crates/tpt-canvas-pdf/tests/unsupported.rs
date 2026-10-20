//! Calls the PDF backend writes as placeholder comments must be counted.

use tpt_canvas::text::ShapedRun;
use tpt_canvas::{CanvasBackend, Color, FillRule, ImageId, ImageRef, Paint, PathBuilder, Rect};
use tpt_canvas_pdf::PdfBackend;

#[test]
fn placeholder_output_is_counted() {
    let mut p = PdfBackend::new(8.0, 8.0);
    let mut b = PathBuilder::new();
    b.rect(0.0, 0.0, 4.0, 4.0);
    let path = b.build();

    let pattern = Paint::Pattern {
        image: ImageId(1),
        transform: tpt_canvas::Affine2::IDENTITY,
    };
    p.fill_path(&path, &pattern, FillRule::NonZero);
    p.draw_glyphs(&ShapedRun::new(1, 12.0), &Paint::Solid(Color::BLACK));
    p.draw_image(
        &ImageRef {
            id: ImageId(1),
            width: 1,
            height: 1,
        },
        Rect::new(0.0, 0.0, 1.0, 1.0),
        Rect::new(0.0, 0.0, 1.0, 1.0),
    );
    assert_eq!(p.unsupported(), 3);

    p.fill_path(&path, &Paint::Solid(Color::BLACK), FillRule::NonZero);
    assert_eq!(p.unsupported(), 3);
}

#[test]
fn approximated_gradients_are_counted() {
    use tpt_canvas::{ColorSpace, SpreadMode, Stop, Vec2};
    let mut p = PdfBackend::new(8.0, 8.0);
    let mut b = PathBuilder::new();
    b.rect(0.0, 0.0, 4.0, 4.0);
    let path = b.build();
    let stops = [
        Stop {
            offset: 0.0,
            color: Color::rgb(1.0, 0.0, 0.0),
        },
        Stop {
            offset: 1.0,
            color: Color::rgb(0.0, 0.0, 1.0),
        },
    ];
    let grad = |spread| Paint::LinearGradient {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(4.0, 0.0),
        stops: stops.to_vec(),
        spread,
        space: ColorSpace::Srgb,
    };

    p.fill_path(&path, &grad(SpreadMode::Pad), FillRule::NonZero);
    assert_eq!(p.unsupported(), 0);
    // Linear Repeat and Reflect are tiled, so nothing is approximated.
    p.fill_path(&path, &grad(SpreadMode::Repeat), FillRule::NonZero);
    p.fill_path(&path, &grad(SpreadMode::Reflect), FillRule::NonZero);
    assert_eq!(p.unsupported(), 0);

    // Radial spreads are still drawn as Pad, and are counted.
    let radial = Paint::RadialGradient {
        center: Vec2::new(2.0, 2.0),
        radius: 2.0,
        stops: stops.to_vec(),
        spread: SpreadMode::Repeat,
        space: ColorSpace::Srgb,
    };
    p.fill_path(&path, &radial, FillRule::NonZero);
    assert_eq!(p.unsupported(), 1);
}

#[test]
fn repeated_linear_gradient_is_tiled_per_unit() {
    use tpt_canvas::{ColorSpace, SpreadMode, Stop, Vec2};
    // The gradient runs 0..4 along x and the clip spans 0..8, so two unit tiles cover it.
    let mut p = PdfBackend::new(8.0, 8.0);
    let mut b = PathBuilder::new();
    b.rect(0.0, 0.0, 8.0, 4.0);
    let stops = [
        Stop {
            offset: 0.0,
            color: Color::rgb(1.0, 0.0, 0.0),
        },
        Stop {
            offset: 1.0,
            color: Color::rgb(0.0, 0.0, 1.0),
        },
    ];
    let paint = |spread| Paint::LinearGradient {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(4.0, 0.0),
        stops: stops.to_vec(),
        spread,
        space: ColorSpace::Srgb,
    };
    p.fill_path(&b.build(), &paint(SpreadMode::Repeat), FillRule::NonZero);
    let bytes = p.finish();
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(text.matches("/Extend [false false]").count(), 2);
    // Tile 0 runs forward, tile 1 is the repeat, so both use the same orientation.
    assert!(text.contains("/Coords [0 0 4 0]"));
    assert!(text.contains("/Coords [4 0 8 0]"));
}

#[test]
fn reflected_odd_tiles_run_backwards() {
    use tpt_canvas::{ColorSpace, SpreadMode, Stop, Vec2};
    let mut p = PdfBackend::new(8.0, 8.0);
    let mut b = PathBuilder::new();
    b.rect(0.0, 0.0, 8.0, 4.0);
    let stops = [
        Stop {
            offset: 0.0,
            color: Color::rgb(1.0, 0.0, 0.0),
        },
        Stop {
            offset: 1.0,
            color: Color::rgb(0.0, 0.0, 1.0),
        },
    ];
    let paint = Paint::LinearGradient {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(4.0, 0.0),
        stops: stops.to_vec(),
        spread: SpreadMode::Reflect,
        space: ColorSpace::Srgb,
    };
    p.fill_path(&b.build(), &paint, FillRule::NonZero);
    let bytes = p.finish();
    let text = String::from_utf8_lossy(&bytes);
    // Tile 0 forward, tile 1 reversed: its axis starts at x = 8 and ends at x = 4.
    assert!(text.contains("/Coords [0 0 4 0]"));
    assert!(text.contains("/Coords [8 0 4 0]"));
}
