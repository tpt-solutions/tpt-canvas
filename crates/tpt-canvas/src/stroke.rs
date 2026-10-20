//! Stroker: flattened polylines to outline polygons (caps, joins, miter limit, dashes).
//!
//! Each piece (segment rectangle, join wedge, cap) is emitted as a polygon wound
//! counter-clockwise. A nonzero fill of the whole set therefore yields the union,
//! which is what a stroke is. The outline is independent of how it is later
//! filled or clipped.

use alloc::{vec, vec::Vec};
use core::f32::consts::TAU;
use core::ops::Range;

use crate::path::{FlattenedPath, Polyline};
use crate::style::{LineCap, LineJoin, StrokeStyle};
use crate::Vec2;

/// Returns closed outline polygons for stroking `path` with `style`.
///
/// `tolerance` controls round-join and round-cap circle resolution. Output is in
/// the same space as `path`; transform either side to change coordinate space.
pub fn stroke_outline(path: &FlattenedPath, style: &StrokeStyle, tolerance: f32) -> Vec<Vec<Vec2>> {
    let hw = style.width * 0.5;
    if !hw.is_finite() || hw <= 0.0 {
        return Vec::new();
    }
    let tol = if tolerance > 0.0 { tolerance } else { 0.1 };
    let mut out = Vec::new();
    for poly in &path.polylines {
        match valid_dash(&style.dash) {
            Some(pattern) => {
                for piece in dash_polyline(poly, &pattern, style.dash_offset) {
                    stroke_polyline(&piece, false, hw, style, tol, &mut out);
                }
            }
            None => stroke_polyline(&poly.points, poly.closed, hw, style, tol, &mut out),
        }
    }
    out
}

fn stroke_polyline(
    raw: &[Vec2],
    closed: bool,
    hw: f32,
    style: &StrokeStyle,
    tol: f32,
    out: &mut Vec<Vec<Vec2>>,
) {
    let mut pts: Vec<Vec2> = Vec::with_capacity(raw.len());
    for &p in raw {
        if pts.last() != Some(&p) {
            pts.push(p);
        }
    }
    if pts.is_empty() {
        return;
    }
    if pts.len() == 1 {
        // Zero-length subpath: only round and square caps leave a mark.
        let p = pts[0];
        let x_axis = Vec2::new(1.0, 0.0);
        push_cap(out, p, x_axis, hw, style, tol);
        return;
    }
    let closed = closed && pts.len() >= 3;
    if closed && pts.first() == pts.last() {
        pts.pop();
    }
    let n = pts.len();
    let seg_count = if closed { n } else { n - 1 };
    let dir = |i: usize| (pts[(i + 1) % n] - pts[i]).normalized();

    for i in 0..seg_count {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let nrm = dir(i).perp() * hw;
        out.push(orient_ccw(vec![a + nrm, b + nrm, b - nrm, a - nrm]));
    }

    let joins: Range<usize> = if closed { 0..n } else { 1..n - 1 };
    for i in joins {
        join(pts[i], dir((i + n - 1) % n), dir(i), hw, style, tol, out);
    }

    if !closed {
        push_cap(out, pts[0], -dir(0), hw, style, tol);
        push_cap(out, pts[n - 1], dir(n - 2), hw, style, tol);
    }
}

/// Join wedge at `p` between incoming direction `d0` and outgoing `d1`.
fn join(
    p: Vec2,
    d0: Vec2,
    d1: Vec2,
    hw: f32,
    style: &StrokeStyle,
    tol: f32,
    out: &mut Vec<Vec<Vec2>>,
) {
    let cross = d0.cross(d1);
    if cross.abs() < 1e-7 {
        // Straight continuation needs nothing; a 180-degree reversal is covered by the
        // butt ends of the two segments, so only round joins add geometry.
        if style.join == LineJoin::Round && d0.dot(d1) < 0.0 {
            out.push(circle(p, hw, tol));
        }
        return;
    }
    if style.join == LineJoin::Round {
        out.push(circle(p, hw, tol));
        return;
    }
    // The outer side of the turn is opposite to the turn direction.
    let s = if cross > 0.0 { -1.0 } else { 1.0 };
    let n0 = d0.perp() * (hw * s);
    let n1 = d1.perp() * (hw * s);
    let bevel = || orient_ccw(vec![p, p + n0, p + n1]);
    if style.join == LineJoin::Bevel {
        out.push(bevel());
        return;
    }
    // Miter: tip lies on the bisector, at distance hw / cos(half the turn angle).
    let bis = (n0 + n1).normalized();
    let cos_half = bis.dot(n0.normalized());
    if cos_half > 1e-6 && 1.0 / cos_half <= style.miter_limit {
        let tip = p + bis * (hw / cos_half);
        out.push(orient_ccw(vec![p, p + n0, tip, p + n1]));
    } else {
        out.push(bevel());
    }
}

