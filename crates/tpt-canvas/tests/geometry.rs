//! Arc conversion, flattening tolerance, transform properties, and fuzzing.

use std::f32::consts::{FRAC_PI_2, PI};

use tpt_canvas::testkit::{fuzz, Rng};
use tpt_canvas::{Affine2, Path, PathBuilder, PathCommand, Vec2};

/// Distance from `p` to the closest point on any polyline segment.
fn dist_to_polyline(p: Vec2, pts: &[Vec2]) -> f32 {
    let mut best = f32::INFINITY;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let ab = b - a;
        let len2 = ab.dot(ab);
        let t = if len2 > 0.0 {
            ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        best = best.min((a + ab * t).distance(p));
    }
    best
}

/// Samples the path's cubic/quad segments at `n` points each (test helper for exact curves).
fn sample_points(path: &Path, n: usize) -> Vec<Vec2> {
    let mut out = Vec::new();
    let mut cur = Vec2::ZERO;
    for cmd in path.commands() {
        match *cmd {
            PathCommand::MoveTo(p) | PathCommand::LineTo(p) => {
                cur = p;
                out.push(p);
            }
            PathCommand::QuadTo(c, p) => {
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let mt = 1.0 - t;
                    out.push(cur * (mt * mt) + c * (2.0 * mt * t) + p * (t * t));
                }
                cur = p;
            }
            PathCommand::CubicTo(c1, c2, p) => {
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let mt = 1.0 - t;
                    let (a, b, c, d) =
                        (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
                    out.push(cur * a + c1 * b + c2 * c + p * d);
                }
                cur = p;
            }
            PathCommand::Close => {}
        }
    }
    out
}

fn max_radial_error(pts: &[Vec2], center: Vec2, r: f32) -> f32 {
    pts.iter()
        .map(|p| (p.distance(center) - r).abs())
        .fold(0.0, f32::max)
}

#[test]
fn circle_bounding_box_is_exact() {
    let mut b = PathBuilder::new();
    b.circle(10.0, -4.0, 3.0);
    let r = b.build().bounding_box();
    assert!((r.x - 7.0).abs() < 1e-3);
    assert!((r.y + 7.0).abs() < 1e-3);
    assert!((r.w - 6.0).abs() < 1e-3);
    assert!((r.h - 6.0).abs() < 1e-3);
}

#[test]
fn html5_arc_stays_on_circle_for_many_sweeps() {
    for &(start, end, ccw) in &[
        (0.0, FRAC_PI_2, false),
        (0.3, 2.9, false),
        (0.0, 4.0 * PI, false), // full turns draw a full circle
        (1.0, -2.0, true),
        (-3.0, 3.0, true),
    ] {
        let mut b = PathBuilder::new();
        b.arc(5.0, 6.0, 2.0, start, end, ccw);
        let pts = sample_points(&b.build(), 64);
        let err = max_radial_error(&pts, Vec2::new(5.0, 6.0), 2.0);
        assert!(
            err < 2e-3,
            "start={start} end={end} ccw={ccw} radial error {err}"
        );
    }
}

#[test]
fn html5_arc_ccw_and_cw_meet_at_expected_endpoints() {
    let mut b = PathBuilder::new();
    b.arc(0.0, 0.0, 1.0, 0.0, FRAC_PI_2, false);
    let cw = b.build().bounding_box();
    let mut b = PathBuilder::new();
    b.arc(0.0, 0.0, 1.0, 0.0, FRAC_PI_2, true);
    let ccw = b.build().bounding_box();
    // Clockwise quarter from 0 to pi/2 covers the +x,+y quadrant; ccw covers the long way round.
    assert!(cw.max().x <= 1.0 + 1e-4 && cw.max().y <= 1.0 + 1e-4 && cw.min().x >= -1e-4);
    assert!(ccw.min().x <= -1.0 + 1e-3 || ccw.min().y <= -1.0 + 1e-3);
}

