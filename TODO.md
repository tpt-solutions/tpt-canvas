# tpt-canvas — Project Checklist

Source of truth: `spec.txt`. Target: v1.0.0 (all 4 phases).

Legend: `[x]` done and tested · `[~]` partly done (note says what is missing) · `[ ]` not started.

## Decisions (from clarifying Q&A)
- Ecosystem crates (tpt-math, tpt-color, tpt-text, tpt-raster, tpt-testkit, tpt-uir, tpt-model) do **not** exist yet → stubbed locally, swappable later. See `docs/swap-guide.md`.
- **Cargo workspace**: `no_std + alloc` core; each backend is its own crate (keeps wgpu/std out of core).
- Numerics: **f32 + `libm` crate** (no platform libm) for byte-for-byte determinism.
- Backends: PDF, SVG, Raster, WebGPU **+ Canvas2D** (added beyond spec Module D).
- Raster: small scanline AA rasterizer in a `tpt-raster-stub` crate behind a trait.
- WebGPU: tessellator + draw-command list is testable/fuzzable headlessly; `wgpu` lives in a thin optional adapter crate.
- Arcs are lowered to cubic Béziers when the path is built (`PathCommand` has no arc variant). Vector backends never see arcs.
- Path serialization is a self-contained little-endian format (`"TPTP"`, version 1). `tpt-uir` is not present yet.

## Phase 0 — Workspace & Stubs
- [x] Create Cargo workspace (`Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, license) — dual MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`).
- [x] `git init` (repo already exists), CI config (fmt, clippy, test, `no_std` build check on `wasm32-unknown-unknown`) — `.github/workflows/ci.yml` written; not run on GitHub from here.
- [x] Crate `tpt-canvas` (core, `#![no_std]` + `alloc`, `libm` dep)
- [x] Crate `tpt-math-stub`: `Vec2`, `Rect`, `Affine2` (mul, invert, translate/rotate/scale, bbox ops)
- [x] Crate `tpt-color-stub`: `Color`, gradient stop resolution, alpha compositing, color space enum
- [x] Crate `tpt-text-stub`: `ShapedRun`, positioned glyph types
- [x] Crate `tpt-raster-stub`: rasterizer trait + minimal scanline AA implementation
- [x] Crate `tpt-testkit-stub`: golden-file helper, property/fuzz harness helpers
- [x] Stub `ImageId` / `ImageRef` types (decoding is out of scope)
- [x] Document how each stub maps to the real crate (swap plan) — `docs/swap-guide.md`

## Phase 1 — Paths & Math (Weeks 1–3)
### Path (Module A)
- [x] `PathCommand` enum (MoveTo, LineTo, QuadTo, CubicTo, Close) — arcs are lowered to cubics, so there is no `ArcTo` variant.
- [x] `Path` struct + `PathBuilder` (`move_to`, `line_to`, `quad_to`, `cubic_to`, `arc_to`, `close`, `arc`, `elliptical_arc`)
- [x] Convenience shapes: `rect`, `rounded_rect`, `ellipse`/`circle`
- [x] `Path::bounding_box()` (exact for curves, via extrema)
- [x] Path transform (apply `Affine2`)
- [x] Compact serialized path format (flat buffer); round-trip tests — `tpt-uir` interface not defined (crate absent)
### ArcConverter (spec's "biggest trap")
- [x] HTML5 `arc()` semantics: start/end angle, counter-clockwise flag, full-circle and >2π sweeps
- [x] `arcTo(x1,y1,x2,y2,r)` tangent-arc semantics
- [x] SVG endpoint-arc → center parameterization (rx/ry/rotation/flags, radius scaling)
- [x] Arc → cubic Bézier conversion (segment splitting ≤ 90°, handle `4/3·tan(θ/4)`)
- [x] Arc → line segments via flattening
- [x] Heavy tests: degenerate arcs, coincident points, collinear corners, NaN/overflow guards, radial error bound vs. true circle (`tests/geometry.rs`)
### Flattening
- [x] Quad/cubic flattening with tolerance (uniform subdivision, fixed-order arithmetic)
- [x] `FlattenedPath` type (subpaths, closed flags)
- [x] Cross-platform determinism test (hashes on native + wasm32) — `tests/determinism.rs` produces one digest over flattening, stroking, tessellation and path bytes. The same digest is produced natively and under Node on wasm32. CI runs the wasm side.
- [x] Tolerance sweep tests (max deviation ≤ tolerance)
### Math integration
- [x] Wire `Affine2`/`Rect` into path code
- [x] Property tests: `A · A⁻¹ == identity` within defined bound; no drift over deep stacks (5000 ops)

