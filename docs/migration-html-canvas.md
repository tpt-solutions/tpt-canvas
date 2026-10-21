# Migrating from HTML Canvas

This guide maps the `CanvasRenderingContext2D` API to tpt-canvas. It is for code that draws
shapes, paths and gradients. Text, shadows, blend modes and images loaded from URLs are
covered at the end, because they differ.

Written for: developers who already draw with `<canvas>` and want the same drawing code to
produce SVG, PDF or PNG output as well.

## The main difference

A canvas is a bitmap you draw on in place. A `Context` records each call and passes it to a
backend, and the backend decides the output. Pick the backend when you build the context:

```rust
use tpt_canvas::prelude::*;
use tpt_canvas_raster::RasterBackend;

let mut ctx = Context::new(RasterBackend::new(200, 100));
// ... draw ...
let pixmap = ctx.into_backend().into_pixmap();
let png: Vec<u8> = pixmap.to_png();
```

For the browser, `tpt_canvas_canvas2d::Canvas2dBackend::new(ctx2d)` wraps a real
`CanvasRenderingContext2d`, so the same `Context` code draws to a `<canvas>` element.

## Call-by-call mapping

| Canvas | tpt-canvas |
| --- | --- |
| `ctx.save()` / `ctx.restore()` | `ctx.save()` / `ctx.restore()` |
| `ctx.translate(x, y)` | `ctx.translate(x, y)` |
| `ctx.rotate(theta)` (radians) | `ctx.rotate(theta)` (radians) |
| `ctx.scale(sx, sy)` | `ctx.scale(sx, sy)` |
| `ctx.transform(a, b, c, d, e, f)` | `ctx.transform(Affine2 { a, b, c, d, e, f })` |
| `ctx.setTransform(a, b, c, d, e, f)` | `ctx.set_transform(Affine2 { a, b, c, d, e, f })` |
| `ctx.globalAlpha = a` | `ctx.set_global_alpha(a)` |
| `ctx.fillStyle = color` | `ctx.set_fill_paint(Paint::solid(color))` |
| `ctx.strokeStyle = color` | `ctx.set_stroke_paint(Paint::solid(color))` |
| `ctx.lineWidth`, `lineCap`, `lineJoin`, `miterLimit`, `setLineDash` | `ctx.set_stroke_style(StrokeStyle { .. })` |
| `ctx.fillRect(x, y, w, h)` | `ctx.fill_rect(x, y, w, h)` |
| `ctx.strokeRect(x, y, w, h)` | `ctx.stroke_rect(x, y, w, h)` |
| `ctx.clearRect(x, y, w, h)` | `ctx.clear_rect(x, y, w, h)` |
| `ctx.beginPath()` + path calls + `ctx.fill()` | build a `Path` with `PathBuilder`, then `ctx.fill_path(&path)` |
| `ctx.beginPath()` + path calls + `ctx.stroke()` | build a `Path`, then `ctx.stroke_path(&path)` |
| `ctx.clip()` | `ctx.clip(&path)` (the path is passed in, not taken from the current path) |
| `ctx.drawImage(img, sx, sy, sw, sh, dx, dy, dw, dh)` | `ctx.draw_image(&image, src_rect, dst_rect)` (raster only; see below) |
| `ctx.createLinearGradient(x0, y0, x1, y1)` + stops | `Paint::linear(Vec2::new(x0, y0), Vec2::new(x1, y1), &[(0.0, c0), (1.0, c1)])` |
| `ctx.createRadialGradient(...)` | `Paint::radial(center, radius, &stops)` (no focal point) |
| `ctx.getImageData(...)` | `pixmap.pixel(x, y)` on a raster backend (returns `[f32; 4]`) |

The current-path model is gone. Canvas keeps a hidden path that `moveTo`, `lineTo` and
`arc` extend. Here, a `PathBuilder` holds the path, and you pass it to a fill, stroke or
clip. This removes the `beginPath` step, and it means a path can be reused.

```rust
use tpt_canvas::prelude::*;

let mut b = PathBuilder::new();
b.move_to(10.0, 10.0);
b.line_to(90.0, 10.0);
b.arc(50.0, 50.0, 30.0, 0.0, core::f32::consts::PI, false);
b.close();
let path = b.build();

ctx.set_fill_paint(Paint::solid(named::BLUE));
ctx.fill_path(&path);
```

## Arcs and curves

`arc(cx, cy, r, start, end, ccw)` and `arc_to(x1, y1, x2, y2, r)` follow the canvas
definitions, including the counter-clockwise flag and sweeps larger than 2π. The rest of
the path API is `quad_to` and `cubic_to`, which take the same control points as canvas.
Arcs are converted to cubic Béziers when the path is built, so no backend sees an arc.

The SVG arc form (`A rx ry rot large sweep x y`) is available through
`PathBuilder::elliptical_arc`. If the path data is a string, `Path::from_svg_d` parses it.

## Coordinates

The y axis points down, as in canvas. The rule for transform order is also the same: a
transform applies before the transforms already set. See [the tour](tour.md#coordinates-and-transform-order).

## Gradients and colors

Colors are `Color` values. `Color::from_hex` and `Color::from_rgb8` parse the usual inputs,
and the named constants live in `named` (`named::BLUE`, and so on).

Gradient stops are `(offset, Color)` pairs, with offsets between 0 and 1. Canvas's `addColorStop`
call becomes an entry in the slice. Gradients take a `SpreadMode` for areas outside the
gradient, so `Pad` matches canvas.

## Things that differ

- **Text.** Canvas has `fillText` and `measureText`. tpt-canvas has no shaping yet. You
  build a `ShapedRun` of positioned glyphs and call `ctx.draw_glyphs(&run)`. Each glyph is
  turned into a path through the `GlyphSource` seam. Until a real font is wired in,
  text is not available in the examples.
- **Shadows and blur.** Canvas `shadowBlur`, `shadowColor` and friends are not implemented.
- **Blend modes.** `globalCompositeOperation` is not implemented. Group opacity is available
  through `push_layer(alpha)` and `pop_layer()`.
- **Images.** `drawImage` works on the raster backend only, for pixels you already have as
  a `Pixmap::from_rgba8`. There is no decoder, so there is no `Image` loaded from a URL.
  Other backends count the call in `unsupported()`.
- **Erasing.** `clearRect` works on the raster and Canvas2D backends. SVG, PDF and WebGPU
  cannot erase, and they count the call.
- **Hit testing.** Canvas `isPointInPath` maps to `path.contains_point(point, rule)`, and
  `isPointInStroke` maps to `path.stroke_contains_point(point, &style)`.
- **Output.** Canvas has no output format. Here, the backend decides it: PNG from
  `Pixmap::to_png`, SVG from `SvgBackend::finish`, or PDF from `PdfBackend::finish`.

Check `unsupported()` on any backend after a render. It is the count of calls that the
backend could not draw.
