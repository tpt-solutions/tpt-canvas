//! The README picture: gradients, a blurred shadow and multiply blending, drawn once and
//! written as SVG and PNG. Run with `cargo run -p tpt-gallery --example hero`.
//! Output goes to `docs/images/`.

use std::fs;
use std::path::Path as FsPath;

use tpt_canvas::prelude::*;
use tpt_canvas::{BlendMode, Shadow};
use tpt_canvas_pdf::PdfBackend;
use tpt_canvas_raster::RasterBackend;
use tpt_canvas_svg::SvgBackend;

const W: f32 = 360.0;
const H: f32 = 200.0;

/// The scene. It only uses the `Context` API, so every backend gets the same calls.
fn scene<B: CanvasBackend>(ctx: &mut Context<B>) {
    // Background: a vertical gradient.
    ctx.set_fill_paint(Paint::linear(
        Vec2::new(0.0, 0.0),
        Vec2::new(0.0, H),
        &[
            (0.0, Color::rgb(0.97, 0.95, 0.90)),
            (1.0, Color::rgb(0.78, 0.84, 0.94)),
        ],
    ));
    ctx.fill_rect(0.0, 0.0, W, H);

    // Three overlapping discs, multiplied together.
    ctx.set_blend_mode(BlendMode::Multiply);
    let discs = [
        (130.0, 80.0, Color::rgb(0.0, 0.85, 0.95)),
        (190.0, 80.0, Color::rgb(0.95, 0.2, 0.7)),
        (160.0, 128.0, Color::rgb(0.98, 0.85, 0.2)),
    ];
    for (x, y, c) in discs {
        let mut b = PathBuilder::new();
        b.circle(x, y, 58.0);
        ctx.set_fill_paint(Paint::solid(c));
        ctx.fill_path(&b.build());
    }
    ctx.set_blend_mode(BlendMode::Normal);

    // A card with a soft drop shadow, filled with a gradient.
    ctx.set_shadow(Shadow {
        color: Color::rgba(0.0, 0.0, 0.0, 0.7),
        offset: Vec2::new(6.0, 8.0),
        blur: 12.0,
    });
    ctx.set_fill_paint(Paint::linear(
        Vec2::new(250.0, 30.0),
        Vec2::new(340.0, 170.0),
        &[
            (0.0, Color::rgb(0.95, 0.95, 0.98)),
            (1.0, Color::rgb(0.7, 0.75, 0.9)),
        ],
    ));
    let mut card = PathBuilder::new();
    card.rounded_rect(250.0, 30.0, 90.0, 140.0, 14.0);
    ctx.fill_path(&card.build());
    ctx.clear_shadow();

    // A dashed outline around the card.
    ctx.set_stroke_paint(Paint::solid(Color::rgb(0.2, 0.22, 0.3)));
    ctx.set_stroke_style(StrokeStyle {
        width: 1.5,
        dash: vec![5.0, 4.0],
        ..StrokeStyle::default()
    });
    let mut outline = PathBuilder::new();
    outline.rounded_rect(250.0, 30.0, 90.0, 140.0, 14.0);
    ctx.stroke_path(&outline.build());
}

fn main() -> std::io::Result<()> {
    let out = FsPath::new("docs/images");
    fs::create_dir_all(out)?;

    let mut svg = Context::new(SvgBackend::new(W, H));
    scene(&mut svg);
    fs::write(out.join("hero.svg"), svg.into_backend().finish())?;

    let mut raster = Context::new(RasterBackend::new(W as u32, H as u32));
    scene(&mut raster);
    let png = raster.into_backend().into_pixmap().to_png();
    fs::write(out.join("hero.png"), png)?;

    // The same scene as a PDF, for checking the blurred shadow as an embedded image.
    let mut pdf = Context::new(PdfBackend::new(W, H));
    scene(&mut pdf);
    fs::create_dir_all("gallery-out")?;
    fs::write("gallery-out/hero.pdf", pdf.into_backend().finish())?;

    println!("wrote hero.svg and hero.png to docs/images, and hero.pdf to gallery-out");
    Ok(())
}
