//! Randomized robustness and determinism checks for the raster backend.

use tpt_canvas::{Affine2, CanvasBackend, Color, FillRule, Paint, PathBuilder, StrokeStyle};
use tpt_canvas_raster::RasterBackend;
use tpt_testkit_stub::fuzz;

fn random_path(rng: &mut tpt_testkit_stub::Rng) -> tpt_canvas::Path {
    let mut b = PathBuilder::new();
    for _ in 0..(1 + rng.index(10)) {
        let (x, y) = (rng.range(-20.0, 90.0), rng.range(-20.0, 90.0));
        match rng.index(6) {
            0 => b.move_to(x, y),
            1 => b.line_to(x, y),
            2 => b.quad_to(x, y, rng.range(-20.0, 90.0), rng.range(-20.0, 90.0)),
            3 => b.cubic_to(
                x,
                y,
                rng.range(-20.0, 90.0),
                rng.range(-20.0, 90.0),
                rng.range(-20.0, 90.0),
                rng.range(-20.0, 90.0),
            ),
            4 => b.arc(
                x,
                y,
                rng.range(0.0, 40.0),
                rng.range(-7.0, 7.0),
                rng.range(-7.0, 7.0),
                rng.index(2) == 0,
            ),
            _ => b.close(),
        }
    }
    b.build()
}

#[test]
fn random_draws_never_panic_and_stay_in_range() {
    fuzz(42, 400, |rng| {
        let mut rb = RasterBackend::new(64, 64);
        let path = random_path(rng);
        let rule = if rng.index(2) == 0 {
            FillRule::NonZero
        } else {
            FillRule::EvenOdd
        };
        rb.push_transform(&Affine2::scale(rng.range(0.5, 2.0), rng.range(0.5, 2.0)));
        rb.fill_path(&path, &Paint::Solid(Color::rgba(0.2, 0.5, 0.7, 0.6)), rule);
        let style = StrokeStyle {
            width: rng.range(0.0, 6.0),
            dash: if rng.index(2) == 0 {
                vec![]
            } else {
                vec![rng.range(0.1, 5.0), rng.range(0.1, 5.0)]
            },
            ..StrokeStyle::default()
        };
        rb.stroke_path(&path, &Paint::Solid(Color::BLACK), &style);
        rb.pop_transform();
        for p in &rb.pixmap().data {
            assert!(
                p.iter()
                    .all(|c| c.is_finite() && (-1e-4..=1.0 + 1e-4).contains(c)),
                "{p:?}"
            );
            assert!(
                p[0] <= p[3] + 1e-4 && p[1] <= p[3] + 1e-4 && p[2] <= p[3] + 1e-4,
                "not premultiplied: {p:?}"
            );
        }
    });
}

#[test]
fn identical_draws_produce_identical_pixels() {
    let render = || {
        let mut b = PathBuilder::new();
        b.circle(32.0, 32.0, 20.0);
        b.rounded_rect(4.0, 4.0, 20.0, 12.0, 4.0);
        let path = b.build();
        let mut rb = RasterBackend::new(64, 64);
        rb.fill_path(
            &path,
            &Paint::Solid(Color::rgb(0.9, 0.1, 0.4)),
            FillRule::NonZero,
        );
        rb.stroke_path(
            &path,
            &Paint::Solid(Color::BLACK),
            &StrokeStyle {
                width: 2.0,
                ..StrokeStyle::default()
            },
        );
        rb.into_pixmap().to_rgba8()
    };
    assert_eq!(render(), render());
}