#[test]
fn arc_to_tangent_arc_matches_geometry() {
    // Corner at (10,0) turning toward (10,10) with radius 5: the arc centre is (5,5).
    let mut b = PathBuilder::new();
    b.move_to(0.0, 0.0);
    b.arc_to(10.0, 0.0, 10.0, 10.0, 5.0);
    let path = b.build();
    let pts = sample_points(&path, 64);
    let arc_pts: Vec<Vec2> = pts
        .iter()
        .copied()
        .filter(|p| p.x > 5.0 && p.y > 0.0)
        .collect();
    let err = max_radial_error(&arc_pts, Vec2::new(5.0, 5.0), 5.0);
    assert!(err < 5e-3, "tangent arc radial error {err}");
    // Canvas arcTo: the arc starts at the first tangent point (5,0) and ends at the second
    // tangent point (10,5). The line from there to (10,10) is not drawn by arcTo itself.
    let flat = path.flatten(0.001);
    let all = &flat.polylines[0].points;
    assert!(dist_to_polyline(Vec2::new(5.0, 0.0), all) < 1e-3);
    assert!(all.last().unwrap().distance(Vec2::new(10.0, 5.0)) < 1e-3);
}

#[test]
fn arc_to_degenerate_inputs_become_lines() {
    let mut b = PathBuilder::new();
    b.move_to(0.0, 0.0);
    b.arc_to(0.0, 0.0, 5.0, 5.0, 3.0); // p0 == p1
    b.arc_to(5.0, 5.0, 10.0, 10.0, 3.0); // collinear
    let path = b.build();
    assert!(path
        .commands()
        .iter()
        .all(|c| !matches!(c, PathCommand::CubicTo(..))));
}

#[test]
fn svg_endpoint_arc_semicircle() {
    let mut b = PathBuilder::new();
    b.move_to(0.0, 0.0);
    b.elliptical_arc(5.0, 5.0, 0.0, false, true, 10.0, 0.0);
    let pts = sample_points(&b.build(), 64);
    let err = max_radial_error(&pts, Vec2::new(5.0, 0.0), 5.0);
    assert!(err < 2e-3, "semicircle radial error {err}");
}

#[test]
fn svg_endpoint_arc_scales_small_radii() {
    // Radius 1 cannot span a 10-unit chord: SVG scales it up to a semicircle of radius 5.
    let mut b = PathBuilder::new();
    b.move_to(0.0, 0.0);
    b.elliptical_arc(1.0, 1.0, 0.0, false, false, 10.0, 0.0);
    let path = b.build();
    let pts = sample_points(&path, 64);
    let err = max_radial_error(&pts, Vec2::new(5.0, 0.0), 5.0);
    assert!(err < 2e-3, "scaled radial error {err}");
    assert!(pts.last().unwrap().distance(Vec2::new(10.0, 0.0)) < 1e-4);
}

#[test]
fn svg_endpoint_arc_large_flag_picks_the_big_arc() {
    // Small and large arcs between the same endpoints mirror through the chord.
    let mut small = PathBuilder::new();
    small.move_to(0.0, 0.0);
    small.elliptical_arc(5.0, 5.0, 0.0, false, true, 6.0, 0.0);
    let mut large = PathBuilder::new();
    large.move_to(0.0, 0.0);
    large.elliptical_arc(5.0, 5.0, 0.0, true, true, 6.0, 0.0);
    let s = small.build().bounding_box();
    let l = large.build().bounding_box();
    assert!(l.w * l.h > s.w * s.h, "large arc should enclose more area");
}

#[test]
fn flatten_respects_tolerance_sweep() {
    let mut b = PathBuilder::new();
    b.move_to(0.0, 0.0);
    b.cubic_to(0.0, 200.0, 300.0, -150.0, 300.0, 50.0);
    b.quad_to(400.0, 300.0, 100.0, 300.0);
    let path = b.build();
    let reference = sample_points(&path, 4000);
    for tol in [4.0f32, 1.0, 0.1, 0.01] {
        let flat = path.flatten(tol);
        let mut worst = 0.0f32;
        for p in &reference {
            let d = flat
                .polylines
                .iter()
                .map(|pl| dist_to_polyline(*p, &pl.points))
                .fold(f32::INFINITY, f32::min);
            worst = worst.max(d);
        }
        assert!(worst <= tol + 1e-3, "tol {tol}: worst deviation {worst}");
    }
}

