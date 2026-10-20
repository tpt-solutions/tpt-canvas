//! A ruled invoice-style table written as a single-page PDF to `gallery-out/pdf_table.pdf`.
//! Text is not drawn yet (no font outlines exist), so the table shows the layout only.

use std::fs;
use std::path::Path as FsPath;

use tpt_canvas::prelude::*;
use tpt_canvas_pdf::PdfBackend;

const W: f32 = 420.0;
const H: f32 = 300.0;
const LEFT: f32 = 40.0;
const TOP: f32 = 40.0;
const ROW_H: f32 = 28.0;
const COLS: [f32; 4] = [0.0, 200.0, 280.0, 350.0];

fn main() -> std::io::Result<()> {
    let mut ctx = Context::new(PdfBackend::new(W, H));

    // Header band with a gradient.
    ctx.set_fill_paint(Paint::linear(
        Vec2::new(LEFT, TOP),
        Vec2::new(W - LEFT, TOP),
        &[(0.0, named::BLUE), (1.0, named::CYAN)],
    ));
    ctx.fill_rect(LEFT, TOP, W - 2.0 * LEFT, ROW_H);

    // Alternating row shading.
    for row in 1..6 {
        let y = TOP + ROW_H * row as f32;
        let tint = if row % 2 == 0 {
            named::GRAY.lighten(0.85)
        } else {
            named::WHITE
        };
        ctx.set_fill_paint(Paint::solid(tint));
        ctx.fill_rect(LEFT, y, W - 2.0 * LEFT, ROW_H);
    }

    // Grid lines.
    ctx.set_stroke_paint(Paint::solid(named::GRAY));
    ctx.set_stroke_style(StrokeStyle {
        width: 0.75,
        ..StrokeStyle::default()
    });
    for row in 0..=6 {
        let y = TOP + ROW_H * row as f32;
        let mut line = PathBuilder::new();
        line.move_to(LEFT, y);
        line.line_to(W - LEFT, y);
        ctx.stroke_path(&line.build());
    }
    for x in COLS {
        let mut line = PathBuilder::new();
        line.move_to(LEFT + x, TOP);
        line.line_to(LEFT + x, TOP + ROW_H * 6.0);
        ctx.stroke_path(&line.build());
    }

    let out = FsPath::new("gallery-out");
    fs::create_dir_all(out)?;
    fs::write(out.join("pdf_table.pdf"), ctx.into_backend().finish())?;
    println!("wrote {}", out.join("pdf_table.pdf").display());
    Ok(())
}
