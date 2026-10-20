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
- [ ] Image/pattern sampling — not implemented; calls are ignored (no image decoding yet).
- [ ] Glyph drawing via positioned runs — not implemented; no glyph outlines yet.
- [~] **Milestone:** complex 2D scene anti-aliased to an image buffer — gradients, clips, strokes and AA verified by unit tests; glyph and image parts pending.

## Phase 3 — Vector Backends: PDF, SVG (Weeks 7–9)
### SvgBackend
- [x] Path → `d="..."` string (curves not flattened; deterministic number formatting)
- [x] Fill/stroke attrs, fill-rule, dashes, caps/joins, miter limit
- [~] Linear/radial gradient `<defs>`, patterns — gradients done (with `spreadMethod` and `color-interpolation`); pattern paint emits a placeholder.
- [x] Clip paths, transforms (`<g>`), opacity (fill/stroke opacity)
- [ ] Glyphs, images — placeholder comments/`<image>` references only; no outlines or bytes yet.
### PdfBackend
- [x] Content stream writer: `m`, `l`, `c`, `h`, `f`/`f*`, `S`, `W n`, `q`/`Q`, `cm` (`re` not used; rectangles are emitted as `m`/`l`/`h`)
- [x] Quad → cubic conversion for PDF
- [x] Stroke state operators (`w`, `J`, `j`, `M`, `d`)
- [~] Shading dictionaries for gradients (`sh`), patterns — Type 2 and stitched Type 3 functions done; Repeat/Reflect drawn as Pad; per-stop alpha ignored; patterns placeholder.
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
- [ ] Glyph + image draws — counted as unsupported; no outlines or pixel data yet.
### Canvas2dBackend (added)
- [x] `web-sys`/`wasm-bindgen` adapter mapping trait calls to Canvas2D — `tpt-canvas-canvas2d`, compiles for `wasm32-unknown-unknown`. Transform and clip pushes map to `save()`/`restore()`.
- [ ] Browser test (wasm-bindgen-test / headless) — not run; no browser here.

## Testing & Quality (continuous)
- [x] Golden-file SVG/PDF scenes (translucent gradients, dashed strokes, complex clips) — `crates/tpt-canvas-svg/tests/golden.rs`, `crates/tpt-canvas-pdf/tests/structure.rs`. Regenerate with `TPT_UPDATE_GOLDEN=1`.
- [x] Tessellation fuzzing (millions of random/malformed paths; no panics) — 1,000,000 malformed contours with holes (`--ignored` test, run in release: 1.8 s, no panics). Smaller fuzz runs are in the default suite for paths, stroker, tessellator, and raster.
- [x] Transform property tests (inverse/identity, deep state stacks)
- [x] Flatten/arc property tests and cross-platform determinism checks (native and wasm32 digests agree).
- [x] Recording-backend equivalence: immediate mode vs. retained replay produce same output (core, and SVG output)
- [x] WebGPU command list vs. raster backend parity (rules, self-intersections, clips, dashed strokes, random polygons)
- [x] `no_std` build verified: `wasm32-unknown-unknown` and `thumbv7em-none-eabihf` (run locally; CI config added, not run remotely)
- [x] Verify zero C-FFI (dependency audit via `cargo tree`; `cargo deny` not installed)

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
- [ ] Glyph and image rendering in every backend (needs tpt-text outlines and image decoding)
- [x] License choice: MIT OR Apache-2.0.
