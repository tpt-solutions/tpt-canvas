//! Ergonomic path construction, shape helpers, and the arc converter.
//!
//! ArcConverter notes:
//! - Every arc is split into segments of at most 90 degrees.
//! - Each segment is a cubic with handle length `4/3 * tan(sweep / 4)`, whose radial
//!   error is about 2.7e-4 of the radius at 90 degrees and smaller for shorter segments.
//! - Three entry points match the three arc definitions in play:
//!   [`PathBuilder::arc`] (HTML5 `arc()`), [`PathBuilder::arc_to`] (HTML5
//!   tangent `arcTo()`), and [`PathBuilder::elliptical_arc`] (SVG endpoint arc).

use core::f32::consts::{FRAC_PI_2, PI, TAU};

use super::{Path, PathCommand};
use crate::Vec2;

/// Builds a [`Path`]. Tracks the current point and subpath start for canvas-style
/// semantics: a segment after `close` starts a new subpath at the closed start point.
#[derive(Clone, Debug, Default)]
pub struct PathBuilder {
    path: Path,
    current: Option<Vec2>,
    start: Vec2,
    closed: bool,
}

impl PathBuilder {
    pub fn new() -> Self {
        PathBuilder::default()
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        let p = Vec2::new(x, y);
        self.path.commands.push(PathCommand::MoveTo(p));
        self.current = Some(p);
        self.start = p;
        self.closed = false;
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        let p = Vec2::new(x, y);
        if self.current.is_none() {
            self.move_to(x, y);
            return;
        }
        self.reopen();
        self.path.commands.push(PathCommand::LineTo(p));
        self.current = Some(p);
    }

