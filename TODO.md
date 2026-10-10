# tpt-canvas — Project Checklist

Source of truth: `spec.txt`. Target: v1.0.0 (all 4 phases).

## Decisions (from clarifying Q&A)
- Ecosystem crates (tpt-math, tpt-color, tpt-text, tpt-raster, tpt-testkit, tpt-uir, tpt-model) do **not** exist yet → stubbed locally, swappable later.
- **Cargo workspace**: `no_std + alloc` core; each backend is its own crate (keeps wgpu/std out of core).
- Numerics: **f32 + `libm` crate** (no platform libm) for byte-for-byte determinism.
- Backends: PDF, SVG, Raster, WebGPU **+ Canvas2D** (added beyond spec Module D).
- Raster: small scanline AA rasterizer in a `tpt-raster-stub` crate behind a trait.
- WebGPU: tessellator + draw-command list is testable/fuzzable headlessly; `wgpu` lives in a thin optional adapter crate.

## Phase 0 — Workspace & Stubs
- [ ] Create Cargo workspace (`Cargo.toml`, `rust-toolchain.toml`, `.gitignore`, license)
- [ ] `git init`, CI config (fmt, clippy, test, `no_std` build check on `wasm32-unknown-unknown`)
- [ ] Crate `tpt-canvas` (core, `#![no_std]` + `alloc`, `libm` dep)
- [ ] Crate `tpt-math-stub`: `Vec2`, `Rect`, `Affine2` (mul, invert, translate/rotate/scale, bbox ops)
- [ ] Crate `tpt-color-stub`: `Color`, gradient stop resolution, alpha compositing, color space enum
- [ ] Crate `tpt-text-stub`: `ShapedRun`, positioned glyph types
- [ ] Crate `tpt-raster-stub`: rasterizer trait + minimal scanline AA implementation
- [ ] Crate `tpt-testkit-stub`: golden-file helper, property/fuzz harness helpers
- [ ] Stub `ImageId` / `ImageRef` types (decoding is out of scope)
- [ ] Document how each stub maps to the real crate (swap plan)

## Phase 1 — Paths & Math (Weeks 1–3)
### Path (Module A)
- [ ] `PathCommand` enum (MoveTo, LineTo, QuadTo, CubicTo, ArcTo, Close)
- [ ] `Path` struct + `PathBuilder` (`move_to`, `line_to`, `quad_to`, `cubic_to`, `arc_to`, `close`)
- [ ] Convenience shapes: `rect`, `rounded_rect`, `ellipse`/`circle`
- [ ] `Path::bounding_box()` (exact for curves, via extrema)
- [ ] Path transform (apply `Affine2`)
- [ ] Compact serialized path format (flat buffer; define `tpt-uir` stub interface); round-trip tests
### ArcConverter (spec's "biggest trap")
- [ ] HTML5 `arc()` semantics: start/end angle, counter-clockwise flag, full-circle and >2π sweeps
- [ ] `arcTo(x1,y1,x2,y2,r)` tangent-arc semantics
- [ ] SVG endpoint-arc → center parameterization (rx/ry/rotation/flags, radius scaling)
- [ ] Arc → cubic Bézier conversion (segment splitting ≤ 90°, precise control-point constant)
- [ ] Arc → line segments via flattening
- [ ] Heavy tests: degenerate arcs, zero radius, coincident points, error bound vs. true circle
### Flattening
- [ ] Quad/cubic flattening with tolerance (strictly ordered, deterministic arithmetic)
- [ ] `FlattenedPath` type (subpaths, closed flags)
- [ ] Cross-platform determinism test (hashes of output on native + wasm32)
- [ ] Tolerance sweep tests (max deviation ≤ tolerance)
### Math integration
- [ ] Wire `Affine2`/`Rect` into path code
- [ ] Property tests: `A · A⁻¹ == identity` within defined bound; no drift over deep stacks