/// Cap at `p`. `dir` points outward from the end of the stroke.
fn push_cap(out: &mut Vec<Vec<Vec2>>, p: Vec2, dir: Vec2, hw: f32, style: &StrokeStyle, tol: f32) {
    match style.cap {
        LineCap::Butt => {}
        LineCap::Round => out.push(circle(p, hw, tol)),
        LineCap::Square => {
            let nrm = dir.perp() * hw;
            let ext = dir * hw;
            out.push(orient_ccw(vec![
                p + nrm,
                p + nrm + ext,
                p - nrm + ext,
                p - nrm,
            ]));
        }
    }
}

/// Circle polygon whose chord sagitta stays within `tol`.
fn circle(c: Vec2, r: f32, tol: f32) -> Vec<Vec2> {
    let n = if r > tol {
        let step = 2.0 * libm::acosf(1.0 - tol / r);
        libm::ceilf(TAU / step).clamp(8.0, 1024.0) as usize
    } else {
        8
    };
    (0..n)
        .map(|i| {
            let a = TAU * i as f32 / n as f32;
            c + Vec2::new(r * libm::cosf(a), r * libm::sinf(a))
        })
        .collect()
}

/// Reverses `pts` if needed so the polygon has positive (counter-clockwise) area.
fn orient_ccw(mut pts: Vec<Vec2>) -> Vec<Vec2> {
    let n = pts.len();
    let area: f32 = (0..n).map(|i| pts[i].cross(pts[(i + 1) % n])).sum();
    if area < 0.0 {
        pts.reverse();
    }
    pts
}

/// Returns an even-length pattern, or `None` for solid strokes.
fn valid_dash(dash: &[f32]) -> Option<Vec<f32>> {
    if dash.is_empty() || dash.iter().any(|d| *d < 0.0 || !d.is_finite()) {
        return None;
    }
    let sum: f32 = dash.iter().sum();
    if sum.is_nan() || sum <= 0.0 {
        return None;
    }
    let mut pattern = dash.to_vec();
    if pattern.len() % 2 == 1 {
        let copy = pattern.clone();
        pattern.extend(copy);
    }
    Some(pattern)
}

