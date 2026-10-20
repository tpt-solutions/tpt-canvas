//! An analog clock face showing 10:08:30, written as SVG to `gallery-out/analog_clock.svg`.
//! The time is fixed so the output is the same on every run.

use std::fs;
use std::path::Path as FsPath;

use tpt_canvas::prelude::*;
use tpt_canvas_svg::SvgBackend;

const SIZE: f32 = 200.0;
const CENTRE: f32 = SIZE / 2.0;
const RADIUS: f32 = 90.0;

fn main() -> std::io::Result<()> {
    let (hour, minute, second) = (10.0_f32, 8.0_f32, 30.0_f32);
    let tau = core::f32::consts::TAU;

    let mut ctx = Context::new(SvgBackend::new(SIZE, SIZE));
    ctx.translate(CENTRE, CENTRE);

    // Face.
    ctx.set_fill_paint(Paint::radial(
        Vec2::new(-20.0, -20.0),
        RADIUS * 1.6,
        &[(0.0, named::WHITE), (1.0, named::GRAY.lighten(0.5))],
    ));
    ctx.fill_circle(0.0, 0.0, RADIUS);
    ctx.set_stroke_paint(Paint::solid(named::BLACK));
    ctx.set_stroke_style(StrokeStyle {
        width: 3.0,
        ..StrokeStyle::default()
    });
    let mut rim = PathBuilder::new();
    rim.circle(0.0, 0.0, RADIUS);
    ctx.stroke_path(&rim.build());

    // Twelve hour ticks.
    ctx.set_stroke_style(StrokeStyle {
        width: 3.0,
        cap: LineCap::Round,
        ..StrokeStyle::default()
    });
    for i in 0..12 {
        ctx.save();
        ctx.rotate(tau * i as f32 / 12.0);
        let mut tick = PathBuilder::new();
        tick.move_to(0.0, -RADIUS + 6.0);
        tick.line_to(0.0, -RADIUS + 16.0);
        ctx.stroke_path(&tick.build());
        ctx.restore();
    }

    // Hands: angles measured clockwise from twelve o'clock.
    let hand =
        |ctx: &mut Context<SvgBackend>, angle: f32, length: f32, width: f32, colour: Color| {
            ctx.save();
            ctx.rotate(angle);
            ctx.set_stroke_paint(Paint::solid(colour));
            ctx.set_stroke_style(StrokeStyle {
                width,
                cap: LineCap::Round,
                ..StrokeStyle::default()
            });
            let mut line = PathBuilder::new();
            line.move_to(0.0, 6.0);
            line.line_to(0.0, -length);
            ctx.stroke_path(&line.build());
            ctx.restore();
        };
    hand(
        &mut ctx,
        tau * (hour + minute / 60.0) / 12.0,
        RADIUS * 0.5,
        6.0,
        named::BLACK,
    );
    hand(
        &mut ctx,
        tau * minute / 60.0,
        RADIUS * 0.75,
        4.0,
        named::BLACK,
    );
    hand(
        &mut ctx,
        tau * second / 60.0,
        RADIUS * 0.85,
        1.5,
        named::RED,
    );

    // Centre pin.
    ctx.set_fill_paint(Paint::solid(named::RED));
    ctx.fill_circle(0.0, 0.0, 4.0);

    let out = FsPath::new("gallery-out");
    fs::create_dir_all(out)?;
    fs::write(out.join("analog_clock.svg"), ctx.into_backend().finish())?;
    println!("wrote {}", out.join("analog_clock.svg").display());
    Ok(())
}
