//! Stub for `tpt-raster`: a scanline anti-aliased coverage rasterizer behind a trait.
//!
//! Output is an 8-bit coverage mask. Coverage is computed by sampling several
//! sub-scanlines per pixel and taking exact horizontal overlap per sample, which
//! gives smooth edges without floating-point drift between platforms that
//! matter for golden tests (all arithmetic is plain f32 in a fixed order).
// Stub crate: replaced by the real ecosystem crate, so its items are not documented individually.
#![allow(missing_docs)]
#![no_std]

extern crate alloc;

use alloc::{vec, vec::Vec};
use core::cmp::Ordering;
use tpt_math_stub::Vec2;

/// Per-pixel coverage, `0` = outside, `255` = fully inside.
#[derive(Clone, Debug, PartialEq)]
pub struct Coverage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Coverage {
    pub fn new(width: u32, height: u32) -> Self {
        Coverage {
            width,
            height,
            data: vec![0; (width as usize) * (height as usize)],
        }
    }

    pub fn full(width: u32, height: u32) -> Self {
        Coverage {
            width,
            height,
            data: vec![255; (width as usize) * (height as usize)],
        }
    }

    pub fn at(&self, x: u32, y: u32) -> u8 {
        self.data[(y as usize) * (self.width as usize) + x as usize]
    }

    /// Multiplies `self` by `other` (intersection of two masks).
    pub fn intersect(&mut self, other: &Coverage) {
        debug_assert_eq!(self.data.len(), other.data.len());
        for (d, o) in self.data.iter_mut().zip(other.data.iter()) {
            *d = ((*d as u32 * *o as u32 + 127) / 255) as u8;
        }
    }
}

/// Turns closed polygons into a coverage mask. Polygons are implicitly closed.
pub trait Rasterizer {
    fn rasterize(
        &self,
        polygons: &[Vec<Vec2>],
        even_odd: bool,
        width: u32,
        height: u32,
    ) -> Coverage;
}

/// Scanline rasterizer with `subsamples` sub-scanlines per pixel row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanlineRasterizer {
    pub subsamples: u32,
}

impl Default for ScanlineRasterizer {
    fn default() -> Self {
        ScanlineRasterizer { subsamples: 4 }
    }
}

impl Rasterizer for ScanlineRasterizer {
    fn rasterize(
        &self,
        polygons: &[Vec<Vec2>],
        even_odd: bool,
        width: u32,
        height: u32,
    ) -> Coverage {
        let mut cov = Coverage::new(width, height);
        if width == 0 || height == 0 {
            return cov;
        }
        let w = width as usize;
        let s = self.subsamples.max(1) as usize;
        let inv_s = 1.0 / s as f32;
        let mut acc = vec![0.0f32; w];
        let mut crossings: Vec<(f32, i32)> = Vec::new();

        for row in 0..height as usize {
            for v in acc.iter_mut() {
                *v = 0.0;
            }
            for k in 0..s {
                let y = row as f32 + (k as f32 + 0.5) * inv_s;
                crossings.clear();
                for poly in polygons {
                    let n = poly.len();
                    if n < 2 {
                        continue;
                    }
                    for i in 0..n {
                        let a = poly[i];
                        let b = poly[(i + 1) % n];
                        if a.y == b.y {
                            continue;
                        }
                        let (dir, lo, hi) = if a.y < b.y { (1, a, b) } else { (-1, b, a) };
                        if y < lo.y || y >= hi.y {
                            continue;
                        }
                        let x = lo.x + (y - lo.y) * (hi.x - lo.x) / (hi.y - lo.y);
                        crossings.push((x, dir));
                    }
                }
                crossings.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap_or(Ordering::Equal));

                let mut winding = 0i32;
                let mut span_start = 0.0f32;
                let mut inside = false;
                for &(x, dir) in crossings.iter() {
                    winding += dir;
                    let now = if even_odd {
                        winding & 1 != 0
                    } else {
                        winding != 0
                    };
                    if now && !inside {
                        span_start = x;
                        inside = true;
                    } else if !now && inside {
                        add_span(&mut acc, span_start, x, inv_s);
                        inside = false;
                    }
                }
            }
            let base = row * w;
            for (x, &c) in acc.iter().enumerate() {
                cov.data[base + x] = (c.min(1.0) * 255.0 + 0.5) as u8;
            }
        }
        cov
    }
}

/// Adds `weight * overlap(pixel, [x0, x1))` to every pixel the span touches.
fn add_span(acc: &mut [f32], x0: f32, x1: f32, weight: f32) {
    let w = acc.len() as f32;
    let x0 = x0.clamp(0.0, w);
    let x1 = x1.clamp(0.0, w);
    if x1 <= x0 {
        return;
    }
    let first = libm::floorf(x0) as usize;
    let last = (libm::floorf(x1) as usize).min(acc.len() - 1);
    for (px, slot) in acc.iter_mut().enumerate().take(last + 1).skip(first) {
        let lo = x0.max(px as f32);
        let hi = x1.min(px as f32 + 1.0);
        if hi > lo {
            *slot += (hi - lo) * weight;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_pixel_square_is_fully_covered() {
        let sq = vec![vec![
            Vec2::new(1.0, 1.0),
            Vec2::new(3.0, 1.0),
            Vec2::new(3.0, 3.0),
            Vec2::new(1.0, 3.0),
        ]];
        let cov = ScanlineRasterizer::default().rasterize(&sq, false, 5, 5);
        assert_eq!(cov.at(2, 2), 255);
        assert_eq!(cov.at(0, 0), 0);
        assert_eq!(cov.at(1, 1), 255);
    }

    #[test]
    fn half_pixel_edge_is_half_covered() {
        let r = vec![vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(0.5, 0.0),
            Vec2::new(0.5, 1.0),
            Vec2::new(0.0, 1.0),
        ]];
        let cov = ScanlineRasterizer::default().rasterize(&r, false, 1, 1);
        assert_eq!(cov.at(0, 0), 128);
    }

    #[test]
    fn even_odd_leaves_hole() {
        let outer = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(4.0, 0.0),
            Vec2::new(4.0, 4.0),
            Vec2::new(0.0, 4.0),
        ];
        let inner = vec![
            Vec2::new(1.0, 1.0),
            Vec2::new(3.0, 1.0),
            Vec2::new(3.0, 3.0),
            Vec2::new(1.0, 3.0),
        ];
        let polys = [outer, inner];
        let rz = ScanlineRasterizer::default();
        let nz = rz.rasterize(&polys, false, 4, 4);
        let eo = rz.rasterize(&polys, true, 4, 4);
        // Same winding direction: nonzero fills the hole, even-odd does not.
        assert_eq!(nz.at(2, 2), 255);
        assert_eq!(eo.at(2, 2), 0);
        assert_eq!(eo.at(0, 0), 255);
    }
}
