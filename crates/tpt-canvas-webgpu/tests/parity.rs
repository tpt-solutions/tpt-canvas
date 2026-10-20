//! The stencil-then-cover CPU executor must agree with the scanline raster backend
//! (apart from anti-aliased edge pixels) for the same scenes.

use tpt_canvas::testkit::fuzz;
use tpt_canvas::{Affine2, CanvasBackend, Color, FillRule, Paint, Path, PathBuilder, StrokeStyle};
use tpt_canvas_raster::RasterBackend;
use tpt_canvas_webgpu::reference::execute;
use tpt_canvas_webgpu::WebGpuBackend;

const W: u32 = 64;
const H: u32 = 64;

fn star(points: &[(f32, f32)]) -> Path {
    let mut b = PathBuilder::new();
    for (i, &(x, y)) in points.iter().enumerate() {
        if i == 0 {
            b.move_to(x, y);
        } else {
            b.line_to(x, y);
        }
    }
    b.close();
    b.build()
}

/// Draws `scene` into both backends and returns (raster alpha, reference alpha) per pixel.
fn both<F: Fn(&mut dyn CanvasBackend)>(scene: F) -> (Vec<f32>, Vec<f32>) {
    let mut raster = RasterBackend::new(W, H);
    scene(&mut raster);
    let raster_alpha: Vec<f32> = raster.pixmap().data.iter().map(|p| p[3]).collect();

    let mut gpu = WebGpuBackend::new(W, H);
    scene(&mut gpu);
    let image = execute(gpu.draw_list());
    let gpu_alpha: Vec<f32> = image.data.iter().map(|p| p[3]).collect();
    (raster_alpha, gpu_alpha)
}

fn assert_agree(name: &str, raster: &[f32], gpu: &[f32]) {
    let mismatched = raster
        .iter()
        .zip(gpu)
        .filter(|(r, g)| (**r >= 0.5) != (**g >= 0.5))
        .count();
    let total = raster.len();
    // Edge pixels are anti-aliased in the raster backend and point-sampled here.
    assert!(
        (mismatched as f32) / (total as f32) < 0.03,
        "{name}: {mismatched} of {total} pixels disagree"
    );
    let ra: f32 = raster.iter().sum();
    let ga: f32 = gpu.iter().sum();
    assert!(
        (ra - ga).abs() / ra.max(1.0) < 0.03,
        "{name}: area raster {ra} gpu {ga}"
    );
}

fn solid() -> Paint {
    Paint::Solid(Color::BLACK)
}

#[test]
fn rounded_rect_fill_agrees() {
    let mut b = PathBuilder::new();
    b.rounded_rect(8.0, 8.0, 40.0, 30.0, 10.0);
    let path = b.build();
    let (r, g) = both(|be| be.fill_path(&path, &solid(), FillRule::NonZero));
    assert_agree("rounded rect", &r, &g);
}

#[test]
fn circle_fill_agrees() {
    let mut b = PathBuilder::new();
    b.circle(32.0, 32.0, 20.0);
    let path = b.build();
    let (r, g) = both(|be| be.fill_path(&path, &solid(), FillRule::NonZero));
    assert_agree("circle", &r, &g);
}

#[test]
fn self_intersecting_pentagram_nonzero_and_evenodd_agree() {
    // {5/2} star: winding 2 in the centre, so the rules differ there.
    let pts: Vec<(f32, f32)> = (0..5)
        .map(|i| {
            let a = (i * 2) as f32 * core::f32::consts::TAU / 5.0 - core::f32::consts::FRAC_PI_2;
            (32.0 + 28.0 * libm::cosf(a), 32.0 + 28.0 * libm::sinf(a))
        })
        .collect();
    let path = star(&pts);
    let (r_nz, g_nz) = both(|be| be.fill_path(&path, &solid(), FillRule::NonZero));
    assert_agree("pentagram nonzero", &r_nz, &g_nz);
    let (r_eo, g_eo) = both(|be| be.fill_path(&path, &solid(), FillRule::EvenOdd));
    assert_agree("pentagram evenodd", &r_eo, &g_eo);
    // The rules must actually differ somewhere, or the test proves nothing.
    assert!(r_nz.iter().zip(&r_eo).any(|(a, b)| (a - b).abs() > 0.5));
}

#[test]
fn clipped_fill_agrees() {
    let mut clip = PathBuilder::new();
    clip.circle(32.0, 32.0, 22.0);
    let clip = clip.build();
    let mut fill = PathBuilder::new();
    fill.rect(0.0, 0.0, 64.0, 64.0);
    let fill = fill.build();
    let (r, g) = both(|be| {
        be.push_clip(&clip);
        be.fill_path(&fill, &solid(), FillRule::NonZero);
        be.pop_clip();
    });
    assert_agree("clipped fill", &r, &g);
}

#[test]
fn nested_transformed_stroke_agrees() {
    let mut b = PathBuilder::new();
    b.move_to(4.0, 4.0);
    b.line_to(50.0, 20.0);
    b.line_to(10.0, 58.0);
    let path = b.build();
    let style = StrokeStyle {
        width: 3.0,
        dash: vec![6.0, 3.0],
        ..StrokeStyle::default()
    };
    let (r, g) = both(|be| {
        be.push_transform(&Affine2::translate(2.0, 1.0));
        be.stroke_path(&path, &solid(), &style);
        be.pop_transform();
    });
    assert_agree("dashed stroke", &r, &g);
}

#[test]
fn points_outside_all_contours_stay_transparent() {
    let mut b = PathBuilder::new();
    b.rect(10.0, 10.0, 4.0, 4.0);
    let path = b.build();
    let (_, g) = both(|be| be.fill_path(&path, &solid(), FillRule::NonZero));
    assert_eq!(g[0], 0.0);
}

#[test]
fn random_self_intersecting_polygons_agree_with_raster() {
    fuzz(31, 60, |rng| {
        let n = 3 + rng.index(8);
        let pts: Vec<(f32, f32)> = (0..n)
            .map(|_| (rng.range(4.0, 60.0), rng.range(4.0, 60.0)))
            .collect();
        let path = star(&pts);
        let rule = if rng.index(2) == 0 {
            FillRule::NonZero
        } else {
            FillRule::EvenOdd
        };
        let (r, g) = both(|be| be.fill_path(&path, &solid(), rule));
        // Random polygons have many thin slivers, so allow a little more edge disagreement.
        let total = r.len() as f32;
        let mismatched = r
            .iter()
            .zip(&g)
            .filter(|(a, b)| (**a >= 0.5) != (**b >= 0.5))
            .count() as f32;
        assert!(
            mismatched / total < 0.06,
            "{pts:?} {rule:?}: {mismatched} mismatched"
        );
    });
}
