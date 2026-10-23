//! One drawing routine, three outputs: SVG, PDF and PNG. Files go to `gallery-out/`.

use std::fs;
use std::path::Path as FsPath;

use tpt_canvas::path;
use tpt_canvas::prelude::*;
use tpt_canvas_pdf::PdfBackend;
use tpt_canvas_raster::RasterBackend;
use tpt_canvas_svg::SvgBackend;

const W: f32 = 240.0;
const H: f32 = 160.0;

/// The scene. It only uses the `Context` API, so every backend gets the same calls.
fn scene<B: CanvasBackend>(ctx: &mut Context<B>) {
    // Gradient banner.
    ctx.set_fill_paint(Paint::linear(
        Vec2::new(0.0, 0.0),
        Vec2::new(W, 0.0),
        &[(0.0, named::BLUE), (1.0, named::CYAN)],
    ));
    let mut banner = PathBuilder::new();
    banner.rounded_rect(12.0, 12.0, 216.0, 56.0, 14.0);
    ctx.fill_path(&banner.build());

    // A translucent disc and a star drawn with the path! macro.
    ctx.save();
    ctx.set_global_alpha(0.6);
    ctx.set_fill_paint(Paint::solid(named::YELLOW));
    ctx.fill_circle(60.0, 110.0, 30.0);
    ctx.restore();

    ctx.save();
    ctx.translate(170.0, 112.0);
    ctx.set_fill_paint(Paint::solid(named::MAGENTA));
    ctx.fill_path(
        &path! { M 0 -28 L 8 -9 L 27 -9 L 12 4 L 18 24 L 0 12 L -18 24 L -12 4 L -27 -9 L -8 -9 Z },
    );
    ctx.restore();

    // A dashed outline.
    ctx.set_stroke_paint(Paint::solid(named::BLACK));
    ctx.set_stroke_style(StrokeStyle {
        width: 2.0,
        dash: vec![6.0, 4.0],
        ..StrokeStyle::default()
    });
    ctx.stroke_rect(12.0, 80.0, 216.0, 68.0);
}

fn main() -> std::io::Result<()> {
    let out = FsPath::new("gallery-out");
    fs::create_dir_all(out)?;

    let mut svg = Context::new(SvgBackend::new(W, H));
    scene(&mut svg);
    fs::write(out.join("scene.svg"), svg.into_backend().finish())?;

    let mut pdf = Context::new(PdfBackend::new(W, H));
    scene(&mut pdf);
    fs::write(out.join("scene.pdf"), pdf.into_backend().finish())?;

    let mut raster = Context::new(RasterBackend::new(W as u32, H as u32));
    scene(&mut raster);
    fs::write(
        out.join("scene.png"),
        raster.into_backend().into_pixmap().to_png(),
    )?;

    println!(
        "wrote scene.svg, scene.pdf and scene.png to {}",
        out.display()
    );
    Ok(())
}
