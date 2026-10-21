//! Compares one scene drawn through two backends, pixel by pixel.
//!
//! The raster backend is the reference for anti-aliased output. The WebGPU backend is
//! checked through its CPU executor, which point-samples coverage. Edge pixels therefore
//! differ by design, so [`Diff`] reports the size of each kind of disagreement rather
//! than a single pass or fail.
//!
//! SVG and PDF output cannot be rasterized inside this workspace, so they are not compared yet.

#![warn(missing_docs)]

use tpt_canvas::CanvasBackend;
use tpt_canvas_raster::RasterBackend;
use tpt_canvas_webgpu::reference::execute;
use tpt_canvas_webgpu::WebGpuBackend;

/// Premultiplied RGBA, one entry per pixel, row-major.
pub type Pixels = Vec<[f32; 4]>;

/// Summary of how two renders of the same scene differ.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Diff {
    /// Total pixels compared.
    pub pixels: usize,
    /// Pixels where exactly one side is at least half covered (alpha crosses 0.5).
    pub coverage_mismatch: usize,
    /// Largest per-channel absolute difference over all pixels.
    pub max_channel_delta: f32,
    /// Mean per-channel absolute difference over all pixels and channels.
    pub mean_channel_delta: f32,
}

impl Diff {
    /// Fraction of pixels whose coverage disagrees, in `[0, 1]`.
    pub fn coverage_mismatch_ratio(&self) -> f32 {
        if self.pixels == 0 {
            0.0
        } else {
            self.coverage_mismatch as f32 / self.pixels as f32
        }
    }
}

/// Compares two equal-length pixel buffers.
///
/// # Panics
///
/// Panics if the buffers have different lengths.
pub fn diff_pixels(a: &[[f32; 4]], b: &[[f32; 4]]) -> Diff {
    assert_eq!(a.len(), b.len(), "pixel buffers differ in length");
    let mut coverage_mismatch = 0;
    let mut max_channel_delta = 0.0f32;
    let mut sum = 0.0f64;
    for (p, q) in a.iter().zip(b) {
        if (p[3] >= 0.5) != (q[3] >= 0.5) {
            coverage_mismatch += 1;
        }
        for c in 0..4 {
            let d = (p[c] - q[c]).abs();
            max_channel_delta = max_channel_delta.max(d);
            sum += d as f64;
        }
    }
    let channels = (a.len() * 4).max(1);
    Diff {
        pixels: a.len(),
        coverage_mismatch,
        max_channel_delta,
        mean_channel_delta: (sum / channels as f64) as f32,
    }
}

/// Draws `scene` on the raster backend and on the WebGPU reference executor.
///
/// Returns both renders as premultiplied RGBA, raster first.
pub fn render_raster_and_webgpu<F>(width: u32, height: u32, scene: F) -> (Pixels, Pixels)
where
    F: Fn(&mut dyn CanvasBackend),
{
    let mut raster = RasterBackend::new(width, height);
    scene(&mut raster);
    let raster_pixels = raster.into_pixmap().data;

    let mut gpu = WebGpuBackend::new(width, height);
    scene(&mut gpu);
    let gpu_pixels = execute(gpu.draw_list()).data;

    (raster_pixels, gpu_pixels)
}

/// Renders `scene` through both backends and reports how they differ.
pub fn diff_raster_and_webgpu<F>(width: u32, height: u32, scene: F) -> Diff
where
    F: Fn(&mut dyn CanvasBackend),
{
    let (raster, gpu) = render_raster_and_webgpu(width, height, scene);
    diff_pixels(&raster, &gpu)
}
