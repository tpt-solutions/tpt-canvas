//! Polygon triangulation for GPU backends.
//!
//! Two strategies are provided:
//!
//! - [`ear_clip`]: triangulates one outer contour with holes into a set of triangles
//!   whose areas sum to the region's area. It is exact for simple (non-self-intersecting)
//!   regions. Holes are merged into the outer contour with mutually visible bridges.
//!   Degenerate input (duplicate points, zero-area slivers) is dropped rather than
//!   emitted. Self-intersecting input never panics, but its output is not a valid
//!   coverage decomposition.
//! - [`fan`]: triangulates one closed contour as a triangle fan from its first point.
//!   Each triangle carries the contour's orientation as a signed winding contribution,
//!   which is what a stencil-then-cover fill needs. It handles self-intersections and
//!   overlapping contours correctly, because the winding sum is exact.

use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::Vec2;

/// Indexed triangles.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Triangulation {
    pub positions: Vec<Vec2>,
    /// Three indices per triangle, into `positions`.
    pub indices: Vec<u32>,
}

impl Triangulation {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Sum of the (unsigned) areas of all triangles.
    pub fn area(&self) -> f32 {
        self.indices
            .chunks_exact(3)
            .map(|t| {
                let a = self.positions[t[0] as usize];
                let b = self.positions[t[1] as usize];
                let c = self.positions[t[2] as usize];
                0.5 * (b - a).cross(c - a).abs()
            })
            .sum()
    }
}

const EPS: f32 = 1e-9;

/// Signed area of a closed ring (positive when counter-clockwise).
pub fn signed_area(ring: &[Vec2]) -> f32 {
    let n = ring.len();
    0.5 * (0..n)
        .map(|i| ring[i].cross(ring[(i + 1) % n]))
        .sum::<f32>()
}

/// Removes consecutive duplicate points, including a closing point equal to the first.
fn clean(ring: &[Vec2]) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::with_capacity(ring.len());
    for &p in ring {
        if !(p.x.is_finite() && p.y.is_finite()) {
            continue;
        }
        if out.last().map_or(true, |q| q.distance(p) > EPS) {
            out.push(p);
        }
    }
    while out.len() > 1 && out[0].distance(out[out.len() - 1]) <= EPS {
        out.pop();
    }
    out
}

/// Triangle fan from `ring[0]`. Triangles keep the ring's orientation, so summing
/// `±1` per triangle over a point gives that point's winding number.
pub fn fan(ring: &[Vec2]) -> Triangulation {
    let pts = clean(ring);
    let mut t = Triangulation::default();
    if pts.len() < 3 {
        return t;
    }
    t.positions = pts.clone();
    for i in 1..pts.len() - 1 {
        t.indices.extend_from_slice(&[0, i as u32, (i + 1) as u32]);
    }
    t
}

/// Triangulates `outer` with `holes` by ear clipping. Output is CCW triangles.
pub fn ear_clip(outer: &[Vec2], holes: &[&[Vec2]]) -> Triangulation {
    let mut poly = clean(outer);
    if poly.len() < 3 {
        return Triangulation::default();
    }
    if signed_area(&poly) < 0.0 {
        poly.reverse();
    }

    let mut all_holes: Vec<Vec<Vec2>> = holes
        .iter()
        .map(|h| clean(h))
        .filter(|h| h.len() >= 3)
        .map(|mut h| {
            // Holes wind opposite to the outer contour.
            if signed_area(&h) > 0.0 {
                h.reverse();
            }
            h
        })
        .collect();
    // Merge the rightmost hole first.
    all_holes.sort_by(|a, b| max_x(b).partial_cmp(&max_x(a)).unwrap_or(Ordering::Equal));
    let outer_ccw = outer_ring(outer);
    let mut pending = all_holes.clone();
    while !pending.is_empty() {
        let hole = pending.remove(0);
        poly = merge_hole(&poly, &hole, &pending, &outer_ccw, &all_holes);
    }

    let mut out = Triangulation {
        positions: poly.clone(),
        indices: Vec::new(),
    };
    clip_ears(&poly, &mut out.indices);
    out
}

/// Outer contour as a clean CCW ring, used to test bridge midpoints.
fn outer_ring(outer: &[Vec2]) -> Vec<Vec2> {
    let mut o = clean(outer);
    if signed_area(&o) < 0.0 {
        o.reverse();
    }
    o
}

fn max_x(ring: &[Vec2]) -> f32 {
    ring.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max)
}