## Phase 2 — Styles & Raster Backend (Weeks 4–6)
### Style (Module B)
- [ ] `FillRule` (NonZero, EvenOdd)
- [ ] `Paint` enum: `Solid`, `LinearGradient`, `RadialGradient`, `Pattern(ImageId, Transform)`
- [ ] Gradient definition (stops, spread/extend mode, color interpolation space) — *define details; spec elides them*
- [ ] `StrokeStyle`: width, `LineCap`, `LineJoin`, miter limit, dash array + offset
- [ ] Stroker: path → stroke outline (caps, joins, miter limit, dashes) for backends that need geometry
### Context (Module C)
- [ ] `Context` with state stack (`save`/`restore`): transform, clip, default styles
- [ ] Transforms: `translate`, `rotate`, `scale`, `transform`, `set_transform`
- [ ] Clip: intersect with new `Path`
- [ ] Immediate-mode drawing API (`fill_path`, `stroke_path`, `draw_glyphs`, `draw_image`)
- [ ] Retained mode: record draw calls into a display list; replay to any backend; serialize
### Backend trait (Module D)
- [ ] `CanvasBackend` trait (`fill_path`, `stroke_path`, `draw_glyphs`, `draw_image`, `push/pop_clip`, `push/pop_transform`)
- [ ] Mock/recording backend for tests
### RasterBackend
- [ ] Rasterizer stub: scanline AA coverage, fill rules
- [ ] Gradient + solid shading, alpha compositing
- [ ] Clip masks
- [ ] Stroke rendering via stroker
- [ ] Image/pattern sampling
- [ ] Glyph drawing via positioned runs
- [ ] **Milestone:** complex 2D scene anti-aliased to an image buffer

## Phase 3 — Vector Backends: PDF, SVG (Weeks 7–9)
### SvgBackend
- [ ] Path → `d="..."` string (curves not flattened; deterministic number formatting)
- [ ] Fill/stroke attrs, fill-rule, dashes, caps/joins, miter limit
- [ ] Linear/radial gradient `<defs>`, patterns
- [ ] Clip paths, transforms (`<g>`), opacity
- [ ] Glyphs, images
### PdfBackend
- [ ] Content stream writer: `m`, `l`, `c`, `h`, `re`, `f`/`f*`, `S`, `W n`, `q`/`Q`, `cm`
- [ ] Quad → cubic conversion for PDF
- [ ] Stroke state operators (`w`, `J`, `j`, `M`, `d`)
- [ ] Shading dictionaries for gradients (`sh`), patterns
- [ ] Alpha via ExtGState
- [ ] Glyph + image XObject emission (interface to tpt-doc)
- [ ] Output a valid minimal PDF for testing; validate with external viewer/qpdf
- [ ] **Milestone:** tpt-doc (stub consumer) generates vector graphics in a PDF

## Phase 4 — WebGPU, Canvas2D & Clipping (Weeks 10–12)
### Tessellation
- [ ] Polygon tessellator (earcut or monotone) with NonZero/EvenOdd handling
- [ ] Robustness: self-intersections, zero-area triangles, duplicate points, holes
- [ ] Stroke tessellation (via stroker)
- [ ] Gradient UV / uniform generation
### WebGpuBackend
- [ ] Vertex/index buffer + draw-command list output (headless, no GPU needed)
- [ ] Stencil-buffer clip system (push/pop nesting)
- [ ] Thin `wgpu` adapter crate: pipelines, shaders (WGSL), submit
- [ ] Glyph + image draws
### Canvas2dBackend (added)
- [ ] `web-sys`/`wasm-bindgen` adapter mapping trait calls to Canvas2D
- [ ] Browser test (wasm-bindgen-test / headless)

## Testing & Quality (continuous)
- [ ] Golden-file SVG/PDF scenes (translucent gradients, dashed strokes, complex clips)
- [ ] Tessellation fuzzing (millions of random/malformed paths; no panics)
- [ ] Transform property tests (inverse/identity, deep state stacks)
- [ ] Flatten/arc property tests and cross-platform determinism checks
- [ ] Recording-backend equivalence: immediate mode vs. retained replay produce same output
- [ ] `no_std` build verified in CI (wasm32 + a bare-metal target)
- [ ] Verify zero C-FFI (dependency audit, `cargo deny`/`tree`)

## Release — v1.0.0
- [ ] Public API review and doc comments on all public items
- [ ] Examples (rounded gradient button pipeline from spec §4, pie chart, gear)
- [ ] README + architecture doc + stub-swap guide
- [ ] Changelog, versioning, publish checklist
- [ ] Tag v1.0.0

## Open items to revisit
- [ ] Gradient and pattern details (spec elides `{ ... }` fields)
- [ ] Swap stubs for real `tpt-*` crates once available
- [ ] Canvas2D scope (added by request; not in spec Module D) — update spec if kept