    pub fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (c, p) = (Vec2::new(cx, cy), Vec2::new(x, y));
        self.ensure_subpath(c);
        self.path.commands.push(PathCommand::QuadTo(c, p));
        self.current = Some(p);
    }

    pub fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        let (c1, c2, p) = (Vec2::new(c1x, c1y), Vec2::new(c2x, c2y), Vec2::new(x, y));
        self.ensure_subpath(c1);
        self.path.commands.push(PathCommand::CubicTo(c1, c2, p));
        self.current = Some(p);
    }

    pub fn close(&mut self) {
        if self.current.is_some() {
            self.path.commands.push(PathCommand::Close);
            self.current = Some(self.start);
            self.closed = true;
        }
    }

    /// HTML5 `arc()`: circle of radius `r` at `(cx, cy)` from `start` to `end` radians.
    /// `ccw` selects counter-clockwise travel. Sweeps of a full turn or more draw a full circle.
    /// Negative or non-finite radii are ignored.
    pub fn arc(&mut self, cx: f32, cy: f32, r: f32, start: f32, end: f32, ccw: bool) {
        if !r.is_finite() || r < 0.0 {
            return;
        }
        let sweep = html5_sweep(start, end, ccw);
        let center = Vec2::new(cx, cy);
        let p0 = center + Vec2::new(r * libm::cosf(start), r * libm::sinf(start));
        self.join_to(p0);
        self.emit_arc(center, r, r, 0.0, start, sweep);
    }

    /// HTML5 `arcTo()`: tangent arc of radius `r` from the current point through
    /// `(x1, y1)` toward `(x2, y2)`. Degenerate inputs reduce to a line.
    pub fn arc_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, r: f32) {
        let p1 = Vec2::new(x1, y1);
        let p2 = Vec2::new(x2, y2);
        let p0 = match self.current {
            Some(p) => p,
            None => {
                self.move_to(x1, y1);
                return;
            }
        };
        if !r.is_finite() || r <= 0.0 || p0 == p1 || p1 == p2 {
            self.line_to(x1, y1);
            return;
        }
        let u0 = (p0 - p1).normalized();
        let u2 = (p2 - p1).normalized();
        if u0.cross(u2).abs() < 1e-7 {
            self.line_to(x1, y1);
            return;
        }
        let half = libm::acosf(u0.dot(u2).clamp(-1.0, 1.0)) * 0.5;
        // Nearly straight corners put the tangent points arbitrarily far away; treat them as lines.
        let tangent_len = r / libm::tanf(half);
        let t1 = p1 + u0 * tangent_len;
        let t2 = p1 + u2 * tangent_len;
        let bisector = (u0 + u2).normalized();
        let center = p1 + bisector * (r / libm::sinf(half));
        let finite = [t1, t2, center]
            .iter()
            .all(|v| v.x.is_finite() && v.y.is_finite());
        if half < 1e-4 || !finite {
            self.line_to(x1, y1);
            return;
        }

        self.line_to(t1.x, t1.y);
        let th1 = libm::atan2f(t1.y - center.y, t1.x - center.x);
        let th2 = libm::atan2f(t2.y - center.y, t2.x - center.x);
        let sweep = wrap_signed(th2 - th1);
        self.emit_arc(center, r, r, 0.0, th1, sweep);
        self.current = Some(t2);
    }

    /// SVG endpoint arc from the current point to `(x, y)`.
    /// `x_rotation` is in radians. Out-of-range radii are scaled up as in SVG F.6.6.
    #[allow(clippy::too_many_arguments)]
    pub fn elliptical_arc(
        &mut self,
        rx: f32,
        ry: f32,
        x_rotation: f32,
        large_arc: bool,
        sweep: bool,
        x: f32,
        y: f32,
    ) {
        let p2 = Vec2::new(x, y);
        let p1 = match self.current {
            Some(p) => p,
            None => {
                self.move_to(x, y);
                return;
            }
        };
        if p1 == p2 {
            return;
        }
        let (mut rx, mut ry) = (rx.abs(), ry.abs());
        if rx == 0.0 || ry == 0.0 {
            self.line_to(x, y);
            return;
        }
        let (sin_phi, cos_phi) = (libm::sinf(x_rotation), libm::cosf(x_rotation));
        let half = (p1 - p2) * 0.5;
        let x1p = cos_phi * half.x + sin_phi * half.y;
        let y1p = -sin_phi * half.x + cos_phi * half.y;

        let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
        if lambda > 1.0 {
            let s = libm::sqrtf(lambda);
            rx *= s;
            ry *= s;
        }
        let (rx2, ry2) = (rx * rx, ry * ry);
        let num = rx2 * ry2 - rx2 * y1p * y1p - ry2 * x1p * x1p;
        let den = rx2 * y1p * y1p + ry2 * x1p * x1p;
        let ratio = num / den;
        let mut coef = if ratio.is_finite() {
            libm::sqrtf(ratio.max(0.0))
        } else {
            0.0
        };
        if large_arc == sweep {
            coef = -coef;
        }
        let cxp = coef * rx * y1p / ry;
        let cyp = -coef * ry * x1p / rx;
        let mid = (p1 + p2) * 0.5;
        let center = Vec2::new(
            cos_phi * cxp - sin_phi * cyp + mid.x,
            sin_phi * cxp + cos_phi * cyp + mid.y,
        );
        if !(center.x.is_finite() && center.y.is_finite()) {
            self.line_to(x, y);
            return;
        }

        let u = Vec2::new((x1p - cxp) / rx, (y1p - cyp) / ry);
        let v = Vec2::new((-x1p - cxp) / rx, (-y1p - cyp) / ry);
        let theta1 = libm::atan2f(u.y, u.x);
        let mut dtheta = libm::atan2f(u.cross(v), u.dot(v));
        if !sweep && dtheta > 0.0 {
            dtheta -= TAU;
        } else if sweep && dtheta < 0.0 {
            dtheta += TAU;
        }
        self.emit_arc(center, rx, ry, x_rotation, theta1, dtheta);
        self.current = Some(p2);
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.move_to(x, y);
        self.line_to(x + w, y);
        self.line_to(x + w, y + h);
        self.line_to(x, y + h);
        self.close();
    }

    /// Rectangle with corner radius `r`, clamped to half the smaller side.
    pub fn rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let r = r.max(0.0).min(w.abs().min(h.abs()) * 0.5);
        if r == 0.0 {
            self.rect(x, y, w, h);
            return;
        }
        let (x1, y1) = (x + w, y + h);
        self.move_to(x + r, y);
        self.line_to(x1 - r, y);
        self.arc(x1 - r, y + r, r, -FRAC_PI_2, 0.0, false);
        self.line_to(x1, y1 - r);
        self.arc(x1 - r, y1 - r, r, 0.0, FRAC_PI_2, false);
        self.line_to(x + r, y1);
        self.arc(x + r, y1 - r, r, FRAC_PI_2, PI, false);
        self.line_to(x, y + r);
        self.arc(x + r, y + r, r, PI, 3.0 * FRAC_PI_2, false);
        self.close();
    }

    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) {
        if !(rx > 0.0 && ry > 0.0) {
            return;
        }
        let center = Vec2::new(cx, cy);
        self.move_to(cx + rx, cy);
        self.emit_arc(center, rx, ry, 0.0, 0.0, TAU);
        self.close();
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32) {
        self.ellipse(cx, cy, r, r);
    }

    pub fn build(self) -> Path {
        self.path
    }

    /// Starts a subpath at `p` if none is active, or draws a line to it.
    fn join_to(&mut self, p: Vec2) {
        if self.current.is_none() {
            self.move_to(p.x, p.y);
            return;
        }
        self.reopen();
        // Computed arc start points carry rounding noise; treat near-coincident points as equal.
        let near = self.current.is_some_and(|c| c.distance(p) <= 1e-5);
        if !near {
            self.line_to(p.x, p.y);
        }
    }

    /// Canvas semantics: a segment with no current point starts a subpath at the
    /// control point (or endpoint) given first.
    fn ensure_subpath(&mut self, implicit_start: Vec2) {
        if self.current.is_none() {
            self.move_to(implicit_start.x, implicit_start.y);
        }
        self.reopen();
    }

    /// After `close`, the next segment starts a new subpath at the closed start point.
    fn reopen(&mut self) {
        if self.closed {
            let s = self.start;
            self.path.commands.push(PathCommand::MoveTo(s));
            self.current = Some(s);
            self.closed = false;
        }
    }

    /// Emits cubic segments of an ellipse arc, starting at the angle `theta1` (the
    /// current point must already lie on it). Each segment spans at most 90 degrees.
    fn emit_arc(&mut self, center: Vec2, rx: f32, ry: f32, rot: f32, theta1: f32, dtheta: f32) {
        self.reopen();
        if dtheta == 0.0 {
            return;
        }
        let n = libm::ceilf(libm::fabsf(dtheta) / FRAC_PI_2).max(1.0) as usize;
        let step = dtheta / n as f32;
        let alpha = arc_alpha(step);
        for i in 0..n {
            let t0 = theta1 + step * i as f32;
            let t1 = t0 + step;
            let (p0, d0) = ellipse_point(center, rx, ry, rot, t0);
            let (p1, d1) = ellipse_point(center, rx, ry, rot, t1);
            let c1 = p0 + d0 * alpha;
            let c2 = p1 - d1 * alpha;
            self.path.commands.push(PathCommand::CubicTo(c1, c2, p1));
            self.current = Some(p1);
        }
    }
}