/// Splices `hole` into `poly` through a bridge to a visible vertex.
fn merge_hole(
    poly: &[Vec2],
    hole: &[Vec2],
    other_holes: &[Vec<Vec2>],
    outer: &[Vec2],
    all_holes: &[Vec<Vec2>],
) -> Vec<Vec2> {
    // Hole vertex with the largest x (ties broken by lowest index).
    let mut hi = 0;
    for (i, p) in hole.iter().enumerate() {
        if p.x > hole[hi].x {
            hi = i;
        }
    }
    let h = hole[hi];

    // Nearest visible polygon vertex wins; fall back to the nearest if none is clear.
    let mut order: Vec<usize> = (0..poly.len()).collect();
    order.sort_by(|&a, &b| {
        poly[a]
            .distance(h)
            .partial_cmp(&poly[b].distance(h))
            .unwrap_or(Ordering::Equal)
    });
    let bridge = order
        .iter()
        .copied()
        .find(|&i| bridge_is_clear(h, poly[i], poly, hole, other_holes, outer, all_holes))
        .unwrap_or(order[0]);

    let mut out = Vec::with_capacity(poly.len() + hole.len() + 2);
    out.extend_from_slice(&poly[..=bridge]);
    for k in 0..=hole.len() {
        out.push(hole[(hi + k) % hole.len()]);
    }
    out.push(poly[bridge]);
    out.extend_from_slice(&poly[bridge + 1..]);
    out
}

/// True when the bridge `h -> target` lies in the region between outer and holes:
/// it crosses no edge, no vertex lies on it, and its midpoint is inside the outer
/// contour and outside every hole.
fn bridge_is_clear(
    h: Vec2,
    target: Vec2,
    poly: &[Vec2],
    hole: &[Vec2],
    other_holes: &[Vec<Vec2>],
    outer: &[Vec2],
    all_holes: &[Vec<Vec2>],
) -> bool {
    let crosses = |ring: &[Vec2]| {
        let n = ring.len();
        (0..n).any(|i| proper_intersect(h, target, ring[i], ring[(i + 1) % n]))
    };
    if crosses(poly) || crosses(hole) || other_holes.iter().any(|o| crosses(o)) {
        return false;
    }
    let on_bridge = |ring: &[Vec2]| {
        ring.iter().any(|&v| {
            v.distance(h) > EPS && v.distance(target) > EPS && point_on_segment(v, h, target)
        })
    };
    if on_bridge(poly) || on_bridge(hole) || other_holes.iter().any(|o| on_bridge(o)) {
        return false;
    }
    let mid = (h + target) * 0.5;
    in_ring(outer, mid) && !all_holes.iter().any(|o| in_ring(o, mid))
}

fn point_on_segment(p: Vec2, a: Vec2, b: Vec2) -> bool {
    let ab = b - a;
    let len2 = ab.dot(ab);
    if len2 <= 0.0 {
        return false;
    }
    let t = (p - a).dot(ab) / len2;
    if !(EPS..=1.0 - EPS).contains(&t) {
        return false;
    }
    (p - a).cross(ab).abs() <= EPS * libm::sqrtf(len2).max(1.0)
}

/// Even-odd crossing test of `p` against a closed ring.
fn in_ring(ring: &[Vec2], p: Vec2) -> bool {
    let n = ring.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y);
            if p.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// Strict segment intersection; shared endpoints and collinear touches do not count.
fn proper_intersect(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let d1 = (b - a).cross(c - a);
    let d2 = (b - a).cross(d - a);
    let d3 = (d - c).cross(a - c);
    let d4 = (d - c).cross(b - c);
    ((d1 > EPS && d2 < -EPS) || (d1 < -EPS && d2 > EPS))
        && ((d3 > EPS && d4 < -EPS) || (d3 < -EPS && d4 > EPS))
}

/// Ear clipping on a CCW weakly simple polygon (bridges repeat positions, so triangles
/// are checked by index and positions are compared when skipping coincident vertices).
fn clip_ears(poly: &[Vec2], indices: &mut Vec<u32>) {
    let mut rem: Vec<usize> = (0..poly.len()).collect();
    // Each pass either clips an ear or drops a collinear vertex, so the loop terminates.
    while rem.len() > 3 {
        let n = rem.len();
        let mut progressed = false;
        for i in 0..n {
            let (ip, ic, inx) = (rem[(i + n - 1) % n], rem[i], rem[(i + 1) % n]);
            let (a, b, c) = (poly[ip], poly[ic], poly[inx]);
            let turn = (b - a).cross(c - b);
            if turn.abs() <= EPS {
                rem.remove(i);
                progressed = true;
                break;
            }
            if turn < 0.0 || blocked(poly, &rem, [ip, ic, inx]) {
                continue;
            }
            push_triangle(indices, ip, ic, inx);
            rem.remove(i);
            progressed = true;
            break;
        }
        if !progressed {
            // Self-intersecting input has no valid ear. Clip one vertex to keep going.
            let (ip, ic, inx) = (rem[n - 1], rem[0], rem[1]);
            push_triangle(indices, ip, ic, inx);
            rem.remove(0);
        }
    }
    if rem.len() == 3 {
        push_triangle(indices, rem[0], rem[1], rem[2]);
    }
}

