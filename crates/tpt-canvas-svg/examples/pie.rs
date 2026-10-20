//! Pie chart: wedges drawn with arcs and filled with solid colours.
//! Run with `cargo run -p tpt-canvas-svg --example pie > pie.svg`.

use tpt_canvas::{Color, Context, Paint, PathBuilder, StrokeStyle};
use tpt_canvas_svg::SvgBackend;

fn main() {
    let values = [
        (0.40, Color::rgb(0.90, 0.35, 0.25)),
        (0.25, Color::rgb(0.25, 0.65, 0.35)),
        (0.20, Color::rgb(0.30, 0.45, 0.90)),
        (0.15, Color::rgb(0.95, 0.75, 0.2)),
    ];
    let mut ctx = Context::new(SvgBackend::new(240.0, 240.0));
    let (cx, cy, r) = (120.0f32, 120.0f32, 96.0f32);
    let mut angle = -core::f32::consts::FRAC_PI_2;
    for (share, color) in values {
        let sweep = share * core::f32::consts::TAU;
        let mut b = PathBuilder::new();
        b.move_to(cx, cy);
        b.arc(cx, cy, r, angle, angle + sweep, false);
        b.close();
        let wedge = b.build();
        ctx.set_fill_paint(Paint::Solid(color));
        ctx.fill_path(&wedge);
        ctx.set_stroke_paint(Paint::Solid(Color::WHITE));
        ctx.set_stroke_style(StrokeStyle {
            width: 2.0,
            ..StrokeStyle::default()
        });
        ctx.stroke_path(&wedge);
        angle += sweep;
    }
    print!("{}", ctx.into_backend().finish());
}