/// Splits a polyline into the "on" pieces of a dash pattern, as open polylines.
fn dash_polyline(poly: &Polyline, pattern: &[f32], offset: f32) -> Vec<Vec<Vec2>> {
    let m = pattern.len();
    let total: f32 = pattern.iter().sum();
    let mut phase = if offset.is_finite() {
        offset - total * libm::floorf(offset / total)
    } else {
        0.0
    };

    // Locate the dash element at the start offset.
    let mut idx = 0usize;
    let mut left = pattern[0];
    while phase > 0.0 {
        if phase >= left {
            phase -= left;
            idx = (idx + 1) % m;
            left = pattern[idx];
        } else {
            left -= phase;
            phase = 0.0;
        }
    }

    let pts = &poly.points;
    let mut pieces = Vec::new();
    if pts.is_empty() {
        return pieces;
    }
    let mut on = idx % 2 == 0;
    let mut current: Vec<Vec2> = if on { vec![pts[0]] } else { Vec::new() };
    let seg_total = if poly.closed {
        pts.len()
    } else {
        pts.len() - 1
    };

    for i in 0..seg_total {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        let len = (b - a).length();
        if len <= 0.0 {
            continue;
        }
        let mut pos = 0.0f32;
        while len - pos > left {
            pos += left;
            let p = a + (b - a) * (pos / len);
            if on {
                current.push(p);
                pieces.push(core::mem::take(&mut current));
            } else {
                current = vec![p];
            }
            on = !on;
            idx = (idx + 1) % m;
            left = pattern[idx];
        }
        left -= len - pos;
        if on {
            current.push(b);
        }
    }
    if on && current.len() >= 2 {
        pieces.push(current);
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::Path;
    use crate::PathBuilder;
    use tpt_raster_stub::{Rasterizer, ScanlineRasterizer};

    fn line(a: Vec2, b: Vec2) -> FlattenedPath {
        let mut pb = PathBuilder::new();
        pb.move_to(a.x, a.y);
        pb.line_to(b.x, b.y);
        pb.build().flatten(0.01)
    }

    /// Area of the union of `polys`, measured by rasterizing at 1 px per unit.
    fn union_area(polys: &[Vec<Vec2>], w: u32, h: u32) -> f32 {
        let cov = ScanlineRasterizer { subsamples: 16 }.rasterize(polys, false, w, h);
        cov.data.iter().map(|&c| c as f32 / 255.0).sum()
    }

    fn shift(polys: &[Vec<Vec2>], d: Vec2) -> Vec<Vec<Vec2>> {
        polys
            .iter()
            .map(|p| p.iter().map(|q| *q + d).collect())
            .collect()
    }

    #[test]
    fn butt_line_is_a_rectangle() {
        let style = StrokeStyle {
            width: 2.0,
            ..StrokeStyle::default()
        };
        let out = stroke_outline(
            &line(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)),
            &style,
            0.01,
        );
        assert_eq!(out.len(), 1);
        let shifted = shift(&out, Vec2::new(0.0, 2.0));
        assert!((union_area(&shifted, 12, 4) - 20.0).abs() < 0.05);
    }

    #[test]
    fn square_caps_extend_by_half_width() {
        let style = StrokeStyle {
            width: 2.0,
            cap: LineCap::Square,
            ..StrokeStyle::default()
        };
        let out = stroke_outline(
            &line(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)),
            &style,
            0.01,
        );
        // Caps are on the sides of the x-range; shift the line so both caps are on-canvas.
        let shifted: Vec<Vec<Vec2>> = out
            .iter()
            .map(|p| p.iter().map(|q| *q + Vec2::new(2.0, 4.0)).collect())
            .collect();
        assert!((union_area(&shifted, 16, 10) - 24.0).abs() < 0.05);
    }

    #[test]
    fn dashes_split_length_evenly() {
        // Length 10 with on=2, off=2 gives on-pieces [0,2], [4,6], [8,10].
        let style = StrokeStyle {
            width: 1.0,
            dash: alloc::vec![2.0, 2.0],
            ..StrokeStyle::default()
        };
        let out = stroke_outline(
            &line(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)),
            &style,
            0.01,
        );
        assert_eq!(out.len(), 3);
        let shifted: Vec<Vec<Vec2>> = out
            .iter()
            .map(|p| p.iter().map(|q| *q + Vec2::new(0.0, 2.0)).collect())
            .collect();
        assert!((union_area(&shifted, 12, 6) - 6.0).abs() < 0.05);
    }

    #[test]
    fn miter_join_on_right_angle_fills_outer_corner() {
        let mut pb = PathBuilder::new();
        pb.move_to(2.0, 2.0);
        pb.line_to(12.0, 2.0);
        pb.line_to(12.0, 12.0);
        let flat = pb.build().flatten(0.01);
        let style = StrokeStyle {
            width: 2.0,
            ..StrokeStyle::default()
        };
        let out = stroke_outline(&flat, &style, 0.01);
        // Two 10x2 bars overlapping in a 1x1 square, plus the 1x1 outer miter square.
        let area = union_area(&out, 16, 16);
        assert!((area - 40.0).abs() < 0.1, "area = {area}");
    }

    #[test]
    fn bevel_join_drops_the_miter_corner() {
        let mut pb = PathBuilder::new();
        pb.move_to(2.0, 2.0);
        pb.line_to(12.0, 2.0);
        pb.line_to(12.0, 12.0);
        let flat = pb.build().flatten(0.01);
        let style = StrokeStyle {
            width: 2.0,
            join: LineJoin::Bevel,
            ..StrokeStyle::default()
        };
        let area = union_area(&stroke_outline(&flat, &style, 0.01), 16, 16);
        // Bevel removes the half-unit triangle the miter would have added.
        assert!((area - 39.5).abs() < 0.1, "area = {area}");
    }

    #[test]
    fn empty_path_has_no_outline() {
        let flat = Path::new().flatten(0.1);
        assert!(stroke_outline(&flat, &StrokeStyle::default(), 0.1).is_empty());
    }
}
