//! Path geometry (Module A).
//!
//! A [`Path`] stores exact curves (lines, quadratics, cubics). Arcs are lowered
//! to cubics by [`PathBuilder`]. [`Path::flatten`] is the only way curves become
//! line segments.

mod builder;
mod flatten;

use alloc::vec::Vec;
use core::fmt;

use crate::{Affine2, Rect, Vec2};

pub use builder::PathBuilder;
pub use flatten::{FlattenedPath, Polyline};

#[derive(Clone, Copy, Debug, PartialEq)]
/// One segment of a path.
pub enum PathCommand {
    /// Starts a new subpath at a point.
    MoveTo(Vec2),
    /// A straight line to a point.
    LineTo(Vec2),
    /// A quadratic Bezier curve, with control point and end point.
    QuadTo(Vec2, Vec2),
    /// A cubic Bezier curve, with two control points and an end point.
    CubicTo(Vec2, Vec2, Vec2),
    /// Closes the current subpath with a straight line to its start.
    Close,
}

#[derive(Clone, Debug, Default, PartialEq)]
/// A sequence of path commands. Curves are kept exact until flattened.
pub struct Path {
    pub(crate) commands: Vec<PathCommand>,
}

/// Errors from [`Path::deserialize`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The input does not start with the `TPTP` magic bytes.
    BadMagic,
    /// The format version is not supported.
    UnsupportedVersion(u8),
    /// An unknown command tag.
    UnknownTag(u8),
    /// The input ended early.
    Truncated,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::BadMagic => write!(f, "bad magic"),
            DecodeError::UnsupportedVersion(v) => write!(f, "unsupported version {v}"),
            DecodeError::UnknownTag(t) => write!(f, "unknown command tag {t}"),
            DecodeError::Truncated => write!(f, "truncated buffer"),
        }
    }
}

const MAGIC: &[u8; 4] = b"TPTP";
const VERSION: u8 = 1;

impl Path {
    /// Creates an empty path.
    pub fn new() -> Self {
        Path::default()
    }

    /// The commands in order.
    pub fn commands(&self) -> &[PathCommand] {
        &self.commands
    }

    /// Whether the path has no commands.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Flattens all curves into polylines with a maximum deviation of `tolerance`.
    pub fn flatten(&self, tolerance: f32) -> FlattenedPath {
        flatten::flatten(self, tolerance)
    }

    /// Exact bounding box, including curve extrema. Empty paths give [`Rect::ZERO`].
    pub fn bounding_box(&self) -> Rect {
        let mut acc: Option<(Vec2, Vec2)> = None;
        let mut cur = Vec2::ZERO;
        let mut start = Vec2::ZERO;
        for cmd in &self.commands {
            match *cmd {
                PathCommand::MoveTo(p) => {
                    include_point(&mut acc, p);
                    cur = p;
                    start = p;
                }
                PathCommand::LineTo(p) => {
                    include_point(&mut acc, p);
                    cur = p;
                }
                PathCommand::QuadTo(c, p) => {
                    let (c1, c2, p3) = quad_to_cubic(cur, c, p);
                    include_cubic(&mut acc, cur, c1, c2, p3);
                    cur = p;
                }
                PathCommand::CubicTo(c1, c2, p) => {
                    include_cubic(&mut acc, cur, c1, c2, p);
                    cur = p;
                }
                PathCommand::Close => cur = start,
            }
        }
        match acc {
            Some((min, max)) => Rect::from_min_max(min, max),
            None => Rect::ZERO,
        }
    }

    /// Returns a copy with every point mapped through `m`. Exact for curves,
    /// because affine maps commute with Bézier evaluation.
    pub fn transform(&self, m: &Affine2) -> Path {
        let commands = self
            .commands
            .iter()
            .map(|cmd| match *cmd {
                PathCommand::MoveTo(p) => PathCommand::MoveTo(m.apply(p)),
                PathCommand::LineTo(p) => PathCommand::LineTo(m.apply(p)),
                PathCommand::QuadTo(c, p) => PathCommand::QuadTo(m.apply(c), m.apply(p)),
                PathCommand::CubicTo(c1, c2, p) => {
                    PathCommand::CubicTo(m.apply(c1), m.apply(c2), m.apply(p))
                }
                PathCommand::Close => PathCommand::Close,
            })
            .collect();
        Path { commands }
    }

    /// Compact little-endian encoding: `"TPTP"`, version byte, then per command a
    /// tag byte (0 move, 1 line, 2 quad, 3 cubic, 4 close) followed by `f32` coords.
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(5 + self.commands.len() * 9);
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        for cmd in &self.commands {
            match *cmd {
                PathCommand::MoveTo(p) => {
                    out.push(0);
                    put_points(&mut out, &[p]);
                }
                PathCommand::LineTo(p) => {
                    out.push(1);
                    put_points(&mut out, &[p]);
                }
                PathCommand::QuadTo(c, p) => {
                    out.push(2);
                    put_points(&mut out, &[c, p]);
                }
                PathCommand::CubicTo(c1, c2, p) => {
                    out.push(3);
                    put_points(&mut out, &[c1, c2, p]);
                }
                PathCommand::Close => out.push(4),
            }
        }
        out
    }

    /// Decodes a path written by `serialize`.
    pub fn deserialize(bytes: &[u8]) -> Result<Path, DecodeError> {
        if bytes.len() < 5 {
            return Err(DecodeError::Truncated);
        }
        if &bytes[0..4] != MAGIC {
            return Err(DecodeError::BadMagic);
        }
        if bytes[4] != VERSION {
            return Err(DecodeError::UnsupportedVersion(bytes[4]));
        }
        let mut cur = &bytes[5..];
        let mut commands = Vec::new();
        while let Some((&tag, rest)) = cur.split_first() {
            cur = rest;
            let (count, make): (usize, fn(&[Vec2]) -> PathCommand) = match tag {
                0 => (1, |p| PathCommand::MoveTo(p[0])),
                1 => (1, |p| PathCommand::LineTo(p[0])),
                2 => (2, |p| PathCommand::QuadTo(p[0], p[1])),
                3 => (3, |p| PathCommand::CubicTo(p[0], p[1], p[2])),
                4 => {
                    commands.push(PathCommand::Close);
                    continue;
                }
                t => return Err(DecodeError::UnknownTag(t)),
            };
            let need = count * 8;
            if cur.len() < need {
                return Err(DecodeError::Truncated);
            }
            let mut pts = [Vec2::ZERO; 3];
            for (i, pt) in pts.iter_mut().take(count).enumerate() {
                let o = i * 8;
                pt.x = f32::from_le_bytes([cur[o], cur[o + 1], cur[o + 2], cur[o + 3]]);
                pt.y = f32::from_le_bytes([cur[o + 4], cur[o + 5], cur[o + 6], cur[o + 7]]);
            }
            cur = &cur[need..];
            commands.push(make(&pts[..count]));
        }
        Ok(Path { commands })
    }
}

