//! Raster backend for tpt-canvas.
//!
//! Geometry is transformed to device space, flattened, and rasterized with the
//! `tpt-raster-stub` scanline rasterizer. Colors are composited premultiplied in f32.
//! Clips are coverage masks intersected on push and restored on pop.
//!
//! Stroking happens in user space (the stroke width scales with the transform),
//! then the outline is transformed to device space.
//!
//! Not yet drawn: glyphs and images (no outlines or pixel data reach this backend
//! yet), and pattern paints. Those calls are accepted and ignored.
#![warn(missing_docs)]

use tpt_canvas::path::Path;
use tpt_canvas::stroke::stroke_outline;
use tpt_canvas::{
    Affine2, CanvasBackend, Color, ColorSpace, FillRule, ImageRef, Paint, Rect, SpreadMode, Stop,
    StrokeStyle, Vec2,
};
use tpt_color_stub::interpolate;
use tpt_raster_stub::{Coverage, Rasterizer, ScanlineRasterizer};
use tpt_text_stub::ShapedRun;

/// Premultiplied RGBA, f32 per channel, row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct Pixmap {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Premultiplied RGBA, row by row.
    pub data: Vec<[f32; 4]>,
}

impl Pixmap {
    /// Creates a transparent pixmap.
    pub fn new(width: u32, height: u32) -> Self {
        Pixmap {
            width,
            height,
            data: vec![[0.0; 4]; (width as usize) * (height as usize)],
        }
    }

    /// Returns the premultiplied pixel at `(x, y)`.
    pub fn pixel(&self, x: u32, y: u32) -> [f32; 4] {
        self.data[(y as usize) * (self.width as usize) + x as usize]
    }

