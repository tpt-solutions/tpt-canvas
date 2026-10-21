//! Pattern paints in PDF: a registered image becomes a tiling pattern, used by fills and by
//! strokes. The matrix carries the page's y flip, so the tests check its numbers.

use tpt_canvas::prelude::*;
use tpt_canvas::{Affine2, CanvasBackend, ImageId};
use tpt_canvas_pdf::PdfBackend;
use tpt_canvas_raster::Pixmap;

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

fn two_by_two() -> Pixmap {
    let rgba = [
        255, 0, 0, 255, 0, 255, 0, 255, //
        0, 0, 255, 255, 255, 255, 0, 255,
    ];
    Pixmap::from_rgba8(2, 2, &rgba).unwrap()
}

fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut b = PathBuilder::new();
    b.rect(x, y, w, h);
    b.build()
}

#[test]
fn pattern_fill_is_a_tiling_pattern_with_the_image_tile() {
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.add_image(ImageId(1), two_by_two());
    let paint = Paint::Pattern {
        image: ImageId(1),
        transform: Affine2::scale(10.0, 10.0),
    };
    backend.fill_path(
        &rect_path(0.0, 0.0, 100.0, 100.0),
        &paint,
        FillRule::NonZero,
    );
    assert_eq!(backend.unsupported(), 0);
    let pdf = text(backend.finish());
    assert!(pdf.contains("/PatternType 1"), "no tiling pattern");
    assert!(pdf.contains("/XStep 2 /YStep 2"), "tile is the image size");
    assert!(
        pdf.contains("/Pattern cs /P0 scn"),
        "fill does not use the pattern"
    );
    assert!(
        pdf.contains("/Pattern << /P0 "),
        "page does not list the pattern"
    );
}

#[test]
fn matrix_flips_y_about_the_page_height() {
    // Translate by (10, 20) on a 100-high page: the flip maps y to 100 - 20 = 80.
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.add_image(ImageId(1), two_by_two());
    let paint = Paint::Pattern {
        image: ImageId(1),
        transform: Affine2::translate(10.0, 20.0),
    };
    backend.fill_path(&rect_path(0.0, 0.0, 5.0, 5.0), &paint, FillRule::NonZero);
    let pdf = text(backend.finish());
    assert!(
        pdf.contains("/Matrix [1 0 0 -1 10 80]"),
        "matrix was not as expected"
    );
}

#[test]
fn pattern_stroke_clips_to_the_outline_and_paints_the_pattern() {
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.add_image(ImageId(1), two_by_two());
    let paint = Paint::Pattern {
        image: ImageId(1),
        transform: Affine2::IDENTITY,
    };
    let style = StrokeStyle {
        width: 4.0,
        ..StrokeStyle::default()
    };
    backend.stroke_path(&rect_path(10.0, 10.0, 40.0, 40.0), &paint, &style);
    assert_eq!(backend.unsupported(), 0);
    let pdf = text(backend.finish());
    assert!(pdf.contains("/Pattern cs /P0 scn"));
    assert!(pdf.contains("W n"), "stroke was not clipped to its outline");
}

#[test]
fn unregistered_pattern_is_counted() {
    let mut backend = PdfBackend::new(100.0, 100.0);
    let paint = Paint::Pattern {
        image: ImageId(9),
        transform: Affine2::IDENTITY,
    };
    backend.fill_path(&rect_path(0.0, 0.0, 5.0, 5.0), &paint, FillRule::NonZero);
    assert_eq!(backend.unsupported(), 1);
    assert!(!text(backend.finish()).contains("/PatternType"));
}

#[test]
fn same_pattern_is_reused_on_one_page() {
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.add_image(ImageId(1), two_by_two());
    let paint = Paint::Pattern {
        image: ImageId(1),
        transform: Affine2::IDENTITY,
    };
    backend.fill_path(&rect_path(0.0, 0.0, 5.0, 5.0), &paint, FillRule::NonZero);
    backend.fill_path(&rect_path(9.0, 9.0, 5.0, 5.0), &paint, FillRule::NonZero);
    let pdf = text(backend.finish());
    assert_eq!(
        pdf.matches("/PatternType 1").count(),
        1,
        "pattern was made twice"
    );
}
