//! Raster backend for tpt-canvas.
//!
//! Geometry is transformed to device space, flattened, and rasterized with the
//! `tpt_canvas::raster` scanline rasterizer. Colors are composited premultiplied in f32.
//! Clips are coverage masks intersected on push and restored on pop.
//!
//! Stroking happens in user space (the stroke width scales with the transform),
//! then the outline is transformed to device space.
//!
//! Images are registered with [`RasterBackend::add_image`]. `draw_image` and pattern
//! paints sample them with nearest-neighbour filtering, so they stay sharp and do
//! not need an interpolation choice. Glyph runs are not drawn here (no outlines reach
//! this backend), and calls for unregistered images are skipped. Every skipped call
//! is counted by [`RasterBackend::unsupported`].
#![warn(missing_docs)]

mod blur;
mod decode;
mod deflate;
pub use deflate::zlib_compress;
mod png;
pub use decode::DecodeError;

use tpt_canvas::color::interpolate;
use tpt_canvas::path::Path;
use tpt_canvas::raster::{Coverage, Rasterizer, ScanlineRasterizer};
use tpt_canvas::stroke::stroke_outline;
use tpt_canvas::text::ShapedRun;
use tpt_canvas::{
    Affine2, BlendMode, CanvasBackend, Color, ColorSpace, FillRule, ImageId, ImageRef, Paint, Rect,
    SpreadMode, Stop, StrokeStyle, Vec2,
};

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

    /// Builds a pixmap from straight-alpha RGBA8 bytes (`4 * width * height` of them).
    /// Returns `None` when the length does not match.
    pub fn from_rgba8(width: u32, height: u32, rgba: &[u8]) -> Option<Self> {
        let n = (width as usize) * (height as usize);
        if rgba.len() != n * 4 {
            return None;
        }
        let data = rgba
            .chunks_exact(4)
            .map(|p| {
                let a = f32::from(p[3]) / 255.0;
                [
                    f32::from(p[0]) / 255.0 * a,
                    f32::from(p[1]) / 255.0 * a,
                    f32::from(p[2]) / 255.0 * a,
                    a,
                ]
            })
            .collect();
        Some(Pixmap {
            width,
            height,
            data,
        })
    }

    /// Returns the premultiplied pixel at `(x, y)`.
    pub fn pixel(&self, x: u32, y: u32) -> [f32; 4] {
        self.data[(y as usize) * (self.width as usize) + x as usize]
    }

    /// Decodes a PNG file into a premultiplied pixmap. See [`DecodeError`] for what is supported.
    pub fn from_png(bytes: &[u8]) -> Result<Pixmap, DecodeError> {
        decode::decode_png(bytes)
    }

    /// Encodes the pixmap as a PNG file (8-bit RGBA, zlib-compressed).
    pub fn to_png(&self) -> Vec<u8> {
        png::encode_rgba8(self.width, self.height, &self.to_rgba8())
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
    images: Vec<(ImageId, Pixmap)>,
    unsupported: u32,
    /// Open groups, innermost last, each with its alpha. Drawing goes to the innermost one.
    layers: Vec<(Pixmap, f32)>,
    /// Blend mode for fills and strokes, set by `set_blend_mode`.
    blend: BlendMode,
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
            images: Vec::new(),
            unsupported: 0,
            layers: Vec::new(),
            blend: BlendMode::Normal,
        }
    }

    /// Registers pixels under `id` for `draw_image` and pattern paints. Replaces any
    /// image already registered under that id.
    pub fn add_image(&mut self, id: ImageId, image: Pixmap) {
        match self.images.iter_mut().find(|(k, _)| *k == id) {
            Some(slot) => slot.1 = image,
            None => self.images.push((id, image)),
        }
    }

    /// Number of calls this backend skipped because it cannot draw them (glyph runs,
    /// and images or patterns that were never registered with `add_image`). Zero means
    /// nothing was dropped.
    pub fn unsupported(&self) -> u32 {
        self.unsupported
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

    fn has_image(&self, id: ImageId) -> bool {
        self.images.iter().any(|(k, _)| *k == id)
    }

    /// Whether `paint` can be drawn. Only patterns can be missing something: their image.
    fn paint_available(&self, paint: &Paint) -> bool {
        match paint {
            Paint::Pattern { image, .. } => self.has_image(*image),
            _ => true,
        }
    }

    /// Rasterizes device-space polygons and applies the active clip.
    fn coverage(&mut self, polygons: &[Vec<Vec2>], even_odd: bool) -> Coverage {
        let (w, h) = (self.pixmap.width, self.pixmap.height);
        let mut cov = self.rasterizer.rasterize(polygons, even_odd, w, h);
        if let Some(clip) = self.clips.last() {
            cov.intersect(clip);
        }
        cov
    }

    /// Rasterizes device-space polygons and composites `paint` through them.
    fn paint_polygons(
        &mut self,
        polygons: &[Vec<Vec2>],
        even_odd: bool,
        paint: &Paint,
        user_to_device: Affine2,
    ) {
        let cov = self.coverage(polygons, even_odd);
        let images = &self.images;
        blend(
            target_of(&mut self.layers, &mut self.pixmap),
            &cov,
            user_to_device,
            self.blend,
            |p| sample_paint(paint, p, images),
        );
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
        if !self.paint_available(paint) {
            self.unsupported += 1;
            return;
        }
        let polys = self.device_polylines(path);
        let ctm = self.ctm();
        self.paint_polygons(&polys, rule == FillRule::EvenOdd, paint, ctm);
    }

    fn stroke_path(&mut self, path: &Path, paint: &Paint, style: &StrokeStyle) {
        if !self.paint_available(paint) {
            self.unsupported += 1;
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
        self.paint_polygons(&device, false, paint, ctm);
    }

    fn draw_glyphs(&mut self, _run: &ShapedRun, _paint: &Paint) {
        self.unsupported += 1;
    }

    fn draw_image(&mut self, image: &ImageRef, src_rect: Rect, dst_rect: Rect) {
        if !self.has_image(image.id) {
            self.unsupported += 1;
            return;
        }
        if dst_rect.w == 0.0 || dst_rect.h == 0.0 {
            return;
        }
        let ctm = self.ctm();
        let corners = [
            Vec2::new(dst_rect.x, dst_rect.y),
            Vec2::new(dst_rect.x + dst_rect.w, dst_rect.y),
            Vec2::new(dst_rect.x + dst_rect.w, dst_rect.y + dst_rect.h),
            Vec2::new(dst_rect.x, dst_rect.y + dst_rect.h),
        ];
        let polygon: Vec<Vec2> = corners.iter().map(|c| ctm.apply(*c)).collect();
        let cov = self.coverage(&[polygon], false);
        let Some((_, pixmap)) = self.images.iter().find(|(k, _)| *k == image.id) else {
            return;
        };
        if pixmap.width == 0 || pixmap.height == 0 {
            return;
        }
        blend(
            target_of(&mut self.layers, &mut self.pixmap),
            &cov,
            ctm,
            self.blend,
            |p| {
                // Map the destination rectangle onto the source rectangle, then sample the
                // nearest source pixel, clamped to the image bounds.
                let fx = (p.x - dst_rect.x) / dst_rect.w;
                let fy = (p.y - dst_rect.y) / dst_rect.h;
                let u = src_rect.x + fx * src_rect.w;
                let v = src_rect.y + fy * src_rect.h;
                Some(pixmap.pixel(clamp_index(u, pixmap.width), clamp_index(v, pixmap.height)))
            },
        );
    }

    fn fill_path_blurred(&mut self, path: &Path, paint: &Paint, rule: FillRule, sigma: f32) {
        if !self.paint_available(paint) {
            self.unsupported += 1;
            return;
        }
        if !sigma.is_finite() || sigma <= 0.0 {
            self.fill_path(path, paint, rule);
            return;
        }
        // Blur before clipping, so the shadow is cut off by the clip rather than blurred into it.
        let polys = self.device_polylines(path);
        let (w, h) = (self.pixmap.width, self.pixmap.height);
        let sharp = self
            .rasterizer
            .rasterize(&polys, rule == FillRule::EvenOdd, w, h);
        let mut cov = blur::blur_coverage(&sharp, sigma * self.ctm_scale());
        if let Some(clip) = self.clips.last() {
            cov.intersect(clip);
        }
        let ctm = self.ctm();
        let images = &self.images;
        blend(
            target_of(&mut self.layers, &mut self.pixmap),
            &cov,
            ctm,
            self.blend,
            |p| sample_paint(paint, p, images),
        );
    }

    fn clear_path(&mut self, path: &Path) {
        let polys = self.device_polylines(path);
        let cov = self.coverage(&polys, false);
        // Scale every covered pixel by (1 - coverage): full coverage erases, partial coverage
        // fades the pixel the same way anti-aliased edges fade a fill.
        let (w, h) = (self.pixmap.width, self.pixmap.height);
        let target = target_of(&mut self.layers, &mut self.pixmap);
        for y in 0..h {
            for x in 0..w {
                let c = cov.at(x, y);
                if c == 0 {
                    continue;
                }
                let keep = 1.0 - c as f32 / 255.0;
                let p = &mut target.data[(y as usize) * (w as usize) + x as usize];
                for channel in p.iter_mut() {
                    *channel *= keep;
                }
            }
        }
    }

    fn set_blend_mode(&mut self, mode: BlendMode) {
        self.blend = mode;
    }

    fn push_layer(&mut self, alpha: f32) {
        let (w, h) = (self.pixmap.width, self.pixmap.height);
        self.layers.push((Pixmap::new(w, h), alpha.clamp(0.0, 1.0)));
    }

    fn pop_layer(&mut self) {
        let Some((group, alpha)) = self.layers.pop() else {
            return;
        };
        let dst = target_of(&mut self.layers, &mut self.pixmap);
        composite_group(dst, &group, alpha);
    }

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

/// Source-over composites a premultiplied sample into `pixmap` wherever `cov` is set.
/// `sample` gives the premultiplied colour at a user-space point, or `None` to skip.
fn blend(
    pixmap: &mut Pixmap,
    cov: &Coverage,
    user_to_device: Affine2,
    mode: BlendMode,
    sample: impl Fn(Vec2) -> Option<[f32; 4]>,
) {
    let (w, h) = (pixmap.width, pixmap.height);
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
            let Some(src) = sample(device_to_user.apply(center)) else {
                continue;
            };
            let k = c as f32 / 255.0;
            let s = [src[0] * k, src[1] * k, src[2] * k, src[3] * k];
            let alpha = s[3];
            if alpha <= 0.0 {
                continue;
            }
            let dst = &mut pixmap.data[(y as usize) * (w as usize) + x as usize];
            if mode == BlendMode::Normal {
                for i in 0..4 {
                    dst[i] = s[i] + dst[i] * (1.0 - alpha);
                }
            } else {
                composite_blended(dst, s, mode);
            }
        }
    }
}