/// Position and derivative of a rotated ellipse at parameter `t`.
fn ellipse_point(center: Vec2, rx: f32, ry: f32, rot: f32, t: f32) -> (Vec2, Vec2) {
    let (s, c) = (libm::sinf(t), libm::cosf(t));
    let (sr, cr) = (libm::sinf(rot), libm::cosf(rot));
    let rotate = |v: Vec2| Vec2::new(cr * v.x - sr * v.y, sr * v.x + cr * v.y);
    let pos = rotate(Vec2::new(rx * c, ry * s));
    let deriv = rotate(Vec2::new(-rx * s, ry * c));
    (center + pos, deriv)
}

/// Handle length factor for a cubic approximating an arc of angle `dtheta`:
/// `4/3 * tan(dtheta / 4)`, the standard choice that matches position and tangent exactly
/// at both ends. For 90 degrees it is about 0.5523, with radial error near 2.7e-4.
fn arc_alpha(dtheta: f32) -> f32 {
    4.0 / 3.0 * libm::tanf(dtheta * 0.25)
}

/// Sweep angle of an HTML5 `arc()` call.
fn html5_sweep(start: f32, end: f32, ccw: bool) -> f32 {
    if !ccw {
        let d = end - start;
        if d >= TAU {
            TAU
        } else {
            positive_mod(d)
        }
    } else {
        let d = start - end;
        if d >= TAU {
            -TAU
        } else {
            -positive_mod(d)
        }
    }
}

fn positive_mod(a: f32) -> f32 {
    let m = libm::fmodf(a, TAU);
    if m < 0.0 {
        m + TAU
    } else {
        m
    }
}

/// Wraps an angle difference into `(-PI, PI]`.
fn wrap_signed(a: f32) -> f32 {
    let mut d = a;
    while d > PI {
        d -= TAU;
    }
    while d <= -PI {
        d += TAU;
    }
    d
}
