//! tpt-canvas: a deterministic, pure-Rust 2D vector graphics API.
//!
//! The drawing API ([`Context`]) is written once and dispatched to any
//! [`CanvasBackend`]: PDF, SVG, raster, or (later) WebGPU/Canvas2D.
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

pub mod backend;
pub mod context;
pub mod display;
pub mod image;
pub mod path;
pub mod stroke;
pub mod style;
pub mod tessellate;

pub use backend::CanvasBackend;
pub use context::Context;
pub use display::{DisplayList, DrawOp};
pub use image::{ImageId, ImageRef};
pub use path::{FlattenedPath, Path, PathBuilder, PathCommand, Polyline};
pub use style::{FillRule, LineCap, LineJoin, Paint, SpreadMode, StrokeStyle};

pub use tpt_color_stub::{Color, ColorSpace, Stop};
pub use tpt_math_stub::{Affine2, Rect, Vec2};
pub use tpt_text_stub::{PositionedGlyph, ShapedRun};