/// Premultiplied colour of `paint` at user-space point `p`, or `None` when nothing is drawn there.
fn sample_paint(paint: &Paint, p: Vec2, images: &[(ImageId, Pixmap)]) -> Option<[f32; 4]> {
    match paint {
        Paint::Pattern { .. } => sample_pattern(paint, p, images),
        _ => paint_color(paint, p).map(premultiply),
    }
}

/// Image pattern: `transform` maps image pixels into user space. The image tiles
/// in both directions, so the point is wrapped into the image before sampling.
fn sample_pattern(paint: &Paint, p: Vec2, images: &[(ImageId, Pixmap)]) -> Option<[f32; 4]> {
    let Paint::Pattern { image, transform } = paint else {
        return None;
    };
    let (_, pixmap) = images.iter().find(|(k, _)| k == image)?;
    if pixmap.width == 0 || pixmap.height == 0 {
        return None;
    }
    let inv = transform.invert()?;
    let q = inv.apply(p);
    let x = q.x.rem_euclid(pixmap.width as f32);
    let y = q.y.rem_euclid(pixmap.height as f32);
    Some(pixmap.pixel(clamp_index(x, pixmap.width), clamp_index(y, pixmap.height)))
}

/// Floors `v` to a pixel index in `0..len`.
fn clamp_index(v: f32, len: u32) -> u32 {
    let max = len.saturating_sub(1) as f32;
    libm::floorf(v).clamp(0.0, max) as u32
}

