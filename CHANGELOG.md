# Changelog

## Unreleased

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
- `tpt-canvas-canvas2d`: Canvas2D backend for wasm32 (compiles; not yet exercised in a browser).
- Display-list binary serialization (`DisplayList::serialize` / `deserialize`).
- Examples: `button` (spec §4 pipeline), `pie`, `gear`, and `quickstart`.

### Not yet implemented
- Glyph outlines and image pixels in every backend (placeholders or unsupported counters).
- Pattern paints in every backend.
- Repeat and Reflect gradient spreads in the PDF and Canvas2D backends (drawn as Pad).
- Browser tests for Canvas2D.
- Full doc coverage of public items (about 150 items still undocumented in the real crates).
- `tpt-canvas-wgpu`: wgpu adapter running WebGPU draw lists on a GPU (stencil-then-cover, R8 clip layers, WGSL). Parity tests pass on a local GPU.
- Glyph and image rendering (needs `tpt-text` outlines and image decoding from ecosystem crates).
