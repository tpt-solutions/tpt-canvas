//! Stub for `tpt-math`: 2D vectors, rectangles and affine transforms.
//!
//! Swap plan: replace this crate with `tpt-math`, keeping the same type names
//! and semantics. Affine composition follows `a * b == apply b, then a`.
// Stub crate: replaced by the real ecosystem crate, so its items are not documented individually.
#![allow(missing_docs)]
#![no_std]

use core::ops::{Add, Mul, Neg, Sub};

/// A point or vector in 2D.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
    }

    /// Scalar (z) component of the 3D cross product.
    pub fn cross(self, o: Vec2) -> f32 {
        self.x * o.y - self.y * o.x
    }

    pub fn length(self) -> f32 {
        libm::sqrtf(self.dot(self))
    }

    /// Unit vector in the same direction, or `self` unchanged when zero-length.
    pub fn normalized(self) -> Vec2 {
        let len = self.length();
        if len > 0.0 {
            self * (1.0 / len)
        } else {
            self
        }
    }

    /// Left-hand normal `(-y, x)`; rotates by +90 degrees.
    pub fn perp(self) -> Vec2 {
        Vec2::new(-self.y, self.x)
    }

    pub fn distance(self, o: Vec2) -> f32 {
        (self - o).length()
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

/// Axis-aligned rectangle: origin `(x, y)` and size `(w, h)`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const ZERO: Rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };

    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }

    pub fn from_min_max(min: Vec2, max: Vec2) -> Self {
        Rect::new(min.x, min.y, max.x - min.x, max.y - min.y)
    }

    pub fn min(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    pub fn max(&self) -> Vec2 {
        Vec2::new(self.x + self.w, self.y + self.h)
    }

    /// Smallest rectangle containing both `self` and `other`.
    pub fn union(&self, other: &Rect) -> Rect {
        let min = Vec2::new(self.x.min(other.x), self.y.min(other.y));
        let max = Vec2::new(
            (self.x + self.w).max(other.x + other.w),
            (self.y + self.h).max(other.y + other.h),
        );
        Rect::from_min_max(min, max)
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.y >= self.y && p.x <= self.x + self.w && p.y <= self.y + self.h
    }
}

/// Affine transform `[a c e; b d f; 0 0 1]`, mapping
/// `x' = a*x + c*y + e`, `y' = b*x + d*y + f`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine2 {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Default for Affine2 {
    fn default() -> Self {
        Affine2::IDENTITY
    }
}

impl Affine2 {
    pub const IDENTITY: Affine2 = Affine2 {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub const fn new(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self {
        Affine2 { a, b, c, d, e, f }
    }

    pub const fn translate(tx: f32, ty: f32) -> Self {
        Affine2::new(1.0, 0.0, 0.0, 1.0, tx, ty)
    }

    pub const fn scale(sx: f32, sy: f32) -> Self {
        Affine2::new(sx, 0.0, 0.0, sy, 0.0, 0.0)
    }

    /// Rotation by `theta` radians (positive turns from +x toward +y).
    pub fn rotate(theta: f32) -> Self {
        let (s, c) = (libm::sinf(theta), libm::cosf(theta));
        Affine2::new(c, s, -s, c, 0.0, 0.0)
    }

    pub fn apply(&self, p: Vec2) -> Vec2 {
        Vec2::new(
            self.a * p.x + self.c * p.y + self.e,
            self.b * p.x + self.d * p.y + self.f,
        )
    }

    pub fn determinant(&self) -> f32 {
        self.a * self.d - self.b * self.c
    }

    /// Inverse transform, or `None` when the matrix is (near-)singular.
    pub fn invert(&self) -> Option<Affine2> {
        let det = self.determinant();
        if !det.is_finite() || det.abs() <= 1e-12 {
            return None;
        }
        let inv = 1.0 / det;
        Some(Affine2::new(
            self.d * inv,
            -self.b * inv,
            -self.c * inv,
            self.a * inv,
            (self.c * self.f - self.d * self.e) * inv,
            (self.b * self.e - self.a * self.f) * inv,
        ))
    }

    /// Bounding box of the transformed rectangle.
    pub fn transform_rect(&self, r: &Rect) -> Rect {
        let corners = [
            self.apply(Vec2::new(r.x, r.y)),
            self.apply(Vec2::new(r.x + r.w, r.y)),
            self.apply(Vec2::new(r.x, r.y + r.h)),
            self.apply(Vec2::new(r.x + r.w, r.y + r.h)),
        ];
        let mut min = corners[0];
        let mut max = corners[0];
        for p in &corners[1..] {
            min = Vec2::new(min.x.min(p.x), min.y.min(p.y));
            max = Vec2::new(max.x.max(p.x), max.y.max(p.y));
        }
        Rect::from_min_max(min, max)
    }
}

/// `self * rhs` applies `rhs` first, then `self`.
impl Mul for Affine2 {
    type Output = Affine2;
    fn mul(self, r: Affine2) -> Affine2 {
        Affine2::new(
            self.a * r.a + self.c * r.b,
            self.b * r.a + self.d * r.b,
            self.a * r.c + self.c * r.d,
            self.b * r.c + self.d * r.d,
            self.a * r.e + self.c * r.f + self.e,
            self.b * r.e + self.d * r.f + self.f,
        )
    }
}
