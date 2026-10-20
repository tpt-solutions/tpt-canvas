//! Glyph drawing through a `GlyphSource`, checked at the pixel level.

use tpt_canvas::math::Vec2;
use tpt_canvas::path::Path;
use tpt_canvas::text::{GlyphId, ShapedRun};
use tpt_canvas::{Color, Context, GlyphSource, Paint, PathBuilder};
use tpt_canvas_raster::RasterBackend;

/// Two synthetic glyphs in a 1000-unit em: id 1 is a full square, id 2 a triangle.
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

fn render(run: &ShapedRun) -> RasterBackend {
    let mut ctx = Context::new(RasterBackend::new(64, 64));
    ctx.set_glyph_source(Box::new(BlockFont));
    ctx.set_fill_paint(Paint::Solid(Color::BLACK));
    ctx.draw_glyphs(run);
    ctx.into_backend()
}

#[test]
fn square_glyph_fills_its_em_box_above_the_baseline() {
    // Font size 20 means the em is 20 px. The origin sits at (10, 40) with y down, so the
    // square covers x in 10..30 and y in 20..40.
    let mut run = ShapedRun::new(7, 20.0);
    run.push(1, Vec2::new(10.0, 40.0));
    let rb = render(&run);
    let p = rb.pixmap();
    assert!(p.pixel(15, 30)[3] > 0.99, "inside the glyph");
    assert!(p.pixel(15, 15)[3] < 0.01, "above the glyph");
    assert!(p.pixel(15, 45)[3] < 0.01, "below the baseline");
    assert!(p.pixel(5, 30)[3] < 0.01, "left of the glyph");
}

#[test]
fn several_glyphs_advance_across_the_run() {
    let mut run = ShapedRun::new(7, 10.0);
    run.push(1, Vec2::new(4.0, 30.0));
    run.push(2, Vec2::new(20.0, 30.0));
    let rb = render(&run);
    let p = rb.pixmap();
    assert!(p.pixel(6, 25)[3] > 0.99, "first glyph");
    // The triangle's base spans its full em width at the baseline row.
    assert!(p.pixel(25, 28)[3] > 0.99, "triangle near its base");
    assert!(p.pixel(30, 21)[3] < 0.01, "triangle apex region is narrow");
}

#[test]
fn unknown_font_draws_nothing() {
    let mut run = ShapedRun::new(99, 20.0);
    run.push(1, Vec2::new(10.0, 40.0));
    let rb = render(&run);
    assert!(rb.pixmap().data.iter().all(|p| p[3] == 0.0));
}

#[test]
fn without_a_source_the_backend_gets_the_run() {
    // With no source, Context forwards the run, which the raster backend ignores.
    let mut run = ShapedRun::new(7, 20.0);
    run.push(1, Vec2::new(10.0, 40.0));
    let mut ctx = Context::new(RasterBackend::new(64, 64));
    ctx.draw_glyphs(&run);
    assert!(ctx.backend().pixmap().data.iter().all(|p| p[3] == 0.0));
}
