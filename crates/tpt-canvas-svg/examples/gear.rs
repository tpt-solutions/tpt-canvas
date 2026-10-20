//! Gear outline built from straight teeth and circular arcs, using the arc converter.
//! Run with `cargo run -p tpt-canvas-svg --example gear > gear.svg`.

use tpt_canvas::{Color, Context, FillRule, Paint, PathBuilder};
use tpt_canvas_svg::SvgBackend;

fn main() {
    let (cx, cy) = (120.0f32, 120.0f32);
    let (r_out, r_in, teeth) = (90.0f32, 74.0f32, 12);
    let step = core::f32::consts::TAU / teeth as f32;
    let mut b = PathBuilder::new();
    for i in 0..teeth {
        let a0 = i as f32 * step;
        let tooth = step * 0.25;
        let gap = step * 0.5;
        // Tooth tip is an arc at the outer radius; the valley between teeth is an arc at the inner radius.
        let p = |r: f32, a: f32| (cx + r * libm::cosf(a), cy + r * libm::sinf(a));
        let (x0, y0) = p(r_in, a0);
        if i == 0 {
            b.move_to(x0, y0);
        } else {
            b.line_to(x0, y0);
        }
        let (x1, y1) = p(r_out, a0 + tooth * 0.5);
        b.line_to(x1, y1);
        b.arc(cx, cy, r_out, a0 + tooth * 0.5, a0 + tooth * 1.5, false);
        let (x2, y2) = p(r_in, a0 + tooth * 1.5 + gap * 0.5);
        b.line_to(x2, y2);
        b.arc(cx, cy, r_in, a0 + tooth * 1.5 + gap * 0.5, a0 + step, false);
    }
    b.close();
    let mut hole = PathBuilder::new();
    hole.circle(cx, cy, 30.0);

    let mut ctx = Context::new(SvgBackend::new(240.0, 240.0));
    ctx.set_fill_paint(Paint::Solid(Color::rgb(0.55, 0.6, 0.7)));
    ctx.set_fill_rule(FillRule::EvenOdd);
    let gear = b.build();
    ctx.fill_path(&gear);
    ctx.set_fill_paint(Paint::Solid(Color::WHITE));
    ctx.fill_path(&hole.build());
    print!("{}", ctx.into_backend().finish());
}