    /// Converts to straight-alpha 8-bit RGBA.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.data.len() * 4);
        for p in &self.data {
            let a = p[3];
            let un = |c: f32| if a > 0.0 { c / a } else { 0.0 };
            for c in [un(p[0]), un(p[1]), un(p[2]), a] {
                out.push((c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
        }
        out
    }
}

/// A `CanvasBackend` that rasterizes into a `Pixmap`.
pub struct RasterBackend {
    pixmap: Pixmap,
    /// Cumulative transform; the top of the stack is the current transform.
    transforms: Vec<Affine2>,
    /// Clip masks; the top is the intersection of all active clips.
    clips: Vec<Coverage>,
    rasterizer: ScanlineRasterizer,
    tolerance: f32,
}

impl RasterBackend {
    /// Creates a backend drawing into a transparent canvas.
    pub fn new(width: u32, height: u32) -> Self {
        RasterBackend {
            pixmap: Pixmap::new(width, height),
            transforms: vec![Affine2::IDENTITY],
            clips: Vec::new(),
            rasterizer: ScanlineRasterizer::default(),
            tolerance: 0.1,
        }
    }

    /// Returns the pixmap drawn so far.
    pub fn pixmap(&self) -> &Pixmap {
        &self.pixmap
    }

    /// Consumes the backend and returns its pixmap.
    pub fn into_pixmap(self) -> Pixmap {
        self.pixmap
    }

    fn ctm(&self) -> Affine2 {
        *self
            .transforms
            .last()
            .expect("base transform is always present")
    }

    /// Rasterizes device-space polygons, applies the active clip, and composites `paint`.
    fn composite(
        &mut self,
        polygons: &[Vec<Vec2>],
        even_odd: bool,
        paint: &Paint,
        user_to_device: Affine2,
    ) {
        let (w, h) = (self.pixmap.width, self.pixmap.height);
        let mut cov = self.rasterizer.rasterize(polygons, even_odd, w, h);
        if let Some(clip) = self.clips.last() {
            cov.intersect(clip);
        }
        let Some(device_to_user) = user_to_device.invert() else {
            return;
        };
        for y in 0..h {
            for x in 0..w {
                let c = cov.at(x, y);
                if c == 0 {
                    continue;
                }
                let center = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let Some(src) = paint_color(paint, device_to_user.apply(center)) else {
                    continue;
                };
                let alpha = src.a * (c as f32 / 255.0);
                if alpha <= 0.0 {
                    continue;
                }
                let dst = &mut self.pixmap.data[(y as usize) * (w as usize) + x as usize];
                let s = [src.r * alpha, src.g * alpha, src.b * alpha, alpha];
                for i in 0..4 {
                    dst[i] = s[i] + dst[i] * (1.0 - alpha);
                }
            }
        }
    }

    fn device_polylines(&self, path: &Path) -> Vec<Vec<Vec2>> {
        path.transform(&self.ctm())
            .flatten(self.tolerance)
            .polylines
            .into_iter()
            .map(|p| p.points)
            .collect()
    }

    /// Scale factor of the current transform, used to convert device tolerance to user space.
    fn ctm_scale(&self) -> f32 {
        let m = self.ctm();
        libm::sqrtf(libm::fabsf(m.determinant())).max(1e-6)
    }
}

impl CanvasBackend for RasterBackend {
    fn fill_path(&mut self, path: &Path, paint: &Paint, rule: FillRule) {
        if matches!(paint, Paint::Pattern { .. }) {
            return;
        }
        let polys = self.device_polylines(path);
        let ctm = self.ctm();
        self.composite(&polys, rule == FillRule::EvenOdd, paint, ctm);
    }

    fn stroke_path(&mut self, path: &Path, paint: &Paint, style: &StrokeStyle) {
        if matches!(paint, Paint::Pattern { .. }) {
            return;
        }
        // Stroke in user space so the width follows the transform, then map the outline.
        let user_tol = self.tolerance / self.ctm_scale();
        let flat = path.flatten(user_tol);
        let ctm = self.ctm();
        let device: Vec<Vec<Vec2>> = stroke_outline(&flat, style, user_tol)
            .into_iter()
            .map(|poly| poly.into_iter().map(|p| ctm.apply(p)).collect())
            .collect();
        self.composite(&device, false, paint, ctm);
    }

    fn draw_glyphs(&mut self, _run: &ShapedRun, _paint: &Paint) {}

    fn draw_image(&mut self, _image: &ImageRef, _src_rect: Rect, _dst_rect: Rect) {}

    fn push_clip(&mut self, path: &Path) {
        let polys = self.device_polylines(path);
        let (w, h) = (self.pixmap.width, self.pixmap.height);
        let mut mask = self.rasterizer.rasterize(&polys, false, w, h);
        if let Some(outer) = self.clips.last() {
            mask.intersect(outer);
        }
        self.clips.push(mask);
    }

    fn pop_clip(&mut self) {
        self.clips.pop();
    }

    fn push_transform(&mut self, transform: &Affine2) {
        let next = self.ctm() * *transform;
        self.transforms.push(next);
    }

    fn pop_transform(&mut self) {
        if self.transforms.len() > 1 {
            self.transforms.pop();
        }
    }
}

/// Color of `paint` at user-space point `p`, or `None` when the paint draws nothing here.
fn paint_color(paint: &Paint, p: Vec2) -> Option<Color> {
    match paint {
        Paint::Solid(c) => Some(*c),
        Paint::LinearGradient {
            start,
            end,
            stops,
            spread,
            space,
        } => {
            let d = *end - *start;
            let len2 = d.dot(d);
            let t = if len2 > 0.0 {
                (p - *start).dot(d) / len2
            } else {
                0.0
            };
            Some(gradient_at(stops, t, *spread, *space))
        }
        Paint::RadialGradient {
            center,
            radius,
            stops,
            spread,
            space,
        } => {
            let t = if *radius > 0.0 {
                p.distance(*center) / radius
            } else {
                0.0
            };
            Some(gradient_at(stops, t, *spread, *space))
        }
        Paint::Pattern { .. } => None,
    }
}

fn gradient_at(stops: &[Stop], t: f32, spread: SpreadMode, space: ColorSpace) -> Color {
    let t = match spread {
        SpreadMode::Pad => t.clamp(0.0, 1.0),
        SpreadMode::Repeat => t - libm::floorf(t),
        SpreadMode::Reflect => {
            let m = t - 2.0 * libm::floorf(t * 0.5);
            if m > 1.0 {
                2.0 - m
            } else {
                m
            }
        }
    };
    interpolate(stops, t, space)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_canvas::PathBuilder;

    #[test]
    fn filled_rect_is_solid_inside_and_empty_outside() {
        let mut b = PathBuilder::new();
        b.rect(2.0, 2.0, 4.0, 4.0);
        let path = b.build();
        let mut rb = RasterBackend::new(8, 8);
        rb.fill_path(
            &path,
            &Paint::Solid(Color::rgb(1.0, 0.0, 0.0)),
            FillRule::NonZero,
        );
        assert_eq!(rb.pixmap().pixel(3, 3), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(rb.pixmap().pixel(0, 0), [0.0; 4]);
    }

    #[test]
    fn circle_area_is_close_to_pi_r_squared() {
        let mut b = PathBuilder::new();
        b.circle(50.0, 50.0, 30.0);
        let mut rb = RasterBackend::new(100, 100);
        rb.fill_path(&b.build(), &Paint::Solid(Color::BLACK), FillRule::NonZero);
        let covered: f32 = rb.pixmap().data.iter().map(|p| p[3]).sum();
        let expected = core::f32::consts::PI * 30.0 * 30.0;
        assert!(
            (covered - expected).abs() / expected < 0.01,
            "area {covered} vs {expected}"
        );
    }

    #[test]
    fn clip_limits_fill() {
        let mut clip = PathBuilder::new();
        clip.rect(0.0, 0.0, 4.0, 8.0);
        let mut fill = PathBuilder::new();
        fill.rect(0.0, 0.0, 8.0, 8.0);
        let mut rb = RasterBackend::new(8, 8);
        rb.push_clip(&clip.build());
        rb.fill_path(
            &fill.build(),
            &Paint::Solid(Color::WHITE),
            FillRule::NonZero,
        );
        rb.pop_clip();
        assert_eq!(rb.pixmap().pixel(1, 1)[3], 1.0);
        assert_eq!(rb.pixmap().pixel(6, 1)[3], 0.0);
    }

    #[test]
    fn transform_moves_geometry() {
        let mut b = PathBuilder::new();
        b.rect(0.0, 0.0, 2.0, 2.0);
        let mut rb = RasterBackend::new(10, 10);
        rb.push_transform(&Affine2::translate(5.0, 5.0));
        rb.fill_path(&b.build(), &Paint::Solid(Color::WHITE), FillRule::NonZero);
        rb.pop_transform();
        assert_eq!(rb.pixmap().pixel(6, 6)[3], 1.0);
        assert_eq!(rb.pixmap().pixel(1, 1)[3], 0.0);
    }

    #[test]
    fn stroke_width_follows_transform() {
        let mut b = PathBuilder::new();
        b.move_to(0.0, 2.0);
        b.line_to(4.0, 2.0);
        let style = StrokeStyle {
            width: 1.0,
            ..StrokeStyle::default()
        };
        let mut rb = RasterBackend::new(40, 40);
        rb.push_transform(&Affine2::scale(4.0, 4.0));
        rb.stroke_path(&b.build(), &Paint::Solid(Color::BLACK), &style);
        rb.pop_transform();
        // Width 1 at scale 4 is 4 device pixels tall.
        let covered: f32 = rb.pixmap().data.iter().map(|p| p[3]).sum();
        assert!((covered - 16.0 * 4.0).abs() < 0.5, "covered {covered}");
    }

    #[test]
    fn linear_gradient_runs_start_to_end() {
        let mut b = PathBuilder::new();
        b.rect(0.0, 0.0, 10.0, 1.0);
        let paint = Paint::LinearGradient {
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(10.0, 0.0),
            stops: alloc_stops(),
            spread: SpreadMode::Pad,
            space: ColorSpace::Srgb,
        };
        let mut rb = RasterBackend::new(10, 1);
        rb.fill_path(&b.build(), &paint, FillRule::NonZero);
        let left = rb.pixmap().pixel(0, 0);
        let right = rb.pixmap().pixel(9, 0);
        assert!(
            left[0] < 0.2 && right[0] > 0.8,
            "left {left:?} right {right:?}"
        );
    }

    fn alloc_stops() -> Vec<Stop> {
        vec![
            Stop {
                offset: 0.0,
                color: Color::BLACK,
            },
            Stop {
                offset: 1.0,
                color: Color::WHITE,
            },
        ]
    }
}
