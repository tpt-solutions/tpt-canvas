//! Stub for `tpt-color`: colors, gradient stop resolution and alpha compositing.
//!
//! Colors are sRGB-encoded, straight (non-premultiplied) alpha, components in `0..=1`.
// Stub crate: replaced by the real ecosystem crate, so its items are not documented individually.

/// A color with sRGB-encoded channels and straight alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Color = Color::rgba(0.0, 0.0, 0.0, 1.0);
    pub const WHITE: Color = Color::rgba(1.0, 1.0, 1.0, 1.0);

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Color::rgba(r, g, b, 1.0)
    }

    pub const fn with_alpha(self, a: f32) -> Self {
        Color { a, ..self }
    }
}

/// Space in which gradient stops are interpolated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorSpace {
    /// Interpolate the sRGB-encoded values directly.
    #[default]
    Srgb,
    /// Decode to linear light, interpolate, re-encode.
    LinearSrgb,
}

/// A gradient stop: `offset` in `0..=1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop {
    pub offset: f32,
    pub color: Color,
}

pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        libm::powf((c + 0.055) / 1.055, 2.4)
    }
}

pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * libm::powf(c, 1.0 / 2.4) - 0.055
    }
}

fn lerp_color(a: Color, b: Color, t: f32, space: ColorSpace) -> Color {
    let mix = |x: f32, y: f32| x + (y - x) * t;
    match space {
        ColorSpace::Srgb => Color::rgba(mix(a.r, b.r), mix(a.g, b.g), mix(a.b, b.b), mix(a.a, b.a)),
        ColorSpace::LinearSrgb => {
            let (la, lb) = (to_linear(a), to_linear(b));
            let m = Color::rgba(
                mix(la.r, lb.r),
                mix(la.g, lb.g),
                mix(la.b, lb.b),
                mix(a.a, b.a),
            );
            Color::rgba(
                linear_to_srgb(m.r),
                linear_to_srgb(m.g),
                linear_to_srgb(m.b),
                m.a,
            )
        }
    }
}

fn to_linear(c: Color) -> Color {
    Color::rgba(
        srgb_to_linear(c.r),
        srgb_to_linear(c.g),
        srgb_to_linear(c.b),
        c.a,
    )
}

/// Resolves the color at `t` (already spread-adjusted into `0..=1`) from stops
/// sorted by ascending `offset`. Empty stops yield transparent black.
pub fn interpolate(stops: &[Stop], t: f32, space: ColorSpace) -> Color {
    let (first, last) = match (stops.first(), stops.last()) {
        (Some(f), Some(l)) => (f, l),
        _ => return Color::TRANSPARENT,
    };
    if t <= first.offset {
        return first.color;
    }
    if t >= last.offset {
        return last.color;
    }
    for pair in stops.windows(2) {
        let (s0, s1) = (&pair[0], &pair[1]);
        if t >= s0.offset && t <= s1.offset {
            let span = s1.offset - s0.offset;
            if span <= 0.0 {
                return s1.color;
            }
            return lerp_color(s0.color, s1.color, (t - s0.offset) / span, space);
        }
    }
    last.color
}

/// Source-over compositing of straight-alpha colors.
pub fn over(src: Color, dst: Color) -> Color {
    let out_a = src.a + dst.a * (1.0 - src.a);
    if out_a <= 0.0 {
        return Color::TRANSPARENT;
    }
    let mix = |s: f32, d: f32| (s * src.a + d * dst.a * (1.0 - src.a)) / out_a;
    Color::rgba(
        mix(src.r, dst.r),
        mix(src.g, dst.g),
        mix(src.b, dst.b),
        out_a,
    )
}
