//! Glyph runs on WebGPU. A `GlyphSource` turns each glyph into a fill before any backend sees it,
//! so WebGPU draws text the way raster does. Without a source, a run is counted, as on every
//! backend.

use tpt_canvas::text::{GlyphId, ShapedRun, StrokeFont};
use tpt_canvas::{Color, Context, GlyphSource, Paint, Path, PathBuilder, Vec2};
use tpt_canvas_raster::RasterBackend;
use tpt_canvas_webgpu::reference::execute;
use tpt_canvas_webgpu::WebGpuBackend;

const W: u32 = 64;
const H: u32 = 64;

/// Two synthetic glyphs in a 1000-unit em: id 1 is a full square, so its fill is exact on whole
/// pixels; id 2 is a triangle.
struct BlockFont;

impl GlyphSource for BlockFont {
    fn units_per_em(&self, font_id: u32) -> Option<f32> {
        (font_id == 7).then_some(1000.0)
    }

    fn outline(&self, font_id: u32, glyph: GlyphId) -> Option<Path> {
        if font_id != 7 {
            return None;
        }
        let mut b = PathBuilder::new();
        match glyph {
            1 => b.rect(0.0, 0.0, 1000.0, 1000.0),
            2 => {
                b.move_to(0.0, 0.0);
                b.line_to(1000.0, 0.0);
                b.line_to(500.0, 1000.0);
                b.close();
            }
            _ => return None,
        }
        Some(b.build())
    }
}

fn square_run() -> ShapedRun {
    // Font size 20: the em is 20 px, so each square covers 20 by 20 pixels above the baseline.
    let mut run = ShapedRun::new(7, 20.0);
    run.push(1, Vec2::new(10.0, 40.0));
    run.push(1, Vec2::new(34.0, 40.0));
    run
}

#[test]
fn glyph_runs_match_the_raster_backend() {
    let mut raster = RasterBackend::new(W, H);
    {
        let mut ctx = Context::new(&mut raster);
        ctx.set_glyph_source(Box::new(BlockFont));
        ctx.set_fill_paint(Paint::Solid(Color::BLACK));
        ctx.draw_glyphs(&square_run());
    }
    let want = raster.into_pixmap().data;

    let mut gpu = WebGpuBackend::new(W, H);
    {
        let mut ctx = Context::new(&mut gpu);
        ctx.set_glyph_source(Box::new(BlockFont));
        ctx.set_fill_paint(Paint::Solid(Color::BLACK));
        ctx.draw_glyphs(&square_run());
    }
    assert_eq!(
        gpu.draw_list().unsupported,
        0,
        "glyphs with a source are drawn"
    );
    let got = execute(gpu.draw_list()).data;

    let mut worst = 0.0f32;
    for (w, g) in want.iter().zip(&got) {
        for c in 0..4 {
            worst = worst.max((w[c] - g[c]).abs());
        }
    }
    assert!(worst < 1e-3, "worst channel difference {worst}");
    // The first square covers x 10..30 and y 20..40 (y down), so (15, 30) is inside it.
    assert!(
        got[30 * W as usize + 15][3] > 0.99,
        "the first glyph is inside the fill"
    );
}

#[test]
fn a_run_without_a_source_is_counted_as_on_raster() {
    let mut raster = RasterBackend::new(W, H);
    Context::new(&mut raster).draw_glyphs(&square_run());
    let mut gpu = WebGpuBackend::new(W, H);
    Context::new(&mut gpu).draw_glyphs(&square_run());
    assert_eq!(raster.unsupported(), 1);
    assert_eq!(gpu.draw_list().unsupported, 1);
    assert!(execute(gpu.draw_list()).data.iter().all(|p| p[3] == 0.0));
}

#[test]
fn the_built_in_stroke_font_draws_on_webgpu() {
    let mut gpu = WebGpuBackend::new(W, H);
    {
        let mut ctx = Context::new(&mut gpu);
        ctx.set_glyph_source(Box::new(StrokeFont));
        ctx.set_fill_paint(Paint::Solid(Color::BLACK));
        ctx.translate(4.0, 50.0);
        ctx.draw_glyphs(&StrokeFont::layout("A1", 40.0));
    }
    assert_eq!(gpu.draw_list().unsupported, 0);
    let got = execute(gpu.draw_list()).data;
    let ink: f32 = got.iter().map(|p| p[3]).sum();
    assert!(ink > 50.0, "the strokes leave ink on the canvas: {ink}");
}
