//! Spec §4 pipeline: save, translate, clip to a rounded rect, then fill a gradient.
//! Run with `cargo run -p tpt-canvas-svg --example button > button.svg`.

use tpt_canvas::{CanvasBackend, ColorSpace};
use tpt_canvas::{Color, Context, FillRule, Paint, PathBuilder, SpreadMode, Stop, Vec2};
use tpt_canvas_svg::SvgBackend;

fn main() {
    let mut ctx = Context::new(SvgBackend::new(240.0, 120.0));
    draw_button(&mut ctx, 20.0, 20.0, 200.0, 56.0);
    print!("{}", ctx.into_backend().finish());
}

fn draw_button<B: CanvasBackend>(ctx: &mut Context<B>, x: f32, y: f32, w: f32, h: f32) {
    ctx.save();
    ctx.translate(x, y);
    let mut clip = PathBuilder::new();
    clip.rounded_rect(0.0, 0.0, w, h, 14.0);
    ctx.clip(&clip.build());

    let mut face = PathBuilder::new();
    face.rounded_rect(0.0, 0.0, w, h, 14.0);
    ctx.set_fill_paint(Paint::LinearGradient {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(0.0, h),
        stops: vec![
            Stop {
                offset: 0.0,
                color: Color::rgb(0.45, 0.7, 1.0),
            },
            Stop {
                offset: 1.0,
                color: Color::rgb(0.1, 0.3, 0.8),
            },
        ],
        spread: SpreadMode::Pad,
        space: ColorSpace::Srgb,
    });
    ctx.fill_path(&face.build());
    ctx.restore();

    ctx.set_fill_rule(FillRule::NonZero);
    ctx.set_fill_paint(Paint::Solid(Color::WHITE));
    let mut label = PathBuilder::new();
    label.circle(x + w * 0.5, y + h * 0.5, 6.0);
    ctx.fill_path(&label.build());
}
