//! One dependency for tpt-canvas. Enable the backends you need as features.
//!
//! ```toml
//! [dependencies]
//! tpt-canvas-facade = { path = "...", features = ["svg", "pdf"] }
//! ```
//!
//! The drawing API is re-exported at the root and in [`prelude`]. Each enabled backend
//! appears under its own module name, such as [`svg`] or [`raster`].
#![no_std]

pub use tpt_canvas::*;

/// The drawing API and the names most code needs. `use tpt_canvas_facade::prelude::*;`.
pub mod prelude {
    pub use tpt_canvas::prelude::*;
}

/// Raster backend (`raster` feature).
#[cfg(feature = "raster")]
pub mod raster {
    pub use tpt_canvas_raster::*;
}

/// SVG backend (`svg` feature).
#[cfg(feature = "svg")]
pub mod svg {
    pub use tpt_canvas_svg::*;
}

/// PDF backend (`pdf` feature).
#[cfg(feature = "pdf")]
pub mod pdf {
    pub use tpt_canvas_pdf::*;
}

/// Headless WebGPU command lists (`webgpu` feature).
#[cfg(feature = "webgpu")]
pub mod webgpu {
    pub use tpt_canvas_webgpu::*;
}

/// Real GPU execution through wgpu (`wgpu` feature).
#[cfg(feature = "wgpu")]
pub mod wgpu {
    pub use tpt_canvas_wgpu::*;
}

/// Canvas2D backend for the browser (`canvas2d` feature, wasm32 only).
#[cfg(all(feature = "canvas2d", target_arch = "wasm32"))]
pub mod canvas2d {
    pub use tpt_canvas_canvas2d::*;
}
