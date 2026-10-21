//! Paints, fill rules and stroke styles (Module B).

use alloc::vec::Vec;

use crate::image::ImageId;
use crate::{Affine2, Color, ColorSpace, Stop, Vec2};

/// Winding rule used when filling a path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FillRule {
    #[default]
    /// Non-zero winding: a point is inside when its winding number is not zero.
    NonZero,
    /// Even-odd: a point is inside when its winding number is odd.
    EvenOdd,
}

/// What happens outside a gradient's `0..=1` range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpreadMode {
    /// Extend the first and last stop colors.
    #[default]
    Pad,
    /// Tile the gradient.
    Repeat,
    /// Tile the gradient, mirroring every other tile.
    Reflect,
}

/// Source of color for a fill or stroke.
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    /// A single colour.
    Solid(Color),
    /// Gradient from `start` (t = 0) to `end` (t = 1), in user space.
    LinearGradient {
        /// The first point of the gradient axis, in user space.
        start: Vec2,
        /// The second point of the gradient axis, in user space.
        end: Vec2,
        /// Colour stops, sorted by offset.
        stops: Vec<Stop>,
        /// Behaviour outside the `0..=1` range.
        spread: SpreadMode,
        /// Colour space used for interpolation.
        space: ColorSpace,
    },
    /// Circle gradient: t = distance from `center` / `radius`, in user space.
    RadialGradient {
        /// Centre of the circle, in user space.
        center: Vec2,
        /// Radius of the circle, in user space.
        radius: f32,
        /// Colour stops, sorted by offset.
        stops: Vec<Stop>,
        /// Behaviour outside the `0..=1` range.
        spread: SpreadMode,
        /// Colour space used for interpolation.
        space: ColorSpace,
    },
    /// Tiled image; `transform` maps image space into user space.
    Pattern {
        /// The image to tile.
        image: ImageId,
        /// Maps image space into user space.
        transform: Affine2,
    },
}

impl Paint {
    /// Convenience constructor for `Paint::Solid`.
    pub fn solid(color: Color) -> Self {
        Paint::Solid(color)
    }

    /// Linear gradient from `start` to `end` with `(offset, color)` stops.
    ///
    /// Pads outside the axis and interpolates in sRGB. Stops are sorted by offset here, so the caller can list them in any order.
    pub fn linear(start: Vec2, end: Vec2, stops: &[(f32, Color)]) -> Self {
        Paint::LinearGradient {
            start,
            end,
            stops: sorted_stops(stops),
            spread: SpreadMode::Pad,
            space: ColorSpace::Srgb,
        }
    }

    /// A copy of this paint with every colour's alpha multiplied by `alpha`.
    ///
    /// Pattern paints are returned unchanged, because they carry no colour of their own.
    pub fn with_opacity(&self, alpha: f32) -> Self {
        let scale = |c: Color| c.with_alpha(c.a * alpha);
        let mut out = self.clone();
        match &mut out {
            Paint::Solid(c) => *c = scale(*c),
            Paint::LinearGradient { stops, .. } | Paint::RadialGradient { stops, .. } => {
                for stop in stops.iter_mut() {
                    stop.color = scale(stop.color);
                }
            }
            Paint::Pattern { .. } => {}
        }
        out
    }

    /// Radial gradient around `center` out to `radius`, with `(offset, color)` stops.
    ///
    /// Pads outside the radius and interpolates in sRGB. Stops are sorted by offset here.
    pub fn radial(center: Vec2, radius: f32, stops: &[(f32, Color)]) -> Self {
        Paint::RadialGradient {
            center,
            radius,
            stops: sorted_stops(stops),
            spread: SpreadMode::Pad,
            space: ColorSpace::Srgb,
        }
    }
}

fn sorted_stops(stops: &[(f32, Color)]) -> Vec<Stop> {
    let mut out: Vec<Stop> = stops
        .iter()
        .map(|&(offset, color)| Stop { offset, color })
        .collect();
    out.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    out
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// How open stroke ends are drawn.
pub enum LineCap {
    #[default]
    /// The stroke ends at the endpoint.
    Butt,
    /// A semicircle past the endpoint.
    Round,
    /// A square extending half the width past the endpoint.
    Square,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// How corners between segments are drawn.
pub enum LineJoin {
    #[default]
    /// Extends the outer edges until they meet, up to the miter limit.
    Miter,
    /// A circular arc around the corner.
    Round,
    /// Connects the outer corners with a straight line.
    Bevel,
}

/// Stroke geometry parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct StrokeStyle {
    /// Stroke width in user units.
    pub width: f32,
    /// End-cap style.
    pub cap: LineCap,
    /// Corner style.
    pub join: LineJoin,
    /// Maximum miter length relative to the stroke width.
    pub miter_limit: f32,
    /// Alternating on/off lengths. Empty means solid. Odd-length arrays are repeated.
    pub dash: Vec<f32>,
    /// Distance into the dash pattern at which the stroke starts.
    pub dash_offset: f32,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        StrokeStyle {
            width: 1.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 10.0,
            dash: Vec::new(),
            dash_offset: 0.0,
        }
    }
}

/// A drop shadow drawn beneath each fill and stroke, set with `Context::set_shadow`.
///
/// The fields follow the HTML canvas shadow properties: `offset` moves the shadow in user
/// space, and `blur` is the blur size, where the Gaussian has a standard deviation of
/// `blur / 2`. The shadow's alpha is multiplied by the global alpha. Blurred stroke shadows are
/// blurred through their outline, which is flattened at a fixed tolerance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    /// Shadow color, including its alpha.
    pub color: Color,
    /// Offset in user units.
    pub offset: Vec2,
    /// Blur size in user units. Zero draws a sharp shadow.
    pub blur: f32,
}

