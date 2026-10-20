//! `clear_rect` on the raster backend: erased pixels, edges, and clipping.

use tpt_canvas::{Affine2, CanvasBackend, Color, Context, FillRule, Paint, PathBuilder};
use tpt_canvas_raster::RasterBackend;

fn rect(x: f32, y: f32, w: f32, h: f32) -> tpt_canvas::Path {
    let mut b = PathBuilder::new();
    b.rect(x, y, w, h);
    b.build()
}

fn filled_8x8() -> RasterBackend {
    let mut r = RasterBackend::new(8, 8);
    r.fill_path(
        &rect(0.0, 0.0, 8.0, 8.0),
        &Paint::Solid(Color::BLACK),
        FillRule::NonZero,
    );
    r
}

#[test]
fn clear_erases_the_covered_pixels_only() {
    let mut r = filled_8x8();
    r.clear_path(&rect(2.0, 2.0, 4.0, 4.0));
    let px = r.pixmap();
    assert_eq!(px.pixel(3, 3)[3], 0.0, "inside the cleared rectangle");
    assert_eq!(px.pixel(0, 0)[3], 1.0, "outside stays opaque");
    assert_eq!(px.pixel(7, 7)[3], 1.0, "outside stays opaque");
}

#[test]
fn clear_respects_the_transform() {
    let mut r = filled_8x8();
    r.push_transform(&Affine2::translate(4.0, 0.0));
    r.clear_path(&rect(0.0, 0.0, 4.0, 8.0));
    r.pop_transform();
    let px = r.pixmap();
    assert_eq!(px.pixel(5, 0)[3], 0.0, "translated into the right half");
    assert_eq!(px.pixel(1, 0)[3], 1.0, "left half untouched");
}

#[test]
fn clear_respects_the_clip() {
    let mut r = filled_8x8();
    r.push_clip(&rect(0.0, 0.0, 4.0, 8.0));
    r.clear_path(&rect(0.0, 0.0, 8.0, 8.0));
    r.pop_clip();
    let px = r.pixmap();
    assert_eq!(px.pixel(1, 1)[3], 0.0, "inside the clip");
    assert_eq!(px.pixel(6, 1)[3], 1.0, "outside the clip stays");
}

#[test]
fn clear_is_not_counted_as_unsupported() {
    let mut r = filled_8x8();
    r.clear_path(&rect(0.0, 0.0, 1.0, 1.0));
    assert_eq!(r.unsupported(), 0);
}

#[test]
fn partial_coverage_fades_edge_pixels() {
    // A clear that covers half of pixel column 4 leaves that column half opaque.
    let mut r = filled_8x8();
    r.clear_path(&rect(0.0, 0.0, 4.5, 8.0));
    let edge = r.pixmap().pixel(4, 3)[3];
    assert!((edge - 0.5).abs() < 0.02, "edge alpha was {edge}");
}

#[test]
fn context_clear_rect_reaches_the_raster_backend() {
    let mut ctx = Context::new(filled_8x8());
    ctx.clear_rect(0.0, 0.0, 2.0, 8.0);
    let r = ctx.into_backend();
    assert_eq!(r.pixmap().pixel(0, 4)[3], 0.0);
    assert_eq!(r.pixmap().pixel(5, 4)[3], 1.0);
}
