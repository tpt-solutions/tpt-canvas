//! CPU executor for [`DrawList`]. It follows the same stencil-then-cover
//! steps as a GPU, so tests can check the algorithm without hardware.
//!
//! Coverage is point-sampled at pixel centers, with no anti-aliasing. Triangle
//! inclusion uses the top-left tie rule, so a point on an edge shared by two fan
//! triangles is counted once.

use tpt_canvas::color::interpolate;
use tpt_canvas::{
    Affine2, BlendMode, Color, ColorSpace, FillRule, Paint, Rect, SpreadMode, Stop, Vec2,
};

use crate::{Command, DrawList, TriRange};

/// Premultiplied RGBA, row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Premultiplied RGBA, row by row.
    pub data: Vec<[f32; 4]>,
}

/// Runs every command in `list` and returns the composed frame.
pub fn execute(list: &DrawList) -> Image {
    let (w, h) = (list.width as usize, list.height as usize);
    // The canvas is the bottom of the stack. Each open group is a transparent image above it.
    let mut stack = vec![Image {
        width: list.width,
        height: list.height,
        data: vec![[0.0; 4]; w * h],
    }];
    let mut layers: Vec<Vec<bool>> = vec![Vec::new(); list.clip_layers as usize];

    for cmd in &list.commands {
        match *cmd {
            Command::BeginGroup => stack.push(Image {
                width: list.width,
                height: list.height,
                data: vec![[0.0; 4]; w * h],
            }),
            Command::EndGroup { alpha } => {
                let group = stack.pop().expect("EndGroup follows BeginGroup");
                composite_group(
                    stack.last_mut().expect("the canvas is always there"),
                    &group,
                    alpha,
                );
            }
            Command::ClipMask {
                layer,
                parent,
                tris,
                rule,
                bounds,
            } => {
                let mut mask = vec![false; w * h];
                for_each_pixel(list, bounds, |idx, p| {
                    let inside_parent = parent.map_or(true, |pl| layers[pl as usize][idx]);
                    mask[idx] = inside_parent && covered(list, tris, rule, p);
                });
                layers[layer as usize] = mask;
            }
            Command::Fill {
                tris,
                rule,
                paint,
                clip,
                bounds,
                blend,
            } => {
                let image = stack.last_mut().expect("the canvas is always there");
                let entry = &list.paints[paint as usize];
                for_each_pixel(list, bounds, |idx, p| {
                    if let Some(cl) = clip {
                        if !layers[cl as usize][idx] {
                            return;
                        }
                    }
                    if !covered(list, tris, rule, p) {
                        return;
                    }
                    let user = entry.device_to_user.apply(p);
                    let src = match &entry.paint {
                        Paint::Pattern { image, transform } => {
                            pattern_color(list, *image, *transform, user, entry.clip_image)
                        }
                        other => paint_color(other, user),
                    };
                    let Some(src) = src else {
                        return;
                    };
                    let dst = &mut image.data[idx];
                    if blend == BlendMode::Normal {
                        let a = src.a;
                        for (i, s) in [src.r, src.g, src.b].into_iter().enumerate() {
                            dst[i] = s * a + dst[i] * (1.0 - a);
                        }
                        dst[3] = a + dst[3] * (1.0 - a);
                    } else {
                        composite_blended(dst, [src.r, src.g, src.b], src.a, blend);
                    }
                });
            }
        }
    }
    while stack.len() > 1 {
        let group = stack.pop().expect("checked above");
        composite_group(
            stack.last_mut().expect("the canvas is always there"),
            &group,
            1.0,
        );
    }
    stack.pop().expect("the canvas is always there")
}

