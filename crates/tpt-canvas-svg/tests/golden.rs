//! Golden-file test: a scene with translucent gradients, dashed strokes, and nested clips.
//! Run with `TPT_UPDATE_GOLDEN=1 cargo test -p tpt-canvas-svg` to rewrite the golden.

use std::path::PathBuf;

use tpt_canvas::{
    CanvasBackend, Color, ColorSpace, Context, DisplayList, FillRule, LineCap, LineJoin, Paint,
    PathBuilder, SpreadMode, Stop, StrokeStyle, Vec2,
};
use tpt_canvas_svg::SvgBackend;
use tpt_testkit_stub::check_golden;

fn scene<B: CanvasBackend>(ctx: &mut Context<B>) {
    // Background: translucent linear gradient.
    let mut bg = PathBuilder::new();
    bg.rounded_rect(10.0, 10.0, 380.0, 280.0, 24.0);
    ctx.set_fill_paint(Paint::LinearGradient {
        start: Vec2::new(10.0, 10.0),
        end: Vec2::new(390.0, 290.0),
        stops: vec![
            Stop {
                offset: 0.0,
                color: Color::rgba(0.2, 0.4, 0.9, 0.9),
            },
            Stop {
                offset: 1.0,
                color: Color::rgba(0.9, 0.3, 0.2, 0.4),
            },
        ],
        spread: SpreadMode::Pad,
        space: ColorSpace::Srgb,
    });
    ctx.fill_path(&bg.build());

    // Overlapping translucent radial gradient circles.
    for (i, cx) in [150.0f32, 230.0].iter().enumerate() {
        let mut c = PathBuilder::new();
        c.circle(*cx, 150.0, 70.0);
        ctx.set_fill_paint(Paint::RadialGradient {
            center: Vec2::new(*cx, 150.0),
            radius: 70.0,
            stops: vec![
                Stop {
                    offset: 0.0,
                    color: Color::rgba(1.0, 1.0, 0.2, 0.8),
                },
                Stop {
                    offset: 0.6,
                    color: Color::rgba(0.1, 0.7, 0.3, 0.5),
                },
                Stop {
                    offset: 1.0,
                    color: Color::rgba(0.1, 0.7, 0.3, 0.0),
                },
            ],
            spread: SpreadMode::Pad,
            space: if i == 0 {
                ColorSpace::Srgb
            } else {
                ColorSpace::LinearSrgb
            },
        });
        ctx.fill_path(&c.build());
    }

    // Nested clips and a rotated, even-odd star.
    ctx.save();
    let mut clip = PathBuilder::new();
    clip.circle(190.0, 150.0, 120.0);
    ctx.clip(&clip.build());
    ctx.translate(190.0, 150.0);
    ctx.rotate(0.2);
    let mut star = PathBuilder::new();
    for i in 0..10 {
        let r = if i % 2 == 0 { 55.0 } else { 22.0 };
        let a = i as f32 * core::f32::consts::PI / 5.0;
        let p = Vec2::new(r * libm::cosf(a), r * libm::sinf(a));
        if i == 0 {
            star.move_to(p.x, p.y);
        } else {
            star.line_to(p.x, p.y);
        }
    }
    star.close();
    ctx.set_fill_paint(Paint::Solid(Color::rgba(0.95, 0.95, 1.0, 0.85)));
    ctx.set_fill_rule(FillRule::EvenOdd);
    ctx.fill_path(&star.build());
    ctx.restore();

    // Dashed, round-joined stroke along a curve.
    let mut curve = PathBuilder::new();
    curve.move_to(20.0, 260.0);
    curve.cubic_to(120.0, 200.0, 280.0, 320.0, 380.0, 240.0);
    ctx.set_stroke_paint(Paint::Solid(Color::rgb(0.05, 0.05, 0.1)));
    ctx.set_stroke_style(StrokeStyle {
        width: 4.0,
        cap: LineCap::Round,
        join: LineJoin::Round,
        dash: vec![12.0, 6.0],
        dash_offset: 3.0,
        ..StrokeStyle::default()
    });
    ctx.stroke_path(&curve.build());
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name)
}

#[test]
fn scene_matches_golden_svg() {
    let mut ctx = Context::new(SvgBackend::new(400.0, 300.0));
    scene(&mut ctx);
    let svg = ctx.into_backend().finish();
    check_golden(&golden_path("scene.svg"), svg.as_bytes()).unwrap();
}

#[test]
fn immediate_and_retained_svg_output_are_identical() {
    let mut direct = Context::new(SvgBackend::new(400.0, 300.0));
    scene(&mut direct);
    let direct = direct.into_backend().finish();

    let mut rec = Context::new(DisplayList::new());
    scene(&mut rec);
    let mut svg = SvgBackend::new(400.0, 300.0);
    rec.into_backend().replay(&mut svg);
    assert_eq!(svg.finish(), direct);
}
