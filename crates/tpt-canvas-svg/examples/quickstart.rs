//! The README example: one drawing routine, written to SVG.
//! Run with `cargo run -p tpt-canvas-svg --example quickstart > out.svg`.

use tpt_canvas::{Color, Context, Paint, PathBuilder};
use tpt_canvas_svg::SvgBackend;

fn main() {
    let mut ctx = Context::new(SvgBackend::new(200.0, 100.0));
    let mut b = PathBuilder::new();
    b.rounded_rect(10.0, 10.0, 180.0, 80.0, 12.0);
    ctx.set_fill_paint(Paint::Solid(Color::rgb(0.2, 0.4, 0.9)));
    ctx.fill_path(&b.build());
    let svg = ctx.into_backend().finish();
    print!("{svg}");
}