## Phase 2 — Styles & Raster Backend (Weeks 4–6)
### Style (Module B)
- [x] `FillRule` (NonZero, EvenOdd)
- [x] `Paint` enum: `Solid`, `LinearGradient`, `RadialGradient`, `Pattern(ImageId, Transform)`
- [x] Gradient definition (stops, spread/extend mode, color interpolation space) — decided: `SpreadMode {Pad, Repeat, Reflect}`, `ColorSpace {Srgb, LinearSrgb}`, linear = start/end, radial = center/radius (no focal point).
- [x] `StrokeStyle`: width, `LineCap`, `LineJoin`, miter limit, dash array + offset
- [x] Stroker: path → stroke outline (caps, joins, miter limit, dashes) — outline is a set of CCW polygons; nonzero fill gives the union.
### Context (Module C)
- [x] `Context` with state stack (`save`/`restore`): transform, clip, default styles
- [x] Transforms: `translate`, `rotate`, `scale`, `transform`, `set_transform`
- [x] Clip: intersect with new `Path`
- [x] Immediate-mode drawing API (`fill_path`, `stroke_path`, `draw_glyphs`, `draw_image`) — glyph and image draws reach the backend; drawing is backend-specific (see below).
- [x] Retained mode: record draw calls into a display list; replay to any backend; binary serialization (`display/codec.rs`) with round-trip tests.
### Backend trait (Module D)
- [x] `CanvasBackend` trait (`fill_path`, `stroke_path`, `draw_glyphs`, `draw_image`, `push/pop_clip`, `push/pop_transform`)
- [x] Mock/recording backend for tests — `DisplayList`
### RasterBackend
- [x] Rasterizer stub: scanline AA coverage, fill rules
- [x] Gradient + solid shading, alpha compositing
- [x] Clip masks
- [x] Stroke rendering via stroker
- [x] Image/pattern sampling — raster only: `RasterBackend::add_image` registers pixels, and `draw_image` and image patterns sample nearest-neighbour. Unregistered images are counted.
- [x] Glyph drawing via positioned runs — `GlyphSource` seam in core: `Context::draw_glyphs` turns each glyph into a transformed fill, so every backend draws text. Tested on raster. Real outlines and shaping wait for `tpt-text`.
- [~] **Milestone:** complex 2D scene anti-aliased to an image buffer — gradients, clips, strokes and AA verified by unit tests; glyph and image parts pending.

## Phase 3 — Vector Backends: PDF, SVG (Weeks 7–9)
### SvgBackend
- [x] Path → `d="..."` string (curves not flattened; deterministic number formatting)
- [x] Fill/stroke attrs, fill-rule, dashes, caps/joins, miter limit
- [~] Linear/radial gradient `<defs>`, patterns — gradients done (with `spreadMethod` and `color-interpolation`); pattern paint emits a placeholder.
- [x] Clip paths, transforms (`<g>`), opacity (fill/stroke opacity)
- [~] Glyphs, images — glyphs arrive as paths through `GlyphSource` (tested). Images are still placeholder `<image>` references, pending a decoder.
### PdfBackend
- [x] Content stream writer: `m`, `l`, `c`, `h`, `f`/`f*`, `S`, `W n`, `q`/`Q`, `cm` (`re` not used; rectangles are emitted as `m`/`l`/`h`)
- [x] Quad → cubic conversion for PDF
- [x] Stroke state operators (`w`, `J`, `j`, `M`, `d`)
- [~] Shading dictionaries for gradients (`sh`), patterns — Type 2 and stitched Type 3 functions done. Linear Repeat/Reflect are tiled. Radial spreads and per-stop alpha are drawn approximately and counted; patterns are a placeholder.
- [x] Alpha via ExtGState
- [ ] Glyph + image XObject emission (interface to tpt-doc) — placeholders only.
- [~] Output a valid minimal PDF; validate with external viewer/qpdf — xref offsets, balanced `q`/`Q`, and object headers checked in tests; **no external viewer run** (no qpdf here).
- [ ] **Milestone:** tpt-doc (stub consumer) generates vector graphics in a PDF — tpt-doc does not exist; not started.