fn put_points(out: &mut Vec<u8>, pts: &[Vec2]) {
    for p in pts {
        out.extend_from_slice(&p.x.to_le_bytes());
        out.extend_from_slice(&p.y.to_le_bytes());
    }
}

/// Degree-elevates a quadratic segment `(p0, c, p1)` to cubic control points.
pub fn quad_to_cubic(p0: Vec2, c: Vec2, p1: Vec2) -> (Vec2, Vec2, Vec2) {
    let c1 = p0 + (c - p0) * (2.0 / 3.0);
    let c2 = p1 + (c - p1) * (2.0 / 3.0);
    (c1, c2, p1)
}

fn include_point(acc: &mut Option<(Vec2, Vec2)>, p: Vec2) {
    *acc = Some(match *acc {
        None => (p, p),
        Some((min, max)) => (
            Vec2::new(min.x.min(p.x), min.y.min(p.y)),
            Vec2::new(max.x.max(p.x), max.y.max(p.y)),
        ),
    });
}

fn eval_cubic(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let mt = 1.0 - t;
    let a = mt * mt * mt;
    let b = 3.0 * mt * mt * t;
    let c = 3.0 * mt * t * t;
    let d = t * t * t;
    p0 * a + p1 * b + p2 * c + p3 * d
}

/// Adds the endpoint and every interior extremum of a cubic to the bounds.
fn include_cubic(acc: &mut Option<(Vec2, Vec2)>, p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2) {
    include_point(acc, p3);
    let mut roots: [f32; 4] = [0.0; 4];
    let mut n = 0;
    for axis in 0..2 {
        let get = |p: Vec2| if axis == 0 { p.x } else { p.y };
        let (q0, q1, q2, q3) = (get(p0), get(p1), get(p2), get(p3));
        // B'(t) / 3 = A t^2 + B t + C
        let a = -q0 + 3.0 * q1 - 3.0 * q2 + q3;
        let b = 2.0 * (q0 - 2.0 * q1 + q2);
        let c = q1 - q0;
        let mut push = |t: f32| {
            if t > 0.0 && t < 1.0 && n < roots.len() {
                roots[n] = t;
                n += 1;
            }
        };
        if a.abs() < 1e-12 {
            if b.abs() > 1e-12 {
                push(-c / b);
            }
        } else {
            let disc = b * b - 4.0 * a * c;
            if disc >= 0.0 {
                let sq = libm::sqrtf(disc);
                push((-b + sq) / (2.0 * a));
                push((-b - sq) / (2.0 * a));
            }
        }
    }
    for &t in &roots[..n] {
        include_point(acc, eval_cubic(p0, p1, p2, p3, t));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PathBuilder;

    #[test]
    fn bounding_box_of_line_and_rect() {
        let mut b = PathBuilder::new();
        b.rect(1.0, 2.0, 3.0, 4.0);
        let r = b.build().bounding_box();
        assert_eq!(r, Rect::new(1.0, 2.0, 3.0, 4.0));
    }

    #[test]
    fn bounding_box_includes_cubic_extremum() {
        // Symmetric bump: peak at t = 0.5, y = 0.75 * height.
        let mut b = PathBuilder::new();
        b.move_to(0.0, 0.0);
        b.cubic_to(0.0, 10.0, 10.0, 10.0, 10.0, 0.0);
        let r = b.build().bounding_box();
        assert!((r.max().y - 7.5).abs() < 1e-4, "max y = {}", r.max().y);
        assert_eq!(r.min().y, 0.0);
    }

    #[test]
    fn serialize_round_trip() {
        let mut b = PathBuilder::new();
        b.move_to(0.5, -2.0);
        b.quad_to(1.0, 1.0, 2.0, 0.0);
        b.cubic_to(3.0, 1.0, 4.0, -1.0, 5.0, 0.0);
        b.line_to(9.0, 9.0);
        b.close();
        let path = b.build();
        let bytes = path.serialize();
        assert_eq!(Path::deserialize(&bytes).unwrap(), path);
    }

    #[test]
    fn deserialize_rejects_garbage() {
        assert_eq!(Path::deserialize(b"nope!"), Err(DecodeError::BadMagic));
        assert_eq!(Path::deserialize(b"TPT"), Err(DecodeError::Truncated));
        let mut bytes = Path::new().serialize();
        bytes.push(9);
        assert_eq!(Path::deserialize(&bytes), Err(DecodeError::UnknownTag(9)));
    }
}
