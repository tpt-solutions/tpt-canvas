//! Point containment for fills and strokes.
//!
//! Curves are flattened at a fixed tolerance, so a point within about that distance of
//! an edge can go either way. The tolerance is far below one device pixel for typical
//! scales.

use super::Path;
use crate::stroke::stroke_outline;
use crate::style::{FillRule, StrokeStyle};
use crate::Vec2;

/// Flattening tolerance used for hit tests, in user units.
const HIT_TOLERANCE: f32 = 0.01;

impl Path {
    /// Whether `p` lies inside the filled area under `rule`. Every subpath is closed for filling.
    pub fn contains_point(&self, p: Vec2, rule: FillRule) -> bool {
        let flat = self.flatten(HIT_TOLERANCE);
        match rule {
            FillRule::NonZero => {
                let winding: i32 = flat
                    .polylines
                    .iter()
                    .map(|poly| winding_number(&poly.points, p))
                    .sum();
                winding != 0
            }
            FillRule::EvenOdd => {
                // A point is inside when a ray to the right crosses an odd number of edges.
                let crossings: u32 = flat
                    .polylines
                    .iter()
                    .map(|poly| ray_crossings(&poly.points, p))
                    .sum();
                crossings % 2 == 1
            }
        }
    }

    /// Whether `p` lies inside the stroked outline of this path with `style`.
    pub fn stroke_contains_point(&self, p: Vec2, style: &StrokeStyle) -> bool {
        let flat = self.flatten(HIT_TOLERANCE);
        let outline = stroke_outline(&flat, style, HIT_TOLERANCE);
        // The stroker emits counter-clockwise polygons whose union is the stroke,
        // so a non-zero winding over all of them tests the union.
        outline
            .iter()
            .map(|poly| winding_number(poly, p))
            .sum::<i32>()
            != 0
    }
}

/// Winding number of the closed polygon `pts` around `p` (Sunday's crossing rule).
fn winding_number(pts: &[Vec2], p: Vec2) -> i32 {
    let n = pts.len();
    if n < 2 {
        return 0;
    }
    let mut wn = 0;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let left = (b.x - a.x) * (p.y - a.y) - (p.x - a.x) * (b.y - a.y);
        if a.y <= p.y {
            if b.y > p.y && left > 0.0 {
                wn += 1;
            }
        } else if b.y <= p.y && left < 0.0 {
            wn -= 1;
        }
    }
    wn
}

/// Number of edges of the closed polygon `pts` that a rightward ray from `p` crosses.
fn ray_crossings(pts: &[Vec2], p: Vec2) -> u32 {
    let n = pts.len();
    if n < 2 {
        return 0;
    }
    let mut count = 0;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        if (a.y <= p.y) != (b.y <= p.y) {
            // x where the edge crosses the horizontal line through p.
            let t = (p.y - a.y) / (b.y - a.y);
            if a.x + t * (b.x - a.x) > p.x {
                count += 1;
            }
        }
    }
    count
}