## Phase 4 — WebGPU, Canvas2D & Clipping (Weeks 10–12)
### Tessellation
- [x] Polygon tessellator with NonZero/EvenOdd handling — `tpt-canvas::tessellate`: ear clipping (own implementation) for simple contours with holes; fan triangulation for the winding-exact stencil path (see WebGPU below). Chose ear clipping over earcut to keep the code in-tree.
- [x] Robustness: duplicate points and zero-area triangles are dropped; holes are bridged to visible vertices; self-intersecting input never panics. Ear clipping does not give a valid decomposition for self-intersecting input. Fills of such input go through the stencil path, which is exact.
- [x] Stroke tessellation (via stroker) — stroke outlines are filled as fans with nonzero.
- [~] Gradient UV / uniform generation — the paint table (`PaintEntry`: paint + device-to-user matrix) is the uniform data; no shader or GPU buffer layout is written yet.
### WebGpuBackend
- [x] Vertex/index buffer + draw-command list output (headless, no GPU needed) — `tpt-canvas-webgpu`. Stencil-then-cover commands. A CPU executor (`reference::execute`) runs the same commands, and parity tests check it against the raster backend.
- [~] Stencil-buffer clip system (push/pop nesting) — clips are mask layers computed the same way and AND-ed with their parent, not literal stencil bits. Same semantics; a GPU adapter decides how to store the layers.
- [x] Thin `wgpu` adapter crate: pipelines, shaders (WGSL), submit — `tpt-canvas-wgpu` (wgpu 30). Verified on a local Intel GPU: 8 GPU-vs-reference parity tests pass. They skip without an adapter, so GPU-less CI runners will not exercise them.
- [~] Glyph + image draws — glyphs drawn as paths through `GlyphSource`. Images are still counted as unsupported.
### Canvas2dBackend (added)
- [x] `web-sys`/`wasm-bindgen` adapter mapping trait calls to Canvas2D — `tpt-canvas-canvas2d`, compiles for `wasm32-unknown-unknown`. Transform and clip pushes map to `save()`/`restore()`.
- [x] Browser tests (wasm-bindgen-test, headless Chrome) — 7 tests read pixels back with getImageData. CI runs them in a dedicated job with a matching chromedriver.

## Testing & Quality (continuous)
- [x] Golden-file SVG/PDF scenes (translucent gradients, dashed strokes, complex clips) — `crates/tpt-canvas-svg/tests/golden.rs`, `crates/tpt-canvas-pdf/tests/structure.rs`. Regenerate with `TPT_UPDATE_GOLDEN=1`.
- [x] Tessellation fuzzing (millions of random/malformed paths; no panics) — 1,000,000 malformed contours with holes (`--ignored` test, run in release: 1.8 s, no panics). Smaller fuzz runs are in the default suite for paths, stroker, tessellator, and raster.
- [x] Transform property tests (inverse/identity, deep state stacks)
- [x] Flatten/arc property tests and cross-platform determinism checks (native and wasm32 digests agree).
- [x] Recording-backend equivalence: immediate mode vs. retained replay produce same output (core, and SVG output)
- [x] WebGPU command list vs. raster backend parity (rules, self-intersections, clips, dashed strokes, random polygons)
- [x] `no_std` build verified: `wasm32-unknown-unknown` and `thumbv7em-none-eabihf` (run locally; CI config added, not run remotely)
- [x] Verify zero C-FFI (dependency audit via `cargo tree`; `renderdoc-sys` allowed, see README; `cargo deny` not installed)

## Release — v1.0.0
- [x] Public API review and doc comments on all public items — every public item in the real crates is documented, and `#![warn(missing_docs)]` guards them. The stub crates allow missing docs on purpose, since they get replaced.
- [x] Examples (rounded gradient button pipeline from spec §4, pie chart, gear) — `crates/tpt-canvas-svg/examples/` (plus `quickstart`).
- [x] README + architecture doc + stub-swap guide — README, `docs/swap-guide.md`; architecture is in the README layout table.
- [~] Changelog, versioning, publish checklist — `CHANGELOG.md` written; publish checklist not done.
- [ ] Tag v1.0.0

## Open items to revisit
- [x] Gradient and pattern details (spec elides `{ ... }` fields) — decided, see Phase 2 Style.
- [ ] Swap stubs for real `tpt-*` crates once available
- [ ] Canvas2D scope (added by request; not in spec Module D) — update spec if kept
- [ ] Real text shaping and outlines (`tpt-text` is not published yet; the `GlyphSource` seam is ready for it)
- [ ] Image decoding and pattern paints (needs an image decoder, for example `tpt-kinetix`)
- [x] License choice: MIT OR Apache-2.0.

## Review follow-ups (2026-10)
Source: platform review. Ordered roughly by priority within each group.

### Bugs and inconsistencies
- [x] README is stale: says WebGPU/Canvas2D are "planned", and the `lib.rs` module doc says "(later)". Update both; show how to get pixels out of the raster backend.
- [x] `Context::restore` pops all clips, then all transforms, though they were pushed interleaved (`context.rs`). Track one stack of op kinds so backend pops mirror push order. Fix before 1.0.0 since it affects backend authors.
- [x] Remove the duplicated doc comment on `Context::draw_glyphs`.
- [x] Unsupported features (images, patterns) are silent no-ops or placeholders. Add an "unsupported" counter or warning list to every backend (WebGPU already counts images).
- [~] PDF: linear Repeat/Reflect are tiled now. Radial spreads and per-stop alpha are still approximated, and the approximation is counted by `PdfBackend::unsupported()`. Implementing them needs a radial tiling scheme and soft masks for alpha.
- [ ] wgpu GPU parity tests skip without an adapter, so CI never runs them. Use a software adapter (lavapipe/llvmpipe) on the Linux runner.
- [x] Decide how stub crates are published — inlined. The stand-ins are modules in `tpt-canvas` (`math`, `color`, `text`, `raster`, `testkit`), and the stub crates are deleted. Their doc comments are not written yet (the modules are `#[allow(missing_docs)]`).