/// True when another remaining vertex lies inside the ear `tri` (coincident positions ignored).
fn blocked(poly: &[Vec2], rem: &[usize], tri: [usize; 3]) -> bool {
    let (a, b, c) = (poly[tri[0]], poly[tri[1]], poly[tri[2]]);
    rem.iter().any(|&j| {
        if tri.contains(&j) {
            return false;
        }
        let q = poly[j];
        if q.distance(a) <= EPS || q.distance(b) <= EPS || q.distance(c) <= EPS {
            return false;
        }
        point_in_triangle(q, a, b, c)
    })
}

fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    (b - a).cross(p - a) >= 0.0 && (c - b).cross(p - b) >= 0.0 && (a - c).cross(p - c) >= 0.0
}

fn push_triangle(indices: &mut Vec<u32>, a: usize, b: usize, c: usize) {
    indices.extend_from_slice(&[a as u32, b as u32, c as u32]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn square(x: f32, y: f32, s: f32, ccw: bool) -> Vec<Vec2> {
        let mut v = vec![
            Vec2::new(x, y),
            Vec2::new(x + s, y),
            Vec2::new(x + s, y + s),
            Vec2::new(x, y + s),
        ];
        if !ccw {
            v.reverse();
        }
        v
    }

    #[test]
    fn convex_square_has_two_triangles() {
        let t = ear_clip(&square(0.0, 0.0, 2.0, true), &[]);
        assert_eq!(t.triangle_count(), 2);
        assert!((t.area() - 4.0).abs() < 1e-5);
    }

    #[test]
    fn clockwise_input_is_accepted() {
        let t = ear_clip(&square(0.0, 0.0, 2.0, false), &[]);
        assert!((t.area() - 4.0).abs() < 1e-5);
    }

    #[test]
    fn square_with_square_hole_area_is_difference() {
        let outer = square(0.0, 0.0, 10.0, true);
        let hole = square(3.0, 3.0, 4.0, false);
        let t = ear_clip(&outer, &[&hole]);
        assert!(
            (t.area() - (100.0 - 16.0)).abs() < 1e-3,
            "area {}",
            t.area()
        );
    }

    #[test]
    fn two_holes_are_bridged() {
        let outer = square(0.0, 0.0, 20.0, true);
        let h1 = square(2.0, 2.0, 3.0, false);
        let h2 = square(12.0, 12.0, 5.0, false);
        let t = ear_clip(&outer, &[&h1, &h2]);
        assert!(
            (t.area() - (400.0 - 9.0 - 25.0)).abs() < 1e-2,
            "area {}",
            t.area()
        );
    }

    #[test]
    fn degenerate_input_yields_nothing() {
        assert_eq!(ear_clip(&[Vec2::ZERO, Vec2::ZERO], &[]).triangle_count(), 0);
        let line = [
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(2.0, 0.0),
        ];
        assert!(ear_clip(&line, &[]).area() < 1e-6);
    }

    #[test]
    fn fan_winding_sum_matches_point_containment() {
        // Concave "L" shape: the signed fan sum must match the crossing-number result.
        let l = [
            Vec2::new(0.0, 0.0),
            Vec2::new(4.0, 0.0),
            Vec2::new(4.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 4.0),
            Vec2::new(0.0, 4.0),
        ];
        let t = fan(&l);
        let fan_winding = |p: Vec2| -> i32 {
            t.indices
                .chunks_exact(3)
                .map(|tri| {
                    let (a, b, c) = (
                        t.positions[tri[0] as usize],
                        t.positions[tri[1] as usize],
                        t.positions[tri[2] as usize],
                    );
                    let s = (b - a).cross(c - a);
                    let (a, b, c) = if s >= 0.0 { (a, b, c) } else { (a, c, b) };
                    match (point_in_triangle(p, a, b, c), s >= 0.0) {
                        (true, true) => 1,
                        (true, false) => -1,
                        _ => 0,
                    }
                })
                .sum()
        };
        // Samples avoid fan edges, where a point is shared by two triangles.
        for p in [
            Vec2::new(0.5, 0.3),
            Vec2::new(0.3, 0.5),
            Vec2::new(3.0, 0.5),
            Vec2::new(0.5, 3.0),
            Vec2::new(3.0, 3.0),
            Vec2::new(-1.0, 2.0),
        ] {
            let expected = if in_ring(&l, p) { 1 } else { 0 };
            assert_eq!(fan_winding(p), expected, "point {p:?}");
        }
    }
}
