# Changelog

## Unreleased

### Added (text labels)
- `tpt_canvas::text::StrokeFont`: a built-in stroke font for digits, capitals (lowercase maps to them) and a few symbols. It is a `GlyphSource`, so labels go through `draw_glyphs` on every backend. `StrokeFont::layout` and `StrokeFont::width` lay out a string. The `labels` example renders it.

### Added (blend modes)
- `Context::set_blend_mode(BlendMode)` and `blend_mode()`. Twelve modes: Normal, the eight separable modes (Multiply, Screen, Overlay, Darken, Lighten, Difference, Exclusion) and the four non-separable ones (Hue, Saturation, Color, Luminosity). The mode is saved and restored with `save`/`restore`. It applies to fills and strokes, not to group layers or `clear_rect`.
- New required `CanvasBackend::set_blend_mode`. Raster composites with the W3C formula. SVG writes `mix-blend-mode`, PDF writes `/BM` into its ExtGState (gradients too), and Canvas2D sets `globalCompositeOperation`. Canvas2D was compiled for wasm but not run in a browser. WebGPU counts blended draws.
- `DrawOp::SetBlendMode` in the display list, encoded as tag 12 with one byte per mode (0 to 11). Older decoders reject this tag.

### Added (SVG blur)
- Blurred shadows are an `feGaussianBlur` filter on SVG, sized to the blur spread.

### Added (shadows)
- Shadows on strokes. An unblurred stroke shadow is the stroke moved by the offset. A blurred one is the stroke outline (flattened at a fixed tolerance) blurred as a fill.
- `Context::set_shadow(Shadow { color, offset, blur })` and `clear_shadow()`. A shadow is drawn under each fill and stroke, and it is saved and restored with the other state.
- New required `CanvasBackend::fill_path_blurred(path, paint, rule, sigma)`. Raster blurs the coverage with three box passes and clips it. SVG draws it as a filter. PDF, WebGPU and Canvas2D count the call and draw nothing for it.
- `DrawOp::FillPathBlurred` in the display list, encoded as tag 11. Older decoders reject this tag.

### Added (benchmarks)
- `tpt-canvas-bench`: criterion benchmarks for flatten, stroke, tessellate and raster fill. CI runs them once each (`bench-smoke`).

### Added (testing and tooling)
- `tpt-canvas-snapshot`: `assert_render!(width, height, scene, "name")` renders a scene on the raster backend and compares it byte-exact with `snapshots/<name>.rgba`. Set `TPT_UPDATE_GOLDEN=1` to write goldens.
- `tpt-canvas-diff`: renders one scene on the raster backend and the WebGPU reference executor, and reports coverage mismatches and channel deltas. SVG and PDF are not compared yet.
- `fuzz/` cargo-fuzz targets (`svg_path`, `path_deserialize`) and a nightly workflow in `.github/workflows/fuzz.yml`.
- A `justfile` with the common tasks, and a `cargo-semver-checks` job on pull requests.
- `docs/migration-html-canvas.md`: maps the HTML Canvas API to tpt-canvas.

### Fixed (CI)
- The `no_std` build step named the `tpt-*-stub` crates, which were removed, so it could not pass. It builds `tpt-canvas` now.

### Added (group opacity)
- `Context::push_layer(alpha)` and `Context::pop_layer()` composite everything drawn inside the group as one unit with `alpha`. Overlapping shapes in the group are not double-faded, unlike setting global alpha. `restore` also closes an open layer.
- New required `CanvasBackend::push_layer` and `pop_layer`. Raster composites a real offscreen group. SVG writes `<g opacity>`. Canvas2D, PDF and WebGPU count the call; they cannot composite a group yet.
- `DrawOp::PushLayer` and `DrawOp::PopLayer` (codec tags 9 and 10).
- `Context::pop_layer` only closes a layer that is the latest thing pushed at its save level. It is a no-op otherwise.

### Added (clearing)
- `Context::clear_rect(x, y, w, h)` sets a rectangle back to transparent, like canvas `clearRect`. The fill paint and global alpha do not apply; the transform and clip do.
- New required `CanvasBackend::clear_path` method. Backend authors must implement it. Raster erases covered pixels (partial coverage fades them). Canvas2D uses `destination-out`. SVG, PDF and WebGPU cannot erase, so they count the call in `unsupported()`.
- `DrawOp::ClearPath` in the display list, encoded as tag 8. Older decoders reject this tag, so lists with clears need a current reader.

### Added (CI and project automation)
- `deny.toml` policy for `cargo deny`: permissive licenses only, no unknown registries, no wildcard versions outside the workspace. CI runs it in a `deny` job. The `-sys` name audit stays in the `check` job, because cargo-deny cannot match name patterns.
- CI builds the rustdoc with `RUSTDOCFLAGS=-D warnings`. The doc warnings that failed it are fixed.
- CI runs the gallery examples, checks their PDFs with `qpdf --check`, and uploads the outputs as an artifact.
- CI runs the wgpu parity tests on Mesa's lavapipe software Vulkan driver, so they no longer skip on the runner.
- CI coverage job with `cargo-llvm-cov` that uploads an lcov file.
- Dependabot for cargo and GitHub Actions, with the wgpu and wasm-bindgen stacks grouped.
- Issue templates for bug reports and feature requests.

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