/// Calls `f(pixel index, pixel center)` for every pixel center inside `bounds` on the canvas.
fn for_each_pixel(list: &DrawList, bounds: tpt_canvas::Rect, mut f: impl FnMut(usize, Vec2)) {
    let w = list.width as usize;
    let x0 = libm::floorf(bounds.x).max(0.0) as usize;
    let y0 = libm::floorf(bounds.y).max(0.0) as usize;
    let x1 = (libm::ceilf(bounds.x + bounds.w) as usize).min(list.width as usize);
    let y1 = (libm::ceilf(bounds.y + bounds.h) as usize).min(list.height as usize);
    for y in y0..y1 {
        for x in x0..x1 {
            f(y * w + x, Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
        }
    }
}

/// Winding-number test for the triangles in `tris`, reduced by the fill rule.
fn covered(list: &DrawList, tris: TriRange, rule: FillRule, p: Vec2) -> bool {
    let start = tris.first as usize;
    let end = start + tris.count as usize;
    let mut winding = 0i32;
    for t in list.indices[start..end].chunks_exact(3) {
        winding += triangle_contribution(
            list.positions[t[0] as usize],
            list.positions[t[1] as usize],
            list.positions[t[2] as usize],
            p,
        );
    }
    match rule {
        FillRule::NonZero => winding != 0,
        FillRule::EvenOdd => winding & 1 != 0,
    }
}

/// `+1` or `-1` when `p` is inside triangle `(a, b, c)` (by its orientation), else `0`.
fn triangle_contribution(a: Vec2, b: Vec2, c: Vec2, p: Vec2) -> i32 {
    let s = (b - a).cross(c - a);
    if s.abs() <= 1e-9 {
        return 0;
    }
    // Normalise to counter-clockwise so every edge test has the same sign convention.
    let (p0, p1, p2) = if s > 0.0 { (a, b, c) } else { (a, c, b) };
    let inside = edge_ok(p0, p1, p) && edge_ok(p1, p2, p) && edge_ok(p2, p0, p);
    match (inside, s > 0.0) {
        (false, _) => 0,
        (true, true) => 1,
        (true, false) => -1,
    }
}

/// Edge test with the top-left rule: exact ties go to exactly one of two shared edges.
fn edge_ok(u: Vec2, v: Vec2, p: Vec2) -> bool {
    let e = (v - u).cross(p - u);
    if e != 0.0 {
        return e > 0.0;
    }
    let d = v - u;
    d.y < 0.0 || (d.y == 0.0 && d.x > 0.0)
}

/// Colour of `paint` at user-space point `p`, or `None` when it draws nothing there.
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

/// Composites a straight-alpha source over premultiplied `dst` with a blend mode, by the W3C
/// formula: `Cr = (1 - ab) * Cs + (1 - as) * Cb + as * ab * B(cb, cs)`, on premultiplied colours.
fn composite_blended(dst: &mut [f32; 4], src: [f32; 3], alpha: f32, mode: BlendMode) {
    let (as_, ab) = (alpha, dst[3]);
    let unpremul = |c: f32| if ab > 0.0 { c / ab } else { 0.0 };
    let cb = [unpremul(dst[0]), unpremul(dst[1]), unpremul(dst[2])];
    let b = mode.blend_rgb(cb, src);
    for c in 0..3 {
        let cs_premult = src[c] * as_;
        dst[c] = (1.0 - ab) * cs_premult + (1.0 - as_) * dst[c] + as_ * ab * b[c];
    }
    dst[3] = as_ + ab * (1.0 - as_);
}

/// The straight colour of a pattern at user-space point `p`: the image repeats in its own pixel
/// space, which `transform` maps into user space. Nearest-neighbour, as the raster backend does.
/// `None` when the image is not in the list.
fn pattern_color(
    list: &DrawList,
    id: tpt_canvas::ImageId,
    transform: Affine2,
    p: Vec2,
    crop: Option<Rect>,
) -> Option<Color> {
    let image = list.images.iter().find(|i| i.id == id)?;
    if image.width == 0 || image.height == 0 {
        return None;
    }
    let q = transform.invert()?.apply(p);
    let (w, h) = (image.width as f32, image.height as f32);
    let (x, y) = match crop {
        // A drawn image: outside its source rectangle nothing is drawn, and it is not repeated.
        Some(r) => {
            if q.x < r.x || q.x >= r.x + r.w || q.y < r.y || q.y >= r.y + r.h {
                return Some(Color::TRANSPARENT);
            }
            (
                libm::floorf(q.x).clamp(0.0, w - 1.0) as usize,
                libm::floorf(q.y).clamp(0.0, h - 1.0) as usize,
            )
        }
        None => (
            libm::floorf(q.x.rem_euclid(w)).clamp(0.0, w - 1.0) as usize,
            libm::floorf(q.y.rem_euclid(h)).clamp(0.0, h - 1.0) as usize,
        ),
    };
    let [r, g, b, a] = image.pixels[y * image.width as usize + x];
    if a <= 0.0 {
        return Some(Color::TRANSPARENT);
    }
    Some(Color::rgba(r / a, g / a, b / a, a))
}

/// Composites a group onto `dst` with `alpha`, on premultiplied colours (source-over):
/// `dst = s * alpha + dst * (1 - s.a * alpha)`. The same as the raster backend's group.
fn composite_group(dst: &mut Image, group: &Image, alpha: f32) {
    for (d, g) in dst.data.iter_mut().zip(&group.data) {
        let s = [g[0] * alpha, g[1] * alpha, g[2] * alpha, g[3] * alpha];
        let keep = 1.0 - s[3];
        for (dc, sc) in d.iter_mut().zip(s) {
            *dc = sc + *dc * keep;
        }
    }
}
