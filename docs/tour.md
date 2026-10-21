# A five-minute tour

This page covers the parts of tpt-canvas that are easy to get wrong. The README has the
overview, and `crates/tpt-gallery/examples/` has runnable code.

## One API, several outputs

Drawing code talks to a `Context`. The `Context` forwards each call to a backend:

```rust
use tpt_canvas::prelude::*;
use tpt_canvas_svg::SvgBackend;

let mut ctx = Context::new(SvgBackend::new(200.0, 100.0));
ctx.set_fill_paint(Paint::solid(named::BLUE));
ctx.fill_rect(10.0, 10.0, 80.0, 40.0);
let svg = ctx.into_backend().finish();
```

Swap `SvgBackend` for `PdfBackend`, `RasterBackend` or `DisplayList` and the same drawing
code produces that output. `DisplayList` records calls so they can be replayed later or
inspected (`crates/tpt-gallery/examples/display_dump.rs`).

## Coordinates and transform order

The y axis points down, as in SVG and canvas.

A new transform applies **before** the ones already set. After `translate(10, 0)` and
then `scale(2, 2)`, the point `(1, 0)` in user space is scaled first and translated
second, so it lands at `(12, 0)`. This is the same order as the HTML canvas.

`save` and `restore` cover the transform, clip, paint, fill rule, stroke style and
global alpha. A `restore` undoes the backend's clips and transforms in reverse push
order.

## Paths

Build paths with `PathBuilder`, or parse SVG path data:

```rust
use tpt_canvas::path;

let star = path! { M 0 -28 L 8 -9 L 27 -9 L 12 4 L 18 24 L 0 12 Z };
let arc = Path::from_svg_d("M0 0 A5 5 0 0 1 10 0")?;
```

`path!` parses at run time, so a bad literal panics the first time it runs. Use
`Path::from_svg_d` when the input comes from outside the program. It returns a
`SvgPathError` with the byte offset of the problem.

Arcs are lowered to cubic Béziers when the path is built. Backends never see an arc.

## Hit testing

```rust
let inside = path.contains_point(Vec2::new(5.0, 5.0), FillRule::NonZero);
let on_stroke = path.stroke_contains_point(Vec2::new(5.0, 5.0), &style);
```

Curves are flattened at a fixed tolerance (0.01 user units), so a point very close to an
edge can go either way.

## Colour and paint

`Color` is sRGB with straight alpha. Helpers from `ColorExt` (imported by the prelude)
add hex parsing, `mix`, `lighten` and `darken`. Named colours are in `named`.

Gradients are built with `Paint::linear` and `Paint::radial`, which pad outside their
range. Build a `Paint::LinearGradient` directly to pick `Repeat` or `Reflect`.

`Context::set_global_alpha` multiplies the alpha of every fill, stroke and glyph drawn
afterwards.

## Images and patterns

Raster, SVG, PDF and Canvas2D draw registered images. Register pixels, then draw:

```rust
use tpt_canvas::ImageId;
use tpt_canvas_raster::{Pixmap, RasterBackend};

let mut raster = RasterBackend::new(64, 64);
raster.add_image(ImageId(1), Pixmap::from_rgba8(2, 2, &rgba_bytes).unwrap());
```

Sampling is nearest-neighbour. A `Paint::Pattern` tiles the image, and `transform` maps
image pixels into user space.

To load a PNG, decode it with `Pixmap::from_png`, then register it the same way. The decoder
supports every PNG colour type and bit depth, with all five row filters, palette transparency
and CRC checks. It does not support interlaced (Adam7) files, and it keeps the high byte of
16-bit samples. Decode errors are a `DecodeError`.

```rust
let png = std::fs::read("logo.png")?;
let logo = Pixmap::from_png(&png)?;
raster.add_image(ImageId(2), logo);
```

## What each backend can't draw yet

| Call | Raster | SVG | PDF | Canvas2D | WebGPU |
|---|---|---|---|---|---|
| Solid and gradient fills and strokes | yes | yes | yes | yes | yes |
| Gradient `Repeat` / `Reflect` (linear) | yes | yes | tiled | counted, drawn as Pad | yes |
| Gradient `Repeat` / `Reflect` (radial) | yes | yes | counted, drawn as Pad | counted, drawn as Pad | yes |
| Gradient per-stop alpha | yes | yes | counted, ignored | yes | yes |
| Images (`add_image`, `draw_image`) | yes | yes (PNG data URI) | yes (image XObject) | yes | counted |
| Patterns | registered only | counted, placeholder | counted, placeholder | counted | counted |
| Glyph runs | via `GlyphSource` | via `GlyphSource` | via `GlyphSource` | via `GlyphSource` | via `GlyphSource` |
| Blend modes (`set_blend_mode`), fills and strokes | yes | yes (`mix-blend-mode`) | yes (`/BM`) | yes | counted |
| Drop shadows on fills and strokes (`set_shadow`), unblurred | yes | yes | yes | yes | yes |
| Drop shadows, blurred | yes | yes | yes (as an image) | yes | counted |

Without a `GlyphSource`, a glyph run is passed to the backend itself, which counts it.
"Counted" means the backend increments its `unsupported()` count, so you can check
after drawing. The count is zero when nothing was skipped or approximated. Raster
skips an image or pattern only when it was not registered, and counts that skip.

Shadows apply to fills and strokes. An unblurred shadow is a second fill or stroke,
so every backend draws it. A blurred shadow is drawn by the raster, SVG, PDF (as an image) and Canvas2D backends; the others
count it and leave it out, so check `unsupported()` if the shadow matters.

## Determinism

Flattening, stroking and formatting use fixed-order arithmetic and `libm`. The same
scene gives the same bytes on every run, and the CI also checks that native and wasm32
builds agree. Output does not depend on the system clock, thread scheduling or hash
iteration order.
