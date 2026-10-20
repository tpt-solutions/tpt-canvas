//! A bar chart with axes and gridlines, written as SVG to `gallery-out/bar_chart.svg`.

use std::fs;
use std::path::Path as FsPath;

use tpt_canvas::prelude::*;
use tpt_canvas_svg::SvgBackend;

const W: f32 = 320.0;
const H: f32 = 200.0;
const LEFT: f32 = 36.0;
const BOTTOM: f32 = 170.0;
const PLOT_W: f32 = 264.0;
const PLOT_H: f32 = 150.0;

fn main() -> std::io::Result<()> {
    let values = [42.0_f32, 77.0, 58.0, 91.0, 64.0, 35.0];
    let max = 100.0_f32;
    let slot = PLOT_W / values.len() as f32;

    let mut ctx = Context::new(SvgBackend::new(W, H));

    // Gridlines every 25 units.
    ctx.set_stroke_paint(Paint::solid(named::GRAY.lighten(0.6)));
    ctx.set_stroke_style(StrokeStyle {
        width: 1.0,
        ..StrokeStyle::default()
    });
    for i in 0..=4 {
        let y = BOTTOM - PLOT_H * i as f32 / 4.0;
        let mut line = PathBuilder::new();
        line.move_to(LEFT, y);
        line.line_to(LEFT + PLOT_W, y);
        ctx.stroke_path(&line.build());
    }

    // Bars, each a gradient from its base colour to a lighter tint.
    let base = Color::from_hex("#3366cc").unwrap_or(named::BLUE);
    for (i, v) in values.iter().enumerate() {
        let h = PLOT_H * v / max;
        let x = LEFT + slot * i as f32 + slot * 0.2;
        let y = BOTTOM - h;
        let w = slot * 0.6;
        ctx.set_fill_paint(Paint::linear(
            Vec2::new(0.0, y),
            Vec2::new(0.0, BOTTOM),
            &[(0.0, base.lighten(0.35)), (1.0, base)],
        ));
        ctx.fill_rect(x, y, w, h);
    }

    // Axes on top of the bars.
    ctx.set_stroke_paint(Paint::solid(named::BLACK));
    ctx.set_stroke_style(StrokeStyle {
        width: 1.5,
        ..StrokeStyle::default()
    });
    let mut axes = PathBuilder::new();
    axes.move_to(LEFT, BOTTOM - PLOT_H);
    axes.line_to(LEFT, BOTTOM);
    axes.line_to(LEFT + PLOT_W, BOTTOM);
    ctx.stroke_path(&axes.build());

    let out = FsPath::new("gallery-out");
    fs::create_dir_all(out)?;
    fs::write(out.join("bar_chart.svg"), ctx.into_backend().finish())?;
    println!("wrote {}", out.join("bar_chart.svg").display());
    Ok(())
}