/// How a drawn fill or stroke combines with what is already on the canvas.
///
/// These are the W3C compositing blend modes, with the same names as CSS `mix-blend-mode`
/// and the canvas `globalCompositeOperation` values. `Normal` is source-over. The first
/// eight are separable, meaning each colour channel is blended on its own. The last four
/// (hue, saturation, color, luminosity) mix whole colours through luminosity and saturation.
/// Blend modes apply to fills and strokes. Group layers always composite normally.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlendMode {
    /// Source-over: the drawing covers what is below, by its alpha.
    #[default]
    Normal,
    /// Darkens: the product of the two colors.
    Multiply,
    /// Lightens: the inverse of the product of the inverses.
    Screen,
    /// Multiply or screen, chosen by the backdrop.
    Overlay,
    /// The darker of the two colors per channel.
    Darken,
    /// The lighter of the two colors per channel.
    Lighten,
    /// The absolute difference of the two colors.
    Difference,
    /// Like difference, with lower contrast.
    Exclusion,
    /// The hue of the source with the saturation and luminosity of the backdrop.
    Hue,
    /// The saturation of the source with the hue and luminosity of the backdrop.
    Saturation,
    /// The hue and saturation of the source with the luminosity of the backdrop.
    Color,
    /// The luminosity of the source with the hue and saturation of the backdrop.
    Luminosity,
}

impl BlendMode {
    /// The CSS / canvas name of this mode, as used by `mix-blend-mode` and `globalCompositeOperation`.
    pub fn css_name(self) -> &'static str {
        match self {
            BlendMode::Normal => "normal",
            BlendMode::Multiply => "multiply",
            BlendMode::Screen => "screen",
            BlendMode::Overlay => "overlay",
            BlendMode::Darken => "darken",
            BlendMode::Lighten => "lighten",
            BlendMode::Difference => "difference",
            BlendMode::Exclusion => "exclusion",
            BlendMode::Hue => "hue",
            BlendMode::Saturation => "saturation",
            BlendMode::Color => "color",
            BlendMode::Luminosity => "luminosity",
        }
    }

    /// The blend function `B(backdrop, source)` on unpremultiplied RGB colors, from the W3C spec.
    pub fn blend_rgb(self, backdrop: [f32; 3], source: [f32; 3]) -> [f32; 3] {
        match self {
            BlendMode::Normal => source,
            BlendMode::Hue => set_lum(set_sat(source, sat(backdrop)), lum(backdrop)),
            BlendMode::Saturation => set_lum(set_sat(backdrop, sat(source)), lum(backdrop)),
            BlendMode::Color => set_lum(source, lum(backdrop)),
            BlendMode::Luminosity => set_lum(backdrop, lum(source)),
            separable => {
                let mut out = [0.0; 3];
                for c in 0..3 {
                    out[c] = separable.blend_channel(backdrop[c], source[c]);
                }
                out
            }
        }
    }

    /// The separable blend function on one channel. Only meaningful for the separable modes.
    fn blend_channel(self, backdrop: f32, source: f32) -> f32 {
        match self {
            BlendMode::Multiply => backdrop * source,
            BlendMode::Screen => backdrop + source - backdrop * source,
            BlendMode::Overlay => hard_light(source, backdrop),
            BlendMode::Darken => backdrop.min(source),
            BlendMode::Lighten => backdrop.max(source),
            BlendMode::Difference => (backdrop - source).abs(),
            BlendMode::Exclusion => backdrop + source - 2.0 * backdrop * source,
            _ => source,
        }
    }
}

/// Hard light with the roles of backdrop and source as in the spec: multiply when the
/// source is dark, screen when it is light.
fn hard_light(backdrop: f32, source: f32) -> f32 {
    if source <= 0.5 {
        backdrop * 2.0 * source
    } else {
        let s = 2.0 * source - 1.0;
        backdrop + s - backdrop * s
    }
}

/// Luminosity of an RGB colour (W3C weights).
fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

/// Saturation: the spread between the largest and smallest channel.
fn sat(c: [f32; 3]) -> f32 {
    let max = c[0].max(c[1]).max(c[2]);
    let min = c[0].min(c[1]).min(c[2]);
    max - min
}

/// Moves a colour to luminosity `l`, then pulls it back into `[0, 1]` (the spec's SetLum).
fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color([c[0] + d, c[1] + d, c[2] + d])
}

/// Scales the colour so the channels keep their order but fill a range of width `s` (SetSat).
fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let mut idx = [0usize, 1, 2];
    idx.sort_by(|&a, &b| {
        c[a].partial_cmp(&c[b])
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    let (min_i, mid_i, max_i) = (idx[0], idx[1], idx[2]);
    let mut out = [0.0; 3];
    let (cmin, cmid, cmax) = (c[min_i], c[mid_i], c[max_i]);
    if cmax > cmin {
        out[mid_i] = (cmid - cmin) * s / (cmax - cmin);
        out[max_i] = s;
    }
    out[min_i] = 0.0;
    out
}

/// Pulls an out-of-range colour back into `[0, 1]` while keeping its luminosity (ClipColor).
fn clip_color(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut out = c;
    if n < 0.0 && l - n > 0.0 {
        for v in out.iter_mut() {
            *v = l + (*v - l) * l / (l - n);
        }
    }
    if x > 1.0 && x - l > 0.0 {
        for v in out.iter_mut() {
            *v = l + (*v - l) * (1.0 - l) / (x - l);
        }
    }
    out
}
