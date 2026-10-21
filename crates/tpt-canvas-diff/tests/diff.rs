//! The raster backend and the WebGPU reference executor should agree on coverage, apart
//! from anti-aliased edge pixels.

use tpt_canvas::{CanvasBackend, Color, FillRule, Paint, PathBuilder};
use tpt_canvas_diff::{diff_pixels, diff_raster_and_webgpu};

fn black() -> Paint {
    Paint::Solid(Color::BLACK)
}

#[test]
fn identical_buffers_have_no_difference() {
    let a = vec![[0.0, 0.0, 0.0, 1.0]; 16];
    let d = diff_pixels(&a, &a);
    assert_eq!(d.coverage_mismatch, 0);
    assert_eq!(d.max_channel_delta, 0.0);
    assert_eq!(d.mean_channel_delta, 0.0);
}

#[test]
fn coverage_flip_is_counted() {
    let a = vec![[0.0, 0.0, 0.0, 1.0], [0.0; 4]];
    let b = vec![[0.0; 4], [0.0; 4]];
    let d = diff_pixels(&a, &b);
    assert_eq!(d.coverage_mismatch, 1);
    assert_eq!(d.coverage_mismatch_ratio(), 0.5);
    assert_eq!(d.max_channel_delta, 1.0);
}

#[test]
fn circle_agrees_between_raster_and_webgpu() {
    let d = diff_raster_and_webgpu(64, 64, |be: &mut dyn CanvasBackend| {
        let mut b = PathBuilder::new();
        b.circle(32.0, 32.0, 20.0);
        be.fill_path(&b.build(), &black(), FillRule::NonZero);
    });
    assert!(
        d.coverage_mismatch_ratio() < 0.03,
        "circle: {} of {} pixels disagree",
        d.coverage_mismatch,
        d.pixels
    );
}

#[test]
fn rounded_rect_agrees_between_raster_and_webgpu() {
    let d = diff_raster_and_webgpu(64, 64, |be: &mut dyn CanvasBackend| {
        let mut b = PathBuilder::new();
        b.rounded_rect(8.0, 8.0, 40.0, 30.0, 10.0);
        be.fill_path(&b.build(), &black(), FillRule::NonZero);
    });
    assert!(d.coverage_mismatch_ratio() < 0.03, "{d:?}");
}

#[test]
fn different_scenes_are_reported() {
    // Same shape size, moved: the coverage must disagree on many pixels.
    let at = |x: f32| {
        move |be: &mut dyn CanvasBackend| {
            let mut b = PathBuilder::new();
            b.rect(x, 8.0, 20.0, 20.0);
            be.fill_path(&b.build(), &black(), FillRule::NonZero);
        }
    };
    let (ra, _) = tpt_canvas_diff::render_raster_and_webgpu(64, 64, at(8.0));
    let (rb, _) = tpt_canvas_diff::render_raster_and_webgpu(64, 64, at(30.0));
    let d = diff_pixels(&ra, &rb);
    assert!(d.coverage_mismatch > 300, "{d:?}");
}
