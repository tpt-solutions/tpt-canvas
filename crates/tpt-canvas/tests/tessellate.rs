//! Property tests for the triangulators: area conservation and no panics.

use tpt_canvas::tessellate::{ear_clip, fan, signed_area};
use tpt_canvas::Vec2;
use tpt_testkit_stub::{fuzz, Rng};

/// Random star-shaped polygon: angles strictly increasing, so it is simple.
fn star(rng: &mut Rng, cx: f32, cy: f32, r_min: f32, r_max: f32, n: usize) -> Vec<Vec2> {
    let mut angles: Vec<f32> = (0..n)
        .map(|i| (i as f32 + rng.range(0.1, 0.9)) / n as f32 * core::f32::consts::TAU)
        .collect();
    angles.sort_by(|a, b| a.partial_cmp(b).unwrap());
    angles
        .iter()
        .map(|&a| {
            let r = rng.range(r_min, r_max);
            Vec2::new(cx + r * libm::cosf(a), cy + r * libm::sinf(a))
        })
        .collect()
}

#[test]
fn ear_clip_conserves_area_for_random_simple_polygons() {
    fuzz(2024, 300, |rng| {
        let n = 3 + rng.index(30);
        let poly = star(rng, 0.0, 0.0, 1.0, 10.0, n);
        let expected = signed_area(&poly).abs();
        let t = ear_clip(&poly, &[]);
        assert!(
            (t.area() - expected).abs() <= 1e-2 * expected.max(1.0),
            "n={n} expected {expected} got {}",
            t.area()
        );
    });
}

#[test]
fn ear_clip_with_hole_conserves_area() {
    fuzz(77, 200, |rng| {
        // Outer star with a small star-shaped hole inside its core (radius < 1 < outer).
        let outer_n = 4 + rng.index(20);
        let outer = star(rng, 0.0, 0.0, 8.0, 12.0, outer_n);
        let hole_n = 3 + rng.index(10);
        let mut hole = star(rng, 0.0, 0.0, 0.5, 2.0, hole_n);
        hole.reverse();
        let expected = signed_area(&outer).abs() - signed_area(&hole).abs();
        let t = ear_clip(&outer, &[&hole]);
        assert!(
            (t.area() - expected).abs() <= 1e-2 * expected.max(1.0),
            "expected {expected} got {}",
            t.area()
        );
    });
}

#[test]
fn malformed_input_never_panics() {
    fuzz(5, 2000, |rng| {
        let n = rng.index(12);
        let ring: Vec<Vec2> = (0..n)
            .map(|_| {
                // Include duplicates, collinear runs and crossings by quantising coordinates.
                let q = |v: f32| (v / 2.0).round() * 2.0;
                Vec2::new(q(rng.range(-6.0, 6.0)), q(rng.range(-6.0, 6.0)))
            })
            .collect();
        let hole: Vec<Vec2> = (0..rng.index(6))
            .map(|_| Vec2::new(rng.range(-2.0, 2.0), rng.range(-2.0, 2.0)))
            .collect();
        let t = ear_clip(&ring, &[&hole]);
        assert!(t.indices.iter().all(|&i| (i as usize) < t.positions.len()));
        assert!(t
            .positions
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite()));
        let f = fan(&ring);
        assert!(f.indices.iter().all(|&i| (i as usize) < f.positions.len()));
    });
}

/// Long-running robustness run. Execute with:
/// `cargo test --release -p tpt-canvas --test tessellate -- --ignored --nocapture`
#[test]
#[ignore]
fn million_malformed_contours_never_panic() {
    let mut rng = Rng::new(0xC0FFEE);
    let mut triangles = 0usize;
    for _ in 0..1_000_000 {
        let n = rng.index(10);
        let ring: Vec<Vec2> = (0..n)
            .map(|_| {
                let q = |v: f32| (v / 2.0).round() * 2.0;
                Vec2::new(q(rng.range(-6.0, 6.0)), q(rng.range(-6.0, 6.0)))
            })
            .collect();
        let hole: Vec<Vec2> = (0..rng.index(5))
            .map(|_| Vec2::new(rng.range(-3.0, 3.0), rng.range(-3.0, 3.0)))
            .collect();
        let t = ear_clip(&ring, &[&hole]);
        assert!(t.indices.iter().all(|&i| (i as usize) < t.positions.len()));
        triangles += t.triangle_count();
        let f = fan(&ring);
        assert!(f.indices.iter().all(|&i| (i as usize) < f.positions.len()));
    }
    println!("triangles produced: {triangles}");
}