fn premultiply(c: Color) -> [f32; 4] {
    [c.r * c.a, c.g * c.a, c.b * c.a, c.a]
}

/// Color of a solid or gradient `paint` at user-space point `p`.
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

/// The pixmap that drawing goes to: the innermost open group, or the base canvas.
fn target_of<'a>(layers: &'a mut [(Pixmap, f32)], base: &'a mut Pixmap) -> &'a mut Pixmap {
    match layers.last_mut() {
        Some((group, _)) => group,
        None => base,
    }
}

/// Source-over composites a finished group onto `dst` with the group's alpha applied once.
fn composite_group(dst: &mut Pixmap, group: &Pixmap, alpha: f32) {
    for (d, g) in dst.data.iter_mut().zip(&group.data) {
        let s = [g[0] * alpha, g[1] * alpha, g[2] * alpha, g[3] * alpha];
        let keep = 1.0 - s[3];
        for (dc, sc) in d.iter_mut().zip(s) {
            *dc = sc + *dc * keep;
        }
    }
}

/// Composites premultiplied `src` onto `dst` with a separable blend mode (W3C compositing):
/// `Cr = (1 - ab) * Cs + (1 - as) * Cb + as * ab * B(cb, cs)` on premultiplied colors.
fn composite_blended(dst: &mut [f32; 4], src: [f32; 4], mode: BlendMode) {
    let (as_, ab) = (src[3], dst[3]);
    let backdrop = *dst;
    // Unpremultiply both colours. A transparent backdrop has no colour to blend with.
    let unpremul = |p: [f32; 4], a: f32| -> [f32; 3] {
        if a > 0.0 {
            [p[0] / a, p[1] / a, p[2] / a]
        } else {
            [0.0; 3]
        }
    };
    let cb = unpremul(backdrop, ab);
    let cs = unpremul(src, as_);
    let b = mode.blend_rgb(cb, cs);
    for c in 0..3 {
        dst[c] = (1.0 - ab) * src[c] + (1.0 - as_) * backdrop[c] + as_ * ab * b[c];
    }
    dst[3] = as_ + ab * (1.0 - as_);
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

    #[test]
    fn draw_image_copies_pixels_into_the_destination() {
        // 2x1 image: red then blue, both opaque.
        let img = Pixmap::from_rgba8(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255]).unwrap();
        let mut rb = RasterBackend::new(4, 1);
        rb.add_image(ImageId(1), img);
        let image = ImageRef {
            id: ImageId(1),
            width: 2,
            height: 1,
        };
        rb.draw_image(
            &image,
            Rect::new(0.0, 0.0, 2.0, 1.0),
            Rect::new(0.0, 0.0, 4.0, 1.0),
        );
        assert_eq!(rb.pixmap().pixel(0, 0), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(rb.pixmap().pixel(1, 0), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(rb.pixmap().pixel(2, 0), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(rb.pixmap().pixel(3, 0), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(rb.unsupported(), 0);
    }

    #[test]
    fn unregistered_image_is_counted_and_skipped() {
        let mut rb = RasterBackend::new(4, 4);
        let image = ImageRef {
            id: ImageId(9),
            width: 1,
            height: 1,
        };
        rb.draw_image(
            &image,
            Rect::new(0.0, 0.0, 1.0, 1.0),
            Rect::new(0.0, 0.0, 4.0, 4.0),
        );
        assert_eq!(rb.unsupported(), 1);
        assert_eq!(rb.pixmap().pixel(0, 0), [0.0; 4]);
    }

    #[test]
    fn pattern_tiles_the_image_across_the_fill() {
        // 2x2 checker of white and transparent pixels, tiled over a 4x4 fill.
        let px = [
            255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255,
        ];
        let img = Pixmap::from_rgba8(2, 2, &px).unwrap();
        let mut rb = RasterBackend::new(4, 4);
        rb.add_image(ImageId(2), img);
        let mut b = PathBuilder::new();
        b.rect(0.0, 0.0, 4.0, 4.0);
        let paint = Paint::Pattern {
            image: ImageId(2),
            transform: Affine2::IDENTITY,
        };
        rb.fill_path(&b.build(), &paint, FillRule::NonZero);
        assert_eq!(rb.pixmap().pixel(0, 0)[3], 1.0);
        assert_eq!(rb.pixmap().pixel(1, 0)[3], 0.0);
        // The tile repeats: (2, 0) matches (0, 0).
        assert_eq!(rb.pixmap().pixel(2, 0)[3], 1.0);
        assert_eq!(rb.unsupported(), 0);
    }

    #[test]
    fn from_rgba8_rejects_wrong_length() {
        assert!(Pixmap::from_rgba8(2, 2, &[0; 15]).is_none());
        assert!(Pixmap::from_rgba8(2, 2, &[0; 16]).is_some());
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