#[test]
fn flatten_is_deterministic() {
    let mut b = PathBuilder::new();
    b.circle(3.0, 4.0, 17.0);
    b.rounded_rect(-5.0, -5.0, 30.0, 20.0, 6.0);
    let path = b.build();
    let first = path.flatten(0.05);
    for _ in 0..3 {
        assert_eq!(path.flatten(0.05), first);
    }
}

#[test]
fn affine_inverse_round_trips_within_bound() {
    fuzz(7, 2000, |rng| {
        let m = Affine2::new(
            rng.range(-3.0, 3.0),
            rng.range(-3.0, 3.0),
            rng.range(-3.0, 3.0),
            rng.range(-3.0, 3.0),
            rng.range(-100.0, 100.0),
            rng.range(-100.0, 100.0),
        );
        // Near-singular matrices amplify rounding error; test only well-conditioned ones.
        if m.determinant().abs() < 0.5 {
            return;
        }
        let Some(inv) = m.invert() else { return };
        let p = Vec2::new(rng.range(-50.0, 50.0), rng.range(-50.0, 50.0));
        let q = inv.apply(m.apply(p));
        assert!(q.distance(p) < 1e-2, "round trip error {}", q.distance(p));
    });
}

#[test]
fn deep_transform_stack_has_no_drift() {
    // Push 5000 rotations/scales/translations, then pop them in reverse.
    let mut rng = Rng::new(99);
    let mut ops = Vec::new();
    let mut acc = Affine2::IDENTITY;
    for _ in 0..5000 {
        let m = match rng.index(3) {
            0 => Affine2::rotate(rng.range(-PI, PI)),
            1 => Affine2::scale(rng.range(0.9, 1.1), rng.range(0.9, 1.1)),
            _ => Affine2::translate(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)),
        };
        acc = acc * m;
        ops.push(m);
    }
    // Multiply the inverses back out in reverse order.
    let mut back = acc;
    for m in ops.iter().rev() {
        back = back * m.invert().unwrap();
    }
    let p = Vec2::new(12.0, -7.0);
    assert!(
        back.apply(p).distance(p) < 1e-2,
        "drift {}",
        back.apply(p).distance(p)
    );
}

#[test]
fn random_paths_never_panic() {
    fuzz(1234, 5000, |rng| {
        let mut b = PathBuilder::new();
        let steps = 1 + rng.index(12);
        for _ in 0..steps {
            let x = rng.range(-1e3, 1e3);
            let y = rng.range(-1e3, 1e3);
            match rng.index(8) {
                0 => b.move_to(x, y),
                1 => b.line_to(x, y),
                2 => b.quad_to(x, y, rng.range(-1e3, 1e3), rng.range(-1e3, 1e3)),
                3 => b.cubic_to(
                    x,
                    y,
                    rng.range(-1e3, 1e3),
                    rng.range(-1e3, 1e3),
                    rng.range(-1e3, 1e3),
                    rng.range(-1e3, 1e3),
                ),
                4 => b.arc(
                    x,
                    y,
                    rng.range(-5.0, 300.0),
                    rng.range(-10.0, 10.0),
                    rng.range(-10.0, 10.0),
                    rng.index(2) == 0,
                ),
                5 => b.arc_to(
                    x,
                    y,
                    rng.range(-1e3, 1e3),
                    rng.range(-1e3, 1e3),
                    rng.range(-5.0, 300.0),
                ),
                6 => b.elliptical_arc(
                    rng.range(-5.0, 300.0),
                    rng.range(-5.0, 300.0),
                    rng.range(-4.0, 4.0),
                    rng.index(2) == 0,
                    rng.index(2) == 0,
                    x,
                    y,
                ),
                _ => b.close(),
            }
        }
        let path = b.build();
        let _ = path.bounding_box();
        let flat = path.flatten(rng.range(0.01, 2.0));
        assert!(flat
            .polylines
            .iter()
            .all(|p| p.points.iter().all(|q| q.x.is_finite() && q.y.is_finite())));
        let round = Path::deserialize(&path.serialize()).unwrap();
        assert_eq!(round, path);
    });
}
