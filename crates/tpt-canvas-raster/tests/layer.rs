//! Group opacity on the raster backend: a group is composited once, with its alpha.

use tpt_canvas::{CanvasBackend, Color, FillRule, Paint, PathBuilder};
use tpt_canvas_raster::RasterBackend;

fn rect(x: f32, y: f32, w: f32, h: f32) -> tpt_canvas::Path {
    let mut b = PathBuilder::new();
    b.rect(x, y, w, h);
    b.build()
}

#[test]
fn group_alpha_applies_once_to_the_composited_group() {
    let mut r = RasterBackend::new(8, 8);
    r.push_layer(0.5);
    // Two fills of the same square inside the group. Each alone would be fully opaque.
    r.fill_path(
        &rect(0.0, 0.0, 8.0, 8.0),
        &Paint::Solid(Color::BLACK),
        FillRule::NonZero,
    );
    r.fill_path(
        &rect(0.0, 0.0, 8.0, 8.0),
        &Paint::Solid(Color::BLACK),
        FillRule::NonZero,
    );
    r.pop_layer();
    let a = r.pixmap().pixel(3, 3)[3];
    assert!((a - 0.5).abs() < 1e-4, "group alpha was {a}, expected 0.5");
}

#[test]
fn group_is_composited_over_existing_content() {
    let mut r = RasterBackend::new(8, 8);
    r.fill_path(
        &rect(0.0, 0.0, 8.0, 8.0),
        &Paint::Solid(Color::BLACK),
        FillRule::NonZero,
    );
    r.push_layer(0.5);
    r.fill_path(
        &rect(0.0, 0.0, 8.0, 8.0),
        &Paint::Solid(Color::rgb(1.0, 1.0, 1.0)),
        FillRule::NonZero,
    );
    r.pop_layer();
    // White at half alpha over opaque black gives mid grey, fully opaque.
    let p = r.pixmap().pixel(1, 1);
    assert!((p[3] - 1.0).abs() < 1e-4);
    assert!((p[0] - 0.5).abs() < 1e-4, "red channel was {}", p[0]);
}

#[test]
fn zero_alpha_group_draws_nothing() {
    let mut r = RasterBackend::new(4, 4);
    r.push_layer(0.0);
    r.fill_path(
        &rect(0.0, 0.0, 4.0, 4.0),
        &Paint::Solid(Color::BLACK),
        FillRule::NonZero,
    );
    r.pop_layer();
    assert_eq!(r.pixmap().pixel(2, 2)[3], 0.0);
}

#[test]
fn nested_groups_composite_inside_out() {
    let mut r = RasterBackend::new(4, 4);
    r.push_layer(0.5);
    r.push_layer(0.5);
    r.fill_path(
        &rect(0.0, 0.0, 4.0, 4.0),
        &Paint::Solid(Color::BLACK),
        FillRule::NonZero,
    );
    r.pop_layer();
    r.pop_layer();
    let a = r.pixmap().pixel(1, 1)[3];
    assert!((a - 0.25).abs() < 1e-4, "nested alpha was {a}");
}

#[test]
fn pop_without_push_is_ignored() {
    let mut r = RasterBackend::new(4, 4);
    r.pop_layer();
    r.fill_path(
        &rect(0.0, 0.0, 4.0, 4.0),
        &Paint::Solid(Color::BLACK),
        FillRule::NonZero,
    );
    assert_eq!(r.pixmap().pixel(0, 0)[3], 1.0);
}