### Missing features
- [x] PNG encoder for `Pixmap` (dependency-free, stored blocks).
- [~] `Context` helpers: `fill_rect`, `stroke_rect`, `fill_circle` and `set_global_alpha` done. `clear` still missing (it needs a new backend op).
- [x] `Color` helpers: `from_hex`, `from_rgb8`, named constants, `lighten`/`darken`/`mix` (in `tpt_canvas::color`, as `ColorExt`).
- [x] `Paint` helpers: `Paint::linear(start, end, &[(offset, color)])` and a radial equivalent.
- [ ] Group opacity / layers (`push_layer`) and blend modes (multiply, screen, ...).
- [ ] Shadows and blur.
- [x] SVG path-string parser: `Path::from_svg_d` (all commands, absolute and relative) and the `path!` macro.
- [ ] Built-in basic font so examples can render labels before `tpt-text` exists.
- [~] Multi-page PDF: `PdfBackend::new_page` works, and the page tree and xref are tested. PDF metadata and embedded fonts are not done.
- [x] Hit-testing: `Path::contains_point` (non-zero and even-odd) and `Path::stroke_contains_point`.
- [~] Raw RGBA image store: `Pixmap::from_rgba8` and `RasterBackend::add_image`, raster only. SVG, PDF and Canvas2D do not draw images yet.

### Innovation
- [ ] Snapshot crate with `assert_render!(scene, "name")` for downstream visual regression tests (builds on the golden helpers).
- [ ] Cross-backend diff tool: render one scene through raster, WebGPU reference, SVG and PDF; report per-pixel disagreement. Doubles as a conformance suite for third-party backends.
- [~] Display-list tooling: the `display_dump` example prints recorded ops. Diff, time-travel viewer and replay are not done.
- [ ] Browser playground (wasm raster or Canvas2D backend with live editor) on GitHub Pages.
- [x] `path!{ M 0 0 L 10 10 Z }` macro (parses at run time and panics on a bad literal).
- [ ] Optional: retained scene graph with dirty-rect tracking on top of `DisplayList`.

### Automation and CI
- [ ] `cargo-deny` job (licenses, advisories, bans) replacing the hand-written `-sys` grep.
- [ ] `cargo-semver-checks` job.
- [ ] Coverage job with `cargo-llvm-cov` and a badge.
- [ ] `cargo xtask` (or justfile): test, golden-update, wasm, examples.
- [ ] Dependabot/renovate (wgpu 30 and wasm-bindgen 0.2.129 are pinned in several places).
- [ ] Criterion benches for flatten, stroke, tessellate and raster fill; fail on large regressions.
- [ ] Nightly `cargo-fuzz` job (the million-path test is `--ignored` and never runs in CI).
- [ ] `cargo-release` or `release-plz` for the publish checklist.
- [ ] Run examples in CI and upload SVG/PNG/PDF outputs as artifacts.
- [ ] `cargo doc` with `-D warnings`, published to Pages.
- [ ] Validate generated PDFs in CI with `qpdf --check`.
- [ ] Run CI on GitHub at least once (never run remotely so far).

### Adoption
- [ ] README rewrite: gallery image at top, same code producing SVG/PDF/PNG, comparison table vs tiny-skia, vello, lyon.
- [~] Top-level examples gallery: `crates/tpt-gallery` has `all_backends`, `shapes`-style scene, `bar_chart`, `analog_clock`, `pdf_table` and `display_dump`, all run with `cargo run -p tpt-gallery --example X`. Missing: QR code, a Canvas2D wasm demo, and a `thumbv7em` embedded demo.
- [x] `prelude` module (`use tpt_canvas::prelude::*`).
- [x] Facade crate: `crates/tpt-canvas-facade` with features `svg`, `pdf`, `raster`, `webgpu`, `wgpu` and `canvas2d`. It is `publish = false`.
- [~] Docs: `docs/tour.md` covers transform order, arcs, paths, hit testing, images and per-backend support. It is not an mdbook yet, and does not cover writing a backend (see `CONTRIBUTING.md`).
- [ ] Migration cheat sheets: from HTML Canvas, cairo, tiny-skia.
- [ ] `cargo generate` templates: CLI renderer, wasm app, custom backend skeleton, chart library.
- [ ] Publish 0.x to crates.io for docs.rs and discoverability.
- [~] `CONTRIBUTING.md` written. Issue templates and "good first issue" labels are not done.
