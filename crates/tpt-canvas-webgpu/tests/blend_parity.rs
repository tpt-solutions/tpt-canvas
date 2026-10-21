//! Blend modes on the WebGPU reference executor must match the raster backend. The rectangles
//! sit on whole pixels, so coverage is exact on both and the pixels can be compared tightly.

use tpt_canvas::{BlendMode, CanvasBackend, Color, FillRule, Paint, Path, PathBuilder};
use tpt_canvas_raster::RasterBackend;
use tpt_canvas_webgpu::reference::execute;
use tpt_canvas_webgpu::WebGpuBackend;

const W: u32 = 32;
const H: u32 = 32;

const ALL_MODES: [BlendMode; 12] = [
    BlendMode::Normal,
    BlendMode::Multiply,
    BlendMode::Screen,
    BlendMode::Overlay,
    BlendMode::Darken,
    BlendMode::Lighten,
    BlendMode::Difference,
    BlendMode::Exclusion,
    BlendMode::Hue,
    BlendMode::Saturation,
    BlendMode::Color,
    BlendMode::Luminosity,
];

fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut b = PathBuilder::new();
    b.rect(x, y, w, h);
    b.build()
}

/// An opaque backdrop, then a half-transparent source drawn in `mode`.
fn scene<B: CanvasBackend>(be: &mut B, mode: BlendMode) {
    be.fill_path(
        &rect(0.0, 0.0, 32.0, 32.0),
        &Paint::Solid(Color::rgba(0.2, 0.6, 0.9, 1.0)),
        FillRule::NonZero,
    );
    be.set_blend_mode(mode);
    be.fill_path(
        &rect(8.0, 8.0, 16.0, 16.0),
        &Paint::Solid(Color::rgba(0.9, 0.4, 0.1, 0.7)),
        FillRule::NonZero,
    );
    be.set_blend_mode(BlendMode::Normal);
}

#[test]
fn every_blend_mode_matches_the_raster_backend() {
    for mode in ALL_MODES {
        let mut raster = RasterBackend::new(W, H);
        scene(&mut raster, mode);
        let want = raster.into_pixmap().data;

        let mut gpu = WebGpuBackend::new(W, H);
        scene(&mut gpu, mode);
        assert_eq!(
            gpu.draw_list().unsupported,
            0,
            "{mode:?} should not be counted"
        );
        let got = execute(gpu.draw_list()).data;

        for (i, (w, g)) in want.iter().zip(&got).enumerate() {
            for c in 0..4 {
                assert!(
                    (w[c] - g[c]).abs() < 1e-4,
                    "{mode:?} pixel {i} channel {c}: raster {w:?}, reference {g:?}"
                );
            }
        }
    }
}
