# tpt-canvas

A deterministic, pure-Rust 2D vector graphics API. Paths, styles and a drawing context, written once and sent to any backend.

`tpt-canvas` is the core of the tpt-canvas project. It contains:

- **Paths**: lines, quadratic and cubic Béziers, and arcs in three forms (HTML5 `arc`, HTML5 `arcTo` and SVG endpoint arcs). Arcs are converted to cubics when a path is built.
- **Flattening** with a guaranteed maximum deviation, plus exact bounding boxes and a compact binary path format.
- **A stroker** with caps, joins, miter limits and dash patterns.
- **Styles**: solid, linear and radial gradients (pad, repeat and reflect spreads; sRGB or linear-light interpolation) and fill rules.
- **`Context`**: a state stack with transforms, clips and immediate-mode drawing, which records into a backend.
- **`DisplayList`**: retained mode. It records calls, replays them into any backend, and serializes to bytes.
- **Tessellation**: ear clipping for contours with holes, and fan triangulation for winding-exact fills.
- **Glyph seam**: `GlyphSource` turns each glyph of a shaped run into a path. Backends then draw text without knowing about fonts.

The crate is `no_std` with `alloc`. Numerics go through `libm`, so output is bit-identical on native and `wasm32` targets.

Backends live in separate crates: `tpt-canvas-svg`, `tpt-canvas-pdf`, `tpt-canvas-raster`, `tpt-canvas-webgpu` (headless), `tpt-canvas-wgpu` and `tpt-canvas-canvas2d`.

## Example

```rust
use tpt_canvas::{Color, Context, Paint, PathBuilder};
use tpt_canvas_svg::SvgBackend;

let mut ctx = Context::new(SvgBackend::new(200.0, 100.0));
let mut b = PathBuilder::new();
b.rounded_rect(10.0, 10.0, 180.0, 80.0, 12.0);
ctx.set_fill_paint(Paint::Solid(Color::rgb(0.2, 0.4, 0.9)));
ctx.fill_path(&b.build());
let svg = ctx.into_backend().finish();
```

## Status

Pre-1.0. The API may still change. Images, pattern paints, and real font shaping are not part of this crate. Shaping belongs to `tpt-text`, which is not yet published.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
