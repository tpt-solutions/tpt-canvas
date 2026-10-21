//! Blurred shadows on the WebGPU reference executor. The raster backend blurs the same shapes,
//! so the two should agree to within the 8-bit rounding of its blur. The rest of these checks
//! use the reference alone, to test the blur's own rules: mass, reach, symmetry, clips and scale.

use tpt_canvas::blur::box_radii;
use tpt_canvas::{Affine2, CanvasBackend, Color, FillRule, Paint, Path, PathBuilder};
use tpt_canvas_raster::RasterBackend;
use tpt_canvas_webgpu::reference::{execute, Image};
use tpt_canvas_webgpu::{Command, WebGpuBackend};

const W: u32 = 32;
const H: u32 = 32;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut b = PathBuilder::new();
    b.rect(x, y, w, h);
    b.build()
}

fn black() -> Paint {
    Paint::Solid(Color::rgba(0.0, 0.0, 0.0, 1.0))
}

/// A 16 by 16 square at (8, 8), blurred with sigma 2. Its blur reaches 4 pixels, so it stays
/// clear of the canvas edges.
fn blurred_square<B: CanvasBackend>(be: &mut B) {
    be.fill_path_blurred(
        &rect(8.0, 8.0, 16.0, 16.0),
        &black(),
        FillRule::NonZero,
        2.0,
    );
}

fn alpha_at(image: &Image, x: usize, y: usize) -> f32 {
    image.data[y * W as usize + x][3]
}

#[test]
fn blurred_shape_matches_the_raster_backend() {
    let mut raster = RasterBackend::new(W, H);
    blurred_square(&mut raster);
    let want = raster.into_pixmap().data;

    let mut gpu = WebGpuBackend::new(W, H);
    blurred_square(&mut gpu);
    assert_eq!(
        gpu.draw_list().unsupported,
        0,
        "a blurred shadow is drawn, not counted"
    );
    let got = execute(gpu.draw_list()).data;

    // Guard against both sides being blank: the raster backend must have blurred the edge.
    assert!(
        want[16 * W as usize + 7][3] > 0.0,
        "raster draws the blur's edge"
    );

    // Raster rounds each blur to 8 bits; the reference does not.
    let mut worst = 0.0f32;
    for (w, g) in want.iter().zip(&got) {
        for c in 0..4 {
            worst = worst.max((w[c] - g[c]).abs());
        }
    }
    assert!(worst < 0.01, "worst channel difference {worst}");
}

#[test]
fn blur_keeps_the_mass_of_a_shape_away_from_the_edges() {
    let mut gpu = WebGpuBackend::new(W, H);
    blurred_square(&mut gpu);
    let image = execute(gpu.draw_list());
    let mass: f32 = image.data.iter().map(|p| p[3]).sum();
    assert!(
        (mass - 256.0).abs() < 1e-2,
        "mass {mass} should equal the square's area, 256"
    );
    assert!(
        (alpha_at(&image, 16, 16) - 1.0).abs() < 1e-6,
        "the middle stays opaque"
    );
}

#[test]
fn blur_reaches_only_as_far_as_its_radii() {
    let mut gpu = WebGpuBackend::new(W, H);
    blurred_square(&mut gpu);
    let image = execute(gpu.draw_list());
    let reach = box_radii(2.0).iter().sum::<usize>();
    // The square's left edge is at x = 8, so pixels at x < 8 - reach get nothing.
    assert_eq!(alpha_at(&image, 8 - reach - 1, 16), 0.0);
    let edge = alpha_at(&image, 7, 16);
    assert!(
        edge > 0.0 && edge < 1.0,
        "the pixel just outside the edge is partly covered: {edge}"
    );
}

#[test]
fn blur_is_symmetric_about_the_shape() {
    let mut gpu = WebGpuBackend::new(W, H);
    blurred_square(&mut gpu);
    let image = execute(gpu.draw_list());
    // The square spans x 8..24 and y 8..24, so it mirrors about x = 15.5 and y = 15.5.
    for x in 0..W as usize {
        let mirror = W as usize - 1 - x;
        assert!((alpha_at(&image, x, 16) - alpha_at(&image, mirror, 16)).abs() < 1e-6);
        assert!((alpha_at(&image, x, 5) - alpha_at(&image, x, 26)).abs() < 1e-6);
    }
}

#[test]
fn clip_cuts_the_blurred_shadow_rather_than_blurring_into_it() {
    let mut be = WebGpuBackend::new(W, H);
    be.push_clip(&rect(0.0, 0.0, 16.0, 32.0));
    blurred_square(&mut be);
    be.pop_clip();
    let image = execute(be.draw_list());
    for y in 0..H as usize {
        for x in 16..W as usize {
            assert_eq!(
                alpha_at(&image, x, y),
                0.0,
                "pixel ({x}, {y}) is outside the clip"
            );
        }
    }
    assert!(
        alpha_at(&image, 12, 16) > 0.0,
        "the part inside the clip is drawn"
    );
}

#[test]
fn zero_blur_is_an_ordinary_fill() {
    let mut be = WebGpuBackend::new(W, H);
    be.fill_path_blurred(
        &rect(8.0, 8.0, 16.0, 16.0),
        &black(),
        FillRule::NonZero,
        0.0,
    );
    let list = be.into_draw_list();
    assert!(
        list.commands
            .iter()
            .all(|c| matches!(c, Command::Fill { .. })),
        "a zero sigma draws a plain fill: {:?}",
        list.commands
    );
}

#[test]
fn radii_follow_the_scale_of_the_transform() {
    let mut be = WebGpuBackend::new(W, H);
    be.push_transform(&Affine2::scale(2.0, 2.0));
    be.fill_path_blurred(&rect(4.0, 4.0, 8.0, 8.0), &black(), FillRule::NonZero, 2.0);
    let list = be.into_draw_list();
    let radii = list
        .commands
        .iter()
        .find_map(|c| match c {
            Command::BlurredFill { radii, .. } => Some(*radii),
            _ => None,
        })
        .expect("a blurred fill is recorded");
    // Sigma 2 in user units is sigma 4 in device pixels under a scale of 2.
    assert_eq!(radii, box_radii(4.0).map(|r| r as u32));
}
