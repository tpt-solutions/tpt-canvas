# Changelog

## Unreleased

### Changed (inlined stand-ins)
- The `tpt-math-stub`, `tpt-color-stub`, `tpt-text-stub`, `tpt-raster-stub` and `tpt-testkit-stub` crates are gone. Their code now lives in `tpt-canvas` as the `math`, `color`, `text`, `raster` and `testkit` modules, so the public crates have no unpublished dependencies.
- Import paths change: `tpt_color_stub::X` becomes `tpt_canvas::color::X`, and the same pattern applies to the other four.
- `testkit::check_golden` is behind the new `testkit` feature of `tpt-canvas`, because it needs std.

### Added (follow-up round 2)
- Raster backend draws images: `RasterBackend::add_image` registers pixels (`Pixmap::from_rgba8`), and `draw_image` and `Paint::Pattern` sample them nearest-neighbour. Unregistered images are counted.
- Linear gradients with `Repeat` or `Reflect` are tiled in the PDF backend. Radial spreads and per-stop alpha are still approximated and counted.
- Canvas2D counts gradients with non-pad spreads, which it draws as pad.
- `Path::contains_point` (non-zero and even-odd) and `Path::stroke_contains_point`.
- `Path::from_svg_d` parses SVG path data (all commands, absolute and relative). `SvgPathError` reports the byte offset. The `path!` macro wraps it.
- `Context::set_global_alpha` and `global_alpha`, restored by `restore`. `Paint::with_opacity` does the scaling.
- `PdfBackend::new_page`, for multi-page documents.
- `tpt-gallery` crate (`publish = false`) with examples: `all_backends`, `bar_chart`, `analog_clock`, `pdf_table`, `display_dump`.
- `tpt-canvas-facade` crate with backend feature flags (`publish = false`).
- `CONTRIBUTING.md` and `docs/tour.md`.

### Added (review follow-ups)
- `Context::restore` pops backend clips and transforms in the order they were pushed. Before, all clips were popped before all transforms.
- `unsupported()` on the raster, SVG, PDF and Canvas2D backends counts calls they skip or approximate: pattern paints, glyph runs, images, and PDF gradients with a non-Pad spread or per-stop alpha.
- `Pixmap::to_png()`: dependency-free PNG encoder (8-bit RGBA, stored deflate).
- `tpt_canvas::color` (`ColorExt`: `from_hex`, `from_rgb8`, `from_rgba8`, `mix`, `lighten`, `darken`; `named` colours).
- `Paint::linear` and `Paint::radial` constructors.
- `Context::fill_rect`, `stroke_rect` and `fill_circle`.
- `tpt_canvas::prelude`.

### Changed
- README: WebGPU and Canvas2D are described as present, and there is a raster pixel-readout example. The example is compiled in a test.

### Added (this round)
- `GlyphSource` seam: `Context::draw_glyphs` draws glyph runs as paths, so every backend draws text.
- Browser tests for Canvas2D run in headless Chrome, and a CI job runs them.
- Per-crate README, CHANGELOG, keywords and categories.

### Added
- Cargo workspace with `rust-toolchain.toml` and CI (fmt, clippy `-D warnings`, tests,
  `no_std` builds for `wasm32-unknown-unknown` and `thumbv7em-none-eabihf`, FFI audit).
- Stub crates: `tpt-math-stub`, `tpt-color-stub`, `tpt-text-stub`, `tpt-raster-stub`, `tpt-testkit-stub`.
- `tpt-canvas` core (`no_std` + `alloc`):
  - `Path`, `PathCommand`, `PathBuilder` with lines, quads, cubics, close, rect, rounded rect, ellipse and circle.
  - Arcs: HTML5 `arc()`, tangent `arc_to()`, and SVG endpoint arcs. All are lowered to cubics of at most 90°.
  - Exact bounding boxes, affine transform, flattening with a guaranteed tolerance, and a compact serialized form.
  - Stroker: caps, joins, miter limit, dash arrays and offsets.
  - Styles: `FillRule`, `Paint` (solid, linear and radial gradients, pattern), `StrokeStyle`, `SpreadMode`.
  - `Context` with a save/restore state stack, transforms, clipping, and immediate-mode drawing.
  - `CanvasBackend` trait and `DisplayList` for retained mode (also a recording backend).
- `tpt-canvas-svg`: SVG backend, with a golden-file scene test.
- `tpt-canvas-pdf`: PDF backend, with xref validation and a golden-file scene test.
- `tpt-canvas-raster`: raster backend with clip masks, gradients, and stroking. Has fuzz and determinism tests.

### Added (Phase 4)
- `tpt_canvas::tessellate`: ear clipping for contours with holes, and fan triangulation.
- `tpt-canvas-webgpu`: headless WebGPU backend. Stencil-then-cover command lists, mask-layer clips, and a CPU reference executor used for parity tests against the raster backend.
- `tpt-canvas-canvas2d`: Canvas2D backend for wasm32, with headless Chrome tests.
- Display-list binary serialization (`DisplayList::serialize` / `deserialize`).
- Examples: `button` (spec §4 pipeline), `pie`, `gear`, and `quickstart`.

### Not yet implemented
- Glyph outlines and image pixels in every backend (placeholders or unsupported counters).
- Pattern paints in every backend.
- Repeat and Reflect gradient spreads in the PDF and Canvas2D backends (drawn as Pad).
- Doc comments on every public item in the real crates, with `#![warn(missing_docs)]`.
- `tpt-canvas-wgpu`: wgpu adapter running WebGPU draw lists on a GPU (stencil-then-cover, R8 clip layers, WGSL). Parity tests pass on a local GPU.
- Real text shaping and outlines (`tpt-text` is not published); image decoding and pattern paints.
