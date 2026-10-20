//! Paints, fill rules and stroke styles (Module B).

use alloc::vec::Vec;

use crate::image::ImageId;
use crate::{Affine2, Color, ColorSpace, Stop, Vec2};

/// Winding rule used when filling a path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FillRule {
    #[default]
    NonZero,
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
    Solid(Color),
    /// Gradient from `start` (t = 0) to `end` (t = 1), in user space.
    LinearGradient {
        start: Vec2,
        end: Vec2,
        stops: Vec<Stop>,
        spread: SpreadMode,
        space: ColorSpace,
    },
    /// Circle gradient: t = distance from `center` / `radius`, in user space.
    RadialGradient {
        center: Vec2,
        radius: f32,
        stops: Vec<Stop>,
        spread: SpreadMode,
        space: ColorSpace,
    },
    /// Tiled image; `transform` maps image space into user space.
    Pattern {
        image: ImageId,
        transform: Affine2,
    },
}

impl Paint {
    pub fn solid(color: Color) -> Self {
        Paint::Solid(color)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// Stroke geometry parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct StrokeStyle {
    pub width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
    pub miter_limit: f32,
    /// Alternating on/off lengths. Empty means solid. Odd-length arrays are repeated.
    pub dash: Vec<f32>,
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
