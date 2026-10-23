//! Visual regression tests for tpt-canvas scenes.
//!
//! [`assert_render!`] draws a scene on the raster backend, then compares the pixels with a
//! golden file in the calling crate's `snapshots/` directory. The raster output is
//! deterministic, so the comparison is byte-exact on every platform.
//!
//! The golden file is `<name>.rgba`: an 8-byte header (width and height as little-endian
//! `u32`), then straight-alpha RGBA8 pixels. Set `TPT_UPDATE_GOLDEN=1` to write the files.
//!
//! ```ignore
//! use tpt_canvas_snapshot::assert_render;
//!
//! #[test]
//! fn button() {
//!     assert_render!(64, 32, |ctx| ctx.fill_rect(0.0, 0.0, 64.0, 32.0), "button");
//! }
//! ```

#![warn(missing_docs)]

use std::path::Path;

use tpt_canvas::testkit::check_golden;
use tpt_canvas::Context;
use tpt_canvas_raster::RasterBackend;

/// Renders `scene` into a `width` x `height` raster and returns the golden encoding.
pub fn render_bytes(
    width: u32,
    height: u32,
    scene: impl FnOnce(&mut Context<RasterBackend>),
) -> Vec<u8> {
    let mut ctx = Context::new(RasterBackend::new(width, height));
    scene(&mut ctx);
    let pixmap = ctx.into_backend().into_pixmap();
    let mut out = Vec::with_capacity(8 + (width as usize) * (height as usize) * 4);
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&pixmap.to_rgba8());
    out
}

/// Checks a rendered scene against the golden at `path`. Used by [`assert_render!`].
///
/// Returns the mismatch message from [`check_golden`] on failure.
pub fn check_render(
    path: &Path,
    width: u32,
    height: u32,
    scene: impl FnOnce(&mut Context<RasterBackend>),
) -> Result<(), String> {
    let bytes = render_bytes(width, height, scene);
    check_golden(path, &bytes)
}

/// Asserts that a scene renders to its stored golden.
///
/// `assert_render!(width, height, |ctx| { ... }, "name")` compares against
/// `snapshots/<name>.rgba` next to the calling crate's `Cargo.toml`.
#[macro_export]
macro_rules! assert_render {
    ($width:expr, $height:expr, $scene:expr, $name:expr $(,)?) => {{
        let path = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("snapshots")
            .join(format!("{}.rgba", $name));
        if let Err(msg) = $crate::check_render(&path, $width, $height, $scene) {
            panic!("snapshot '{}' failed: {msg}", $name);
        }
    }};
}

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_examples {}
