//! Renders one scene to a file. The format comes from the extension: `.png`, `.svg` or `.pdf`.
//!
//! Usage: `cargo run -- out.png`

use std::env;
use std::fs;
use std::process;

use tpt_canvas::prelude::*;
use tpt_canvas::text::StrokeFont;
use tpt_canvas_pdf::PdfBackend;
use tpt_canvas_raster::RasterBackend;
use tpt_canvas_svg::SvgBackend;

const W: f32 = 320.0;
const H: f32 = 200.0;

/// The scene. Replace this with your own drawing code. It only uses `Context`, so it works
/// with every backend.
fn scene<B: CanvasBackend>(ctx: &mut Context<B>) {
    ctx.set_fill_paint(Paint::solid(Color::rgb(0.96, 0.96, 0.98)));
    ctx.fill_rect(0.0, 0.0, W, H);

    ctx.set_fill_paint(Paint::solid(Color::from_hex("#3366cc").unwrap_or(named::BLUE)));
    let mut card = PathBuilder::new();
    card.rounded_rect(24.0, 40.0, 272.0, 120.0, 16.0);
    ctx.fill_path(&card.build());

    ctx.set_glyph_source(Box::new(StrokeFont));
    ctx.set_fill_paint(Paint::solid(named::WHITE));
    // Glyphs start at the origin, so move the label into the card.
    let mut label = StrokeFont::layout("HELLO", 28.0);
    for g in &mut label.glyphs {
        g.pos.x += 44.0;
        g.pos.y += 110.0;
    }
    ctx.draw_glyphs(&label);
}

fn main() {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: {} <out.png|out.svg|out.pdf>", env!("CARGO_PKG_NAME"));
        process::exit(2);
    };

    let bytes: Vec<u8> = if path.ends_with(".svg") {
        let mut ctx = Context::new(SvgBackend::new(W, H));
        scene(&mut ctx);
        ctx.into_backend().finish().into_bytes()
    } else if path.ends_with(".pdf") {
        let mut ctx = Context::new(PdfBackend::new(W, H));
        scene(&mut ctx);
        ctx.into_backend().finish()
    } else if path.ends_with(".png") {
        let mut ctx = Context::new(RasterBackend::new(W as u32, H as u32));
        scene(&mut ctx);
        ctx.into_backend().into_pixmap().to_png()
    } else {
        eprintln!("unknown output format for {path}: use .png, .svg or .pdf");
        process::exit(2);
    };

    if let Err(e) = fs::write(&path, bytes) {
        eprintln!("could not write {path}: {e}");
        process::exit(1);
    }
    println!("wrote {path}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_scene_has_ink() {
        let mut ctx = Context::new(RasterBackend::new(W as u32, H as u32));
        scene(&mut ctx);
        let ink: f32 = ctx.backend().pixmap().data.iter().map(|p| p[3]).sum();
        assert!(ink > 1000.0);
    }
}
