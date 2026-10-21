//! tpt-canvas: a deterministic, pure-Rust 2D vector graphics API.
//!
//! The drawing API ([`Context`]) is written once and dispatched to any
//! [`CanvasBackend`]: PDF, SVG, raster, WebGPU, or Canvas2D.
//!
//! - Immediate mode: call [`Context`] methods and the backend sees each draw call.
//! - Retained mode: record into a [`DisplayList`], then [`DisplayList::replay`]
//!   into any backend, or serialize paths with [`Path::serialize`].
//!
//! Arcs are lowered to cubic Béziers when a path is built, so vector backends
//! never see arc commands. Flattening is the only place curves become lines.
#![warn(missing_docs)]
#![no_std]

extern crate alloc;
// `std` is needed only for the golden-file helper, which is behind the `testkit` feature.
#[cfg(feature = "testkit")]
extern crate std;

pub mod backend;
#[allow(missing_docs)]
pub mod color;
pub mod context;
pub mod display;
pub mod image;
#[allow(missing_docs)]
pub mod math;
pub mod path;
#[allow(missing_docs)]
pub mod raster;
pub mod scene;
pub mod stroke;
pub mod style;
pub mod tessellate;
#[allow(missing_docs)]
pub mod testkit;
#[allow(missing_docs)]
pub mod text;

pub use backend::CanvasBackend;
pub use context::Context;
pub use display::{DisplayList, DrawOp};
pub use image::{ImageId, ImageRef};
pub use path::{FlattenedPath, Path, PathBuilder, PathCommand, Polyline, SvgPathError};
pub use scene::{ItemId, Scene};
pub use style::{BlendMode, FillRule, LineCap, LineJoin, Paint, Shadow, SpreadMode, StrokeStyle};
pub use text::GlyphSource;

pub use color::{Color, ColorSpace, Stop};
pub use math::{Affine2, Rect, Vec2};
pub use text::{PositionedGlyph, ShapedRun};

/// The names most drawing code needs. `use tpt_canvas::prelude::*;` brings them in.
pub mod prelude {
    pub use crate::color::{named, ColorExt};
    pub use crate::{
        Affine2, BlendMode, CanvasBackend, Color, ColorSpace, Context, DisplayList, DrawOp,
        FillRule, GlyphSource, ImageId, ImageRef, LineCap, LineJoin, Paint, Path, PathBuilder,
        Rect, SpreadMode, Stop, StrokeStyle, Vec2,
    };
}
