//! Flattening engine: curves to polylines with a guaranteed maximum deviation.
//!
//! Uniform subdivision with a count from the standard bound. A cubic's
//! second-derivative bound M gives an error of at most `0.75 * M / n^2` when split
//! into `n` pieces, so `n = ceil(sqrt(0.75 * M / tol))` keeps the deviation under `tol`.
//! Evaluation is a fixed-order Bernstein sum, so output is deterministic.

use alloc::vec::Vec;

use super::{quad_to_cubic, Path, PathCommand};
use crate::Vec2;

/// Upper bound on subdivisions per curve. Keeps pathological inputs bounded.
const MAX_SEGMENTS: f32 = 65536.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Polyline {
    pub points: Vec<Vec2>,
    pub closed: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlattenedPath {
    pub polylines: Vec<Polyline>,
}

pub(crate) fn flatten(path: &Path, tolerance: f32) -> FlattenedPath {
    let tol = if tolerance > 0.0 && tolerance.is_finite() {
        tolerance
    } else {
        0.1
    };
    let mut out = FlattenedPath::default();
    let mut cur = Vec2::ZERO;
    let mut start = Vec2::ZERO;

    for cmd in path.commands() {
        match *cmd {
            PathCommand::MoveTo(p) => {
                out.polylines.push(Polyline {
                    points: alloc::vec![p],
                    closed: false,
                });
                cur = p;
                start = p;
            }
            PathCommand::LineTo(p) => {
                ensure_open(&mut out, cur);
                last_mut(&mut out).points.push(p);
                cur = p;
            }
            PathCommand::QuadTo(c, p) => {
                ensure_open(&mut out, cur);
                let (c1, c2, p3) = quad_to_cubic(cur, c, p);
                flatten_cubic(&mut out, cur, c1, c2, p3, tol);
                cur = p;
            }
            PathCommand::CubicTo(c1, c2, p) => {
                ensure_open(&mut out, cur);
                flatten_cubic(&mut out, cur, c1, c2, p, tol);
                cur = p;
            }
            PathCommand::Close => {
                if let Some(last) = out.polylines.last_mut() {
                    last.closed = true;
                }
                cur = start;
            }
        }
    }
    out
}

/// Starts a new polyline at `cur` when none is open (e.g. after `Close`).
fn ensure_open(out: &mut FlattenedPath, cur: Vec2) {
    let needs_new = match out.polylines.last() {
        None => true,
        Some(p) => p.closed,
    };
    if needs_new {
        out.polylines.push(Polyline {
            points: alloc::vec![cur],
            closed: false,
        });
    }
}

fn last_mut(out: &mut FlattenedPath) -> &mut Polyline {
    out.polylines
        .last_mut()
        .expect("ensure_open runs before every segment")
}

fn flatten_cubic(out: &mut FlattenedPath, p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, tol: f32) {
    let d1 = (p0 - p1 * 2.0 + p2).length();
    let d2 = (p1 - p2 * 2.0 + p3).length();
    let m = d1.max(d2);
    let n = libm::ceilf(libm::sqrtf(0.75 * m / tol)).clamp(1.0, MAX_SEGMENTS) as usize;
    let pts = &mut last_mut(out).points;
    for i in 1..n {
        let t = i as f32 / n as f32;
        let mt = 1.0 - t;
        let (a, b, c, d) = (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
        pts.push(p0 * a + p1 * b + p2 * c + p3 * d);
    }
    pts.push(p3);
}

impl FlattenedPath {
    /// Total number of points across all polylines.
    pub fn point_count(&self) -> usize {
        self.polylines.iter().map(|p| p.points.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PathBuilder;

    #[test]
    fn line_flattens_to_its_endpoints() {
        let mut b = PathBuilder::new();
        b.move_to(0.0, 0.0);
        b.line_to(4.0, 0.0);
        let f = b.build().flatten(0.1);
        assert_eq!(f.polylines.len(), 1);
        assert_eq!(
            f.polylines[0].points,
            alloc::vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)]
        );
    }

    #[test]
    fn closing_marks_polyline_and_reopens_after() {
        let mut b = PathBuilder::new();
        b.rect(0.0, 0.0, 1.0, 1.0);
        b.line_to(5.0, 5.0);
        let f = b.build().flatten(0.1);
        assert_eq!(f.polylines.len(), 2);
        assert!(f.polylines[0].closed);
        assert!(!f.polylines[1].closed);
        assert_eq!(f.polylines[1].points[0], Vec2::new(0.0, 0.0));
    }
}
