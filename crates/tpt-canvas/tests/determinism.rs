//! Cross-platform determinism: the same geometry must produce bit-identical results on
//! native and wasm32. The test computes a digest over every float that matters and checks
//! it against a constant. Native and wasm32 runs must produce the same digest.
//!
//! Native:  cargo test -p tpt-canvas --test determinism
//! wasm32:  cargo test -p tpt-canvas --test determinism --target wasm32-unknown-unknown
//!          (needs wasm-bindgen-test-runner on PATH, matching the locked wasm-bindgen version)

use tpt_canvas::stroke::stroke_outline;
use tpt_canvas::style::{LineCap, LineJoin, StrokeStyle};
use tpt_canvas::tessellate::{ear_clip, fan};
use tpt_canvas::{FlattenedPath, Path, PathBuilder, Vec2};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// Digest that native and wasm32 runs must both produce. Re-pin only after confirming both agree.
const EXPECTED: u64 = 0x717d_b922_8d7a_8ad9;

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= x as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn f32(&mut self, v: f32) {
        self.bytes(&v.to_bits().to_le_bytes());
    }
    fn point(&mut self, p: Vec2) {
        self.f32(p.x);
        self.f32(p.y);
    }
    fn flat(&mut self, f: &FlattenedPath) {
        for pl in &f.polylines {
            self.bytes(&[pl.closed as u8]);
            for &p in &pl.points {
                self.point(p);
            }
        }
    }
    fn polys(&mut self, polys: &[Vec<Vec2>]) {
        for poly in polys {
            for &p in poly {
                self.point(p);
            }
        }
    }
}

fn scene_paths() -> Vec<Path> {
    let mut out = Vec::new();
    let mut b = PathBuilder::new();
    b.circle(3.0, 4.0, 17.0);
    out.push(b.build());
    let mut b = PathBuilder::new();
    b.rounded_rect(-5.0, -5.0, 30.0, 20.0, 6.0);
    out.push(b.build());
    let mut b = PathBuilder::new();
    b.move_to(0.0, 0.0);
    b.cubic_to(0.0, 200.0, 300.0, -150.0, 300.0, 50.0);
    b.quad_to(400.0, 300.0, 100.0, 300.0);
    b.arc_to(120.0, 90.0, 40.0, 60.0, 25.0);
    b.elliptical_arc(30.0, 12.0, 0.4, true, false, 80.0, 40.0);
    out.push(b.build());
    out
}

fn digest() -> u64 {
    let mut h = Fnv::new();
    for path in scene_paths() {
        h.bytes(&path.serialize());
        h.flat(&path.flatten(0.05));
        h.bytes(&path.bounding_box().x.to_bits().to_le_bytes());
        let style = StrokeStyle {
            width: 3.5,
            cap: LineCap::Round,
            join: LineJoin::Round,
            dash: vec![4.0, 2.5],
            dash_offset: 1.0,
            ..StrokeStyle::default()
        };
        h.polys(&stroke_outline(&path.flatten(0.05), &style, 0.05));
    }
    let star: Vec<Vec2> = (0..9)
        .map(|i| {
            let r = if i % 2 == 0 { 10.0 } else { 4.0 };
            let a = i as f32 * 0.6981317;
            Vec2::new(r * libm::cosf(a), r * libm::sinf(a))
        })
        .collect();
    let t = ear_clip(&star, &[]);
    h.polys(&t.positions.iter().map(|p| vec![*p]).collect::<Vec<_>>());
    for &i in &t.indices {
        h.bytes(&i.to_le_bytes());
    }
    let f = fan(&star);
    for &i in &f.indices {
        h.bytes(&i.to_le_bytes());
    }
    h.0
}

#[test]
fn digest_matches_across_targets() {
    let d = digest();
    // Printed so the constant can be re-pinned after an intentional change.
    println!("determinism digest: {d:#018x}");
    assert_eq!(
        d, EXPECTED,
        "digest changed; check that native and wasm32 agree"
    );
}
