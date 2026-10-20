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
