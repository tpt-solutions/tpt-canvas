//! Pattern fills on the WebGPU reference executor must match the raster backend. The shapes sit
//! on whole pixels, so coverage is exact on both and the pixels can be compared tightly.

use tpt_canvas::{Affine2, CanvasBackend, FillRule, ImageId, Paint, Path, PathBuilder};
use tpt_canvas_raster::{Pixmap, RasterBackend};
use tpt_canvas_webgpu::reference::execute;
use tpt_canvas_webgpu::WebGpuBackend;

const W: u32 = 40;
const H: u32 = 40;

fn tile() -> Pixmap {
    // A 3 by 2 image with translucent pixels, so the alpha path is exercised too.
    let rgba = [
        255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, //
        255, 255, 0, 64, 255, 0, 255, 255, 0, 255, 255, 255,
    ];
    Pixmap::from_rgba8(3, 2, &rgba).expect("valid size")
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut b = PathBuilder::new();
    b.rect(x, y, w, h);
    b.build()
}

fn paint(transform: Affine2) -> Paint {
    Paint::Pattern {
        image: ImageId(4),
        transform,
    }
}

fn assert_same(name: &str, transform: Affine2) {
    let image = tile();
    let mut raster = RasterBackend::new(W, H);
    raster.add_image(ImageId(4), image.clone());
    raster.fill_path(
        &rect(4.0, 4.0, 32.0, 32.0),
        &paint(transform),
        FillRule::NonZero,
    );
    let want = raster.into_pixmap().data;

    let mut gpu = WebGpuBackend::new(W, H);
    assert!(gpu.add_image(ImageId(4), image.width, image.height, image.data.clone()));
    gpu.fill_path(
        &rect(4.0, 4.0, 32.0, 32.0),
        &paint(transform),
        FillRule::NonZero,
    );
    assert_eq!(
        gpu.draw_list().unsupported,
        0,
        "{name}: a registered pattern is drawn"
    );
    let got = execute(gpu.draw_list()).data;

    for (i, (w, g)) in want.iter().zip(&got).enumerate() {
        for c in 0..4 {
            assert!(
                (w[c] - g[c]).abs() < 1e-4,
                "{name} pixel {i} channel {c}: raster {w:?}, reference {g:?}"
            );
        }
    }
}

#[test]
fn scaled_pattern_matches_the_raster_backend() {
    assert_same("scale 6", Affine2::scale(6.0, 6.0));
}

#[test]
fn rotated_and_offset_pattern_matches_the_raster_backend() {
    let t = Affine2::translate(3.0, 5.0) * Affine2::rotate(0.4) * Affine2::scale(5.0, 5.0);
    assert_same("rotated", t);
}

#[test]
fn unregistered_pattern_is_counted_and_draws_nothing() {
    let mut gpu = WebGpuBackend::new(W, H);
    gpu.fill_path(
        &rect(0.0, 0.0, 8.0, 8.0),
        &paint(Affine2::IDENTITY),
        FillRule::NonZero,
    );
    assert_eq!(gpu.draw_list().unsupported, 1);
    assert!(gpu.draw_list().commands.is_empty());
}

#[test]
fn a_pixel_count_that_does_not_match_is_refused() {
    let mut gpu = WebGpuBackend::new(W, H);
    assert!(!gpu.add_image(ImageId(1), 2, 2, vec![[0.0; 4]; 3]));
    assert!(gpu.draw_list().images.is_empty());
}
