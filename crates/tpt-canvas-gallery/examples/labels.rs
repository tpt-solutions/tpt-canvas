//! Text labels with the built-in stroke font, drawn to SVG and PNG. Files go to `gallery-out/`.
//! Run with `cargo run -p tpt-canvas-gallery --example labels`.

use std::fs;
use std::path::Path as FsPath;

use tpt_canvas::prelude::*;
use tpt_canvas::text::{ShapedRun, StrokeFont};
use tpt_canvas_raster::RasterBackend;
use tpt_canvas_svg::SvgBackend;

const W: f32 = 360.0;
const H: f32 = 150.0;

/// Moves every glyph of a run by `(dx, dy)`, so each label gets its own baseline.
fn at(mut run: ShapedRun, dx: f32, dy: f32) -> ShapedRun {
    for g in &mut run.glyphs {
        g.pos.x += dx;
        g.pos.y += dy;
    }
    run
}

/// The labels. Text goes through the same `Context` calls as shapes.
fn scene<B: CanvasBackend>(ctx: &mut Context<B>) {
    ctx.set_glyph_source(Box::new(StrokeFont));

    ctx.set_fill_paint(Paint::solid(Color::rgb(0.96, 0.96, 0.98)));
    ctx.fill_rect(0.0, 0.0, W, H);

    ctx.set_fill_paint(Paint::solid(Color::rgb(0.1, 0.15, 0.35)));
    ctx.draw_glyphs(&at(StrokeFont::layout("TPT CANVAS", 30.0), 16.0, 40.0));

    ctx.set_fill_paint(Paint::solid(Color::rgb(0.85, 0.3, 0.2)));
    ctx.draw_glyphs(&at(StrokeFont::layout("0123456789", 22.0), 16.0, 80.0));

    ctx.set_fill_paint(Paint::solid(Color::rgb(0.2, 0.55, 0.3)));
    ctx.draw_glyphs(&at(StrokeFont::layout("ABCDEFGHIJKLM", 18.0), 16.0, 115.0));
    ctx.draw_glyphs(&at(StrokeFont::layout("NOPQRSTUVWXYZ", 18.0), 16.0, 138.0));
}

fn main() -> std::io::Result<()> {
    let out = FsPath::new("gallery-out");
    fs::create_dir_all(out)?;

    let mut svg = Context::new(SvgBackend::new(W, H));
    scene(&mut svg);
    fs::write(out.join("labels.svg"), svg.into_backend().finish())?;

    let mut raster = Context::new(RasterBackend::new(W as u32, H as u32));
    scene(&mut raster);
    fs::write(
        out.join("labels.png"),
        raster.into_backend().into_pixmap().to_png(),
    )?;

    println!("wrote labels.svg and labels.png to gallery-out");
    Ok(())
}
