# Migrating from tiny-skia

This guide maps tiny-skia 0.12 onto tpt-canvas. It is for code that draws paths, fills,
strokes and gradients into a `Pixmap`.

Written for: developers who already render with tiny-skia and want the same drawing code
to also produce SVG, PDF or a Canvas2D drawing.

The snippets below were compiled and run in a scratch project against tiny-skia 0.12.0
and the local tpt-canvas crates. They are not part of the workspace.

## The main difference

tiny-skia is a CPU rasterizer. You call it on a `Pixmap`, and it writes pixels. tpt-canvas
is an API with several backends. The raster backend is the one that corresponds to
tiny-skia, and the same `Context` calls can go to SVG, PDF or the browser instead.

## Side by side

tiny-skia:

```rust
use tiny_skia::{Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap,
    Rect, Stroke, StrokeDash, Transform};

let mut pixmap = Pixmap::new(200, 100).expect("non-zero size");
let rect = PathBuilder::from_rect(Rect::from_xywh(10.0, 10.0, 80.0, 40.0).unwrap());
let mut paint = Paint::default();
paint.set_color_rgba8(51, 102, 230, 255);
paint.anti_alias = true;
pixmap.fill_path(&rect, &paint, FillRule::Winding, Transform::identity(), None);

let stroke = Stroke {
    width: 2.0,
    line_cap: LineCap::Round,
    line_join: LineJoin::Round,
    miter_limit: 4.0,
    dash: StrokeDash::new(vec![5.0, 4.0], 0.0),
};
pixmap.stroke_path(&curve, &paint, &stroke, Transform::identity(), None);
let png = pixmap.encode_png();
```

tpt-canvas:

```rust
use tpt_canvas::prelude::*;
use tpt_canvas_raster::RasterBackend;

let mut ctx = Context::new(RasterBackend::new(200, 100));
ctx.set_fill_paint(Paint::solid(Color::from_rgb8(51, 102, 230)));
ctx.fill_rect(10.0, 10.0, 80.0, 40.0);

ctx.set_stroke_paint(Paint::solid(Color::from_rgb8(51, 102, 230)));
ctx.set_stroke_style(StrokeStyle {
    width: 2.0,
    cap: LineCap::Round,
    join: LineJoin::Round,
    miter_limit: 4.0,
    dash: vec![5.0, 4.0],
    dash_offset: 0.0,
});
ctx.stroke_path(&curve);
let png: Vec<u8> = ctx.into_backend().into_pixmap().to_png();
```

## Call-by-call mapping

| tiny-skia | tpt-canvas |
|---|---|
| `Pixmap::new(w, h)` | `Context::new(RasterBackend::new(w, h))` |
| `pixmap.fill_path(&path, &paint, rule, transform, mask)` | `ctx.fill_path(&path)`, with the paint, rule and transform set on the context |
| `pixmap.stroke_path(&path, &paint, &stroke, transform, mask)` | `ctx.stroke_path(&path)`, with `set_stroke_paint` and `set_stroke_style` |
| `pixmap.fill_rect(rect, &paint, transform, mask)` | `ctx.fill_rect(x, y, w, h)` |
| `PathBuilder::new()` … `finish()` (returns `Option<Path>`) | `PathBuilder::new()` … `build()` (returns `Path`; an empty path is still a `Path`) |
| `PathBuilder::from_rect(rect)` | `PathBuilder::rect` on a builder, then `build` |
| `PathBuilder::from_circle(cx, cy, r)` | `PathBuilder::circle` on a builder, or `ctx.fill_circle` |
| `Paint::default()` with `set_color_rgba8(r, g, b, a)` | `Paint::solid(Color::rgba(...))`, or `Color::from_rgb8(r, g, b)` |
| `Color::from_rgba(r, g, b, a)` (returns `Option`) | `Color::rgba(r, g, b, a)` (floats 0 to 1, no `Option`) |
| `Stroke { width, line_cap, line_join, miter_limit, dash }` | `StrokeStyle { width, cap, join, miter_limit, dash, dash_offset }` |
| `LinearGradient::new(start, end, stops, mode, t)` | `Paint::linear(start, end, &[(offset, color), ...])`. The spread is `Pad`. |
| `RadialGradient::new(start, start_radius, end, end_radius, stops, mode, t)` | `Paint::radial(center, radius, &stops)`, which is one circle. A two-circle gradient, with different start and end circles, has no equivalent. |
| `Transform::from_translate(tx, ty)` | `Affine2::translate(tx, ty)` with `ctx.transform`, or `ctx.translate` |
| `Transform::from_rotate(deg)` (degrees) | `ctx.rotate(radians)`. Convert with `f32::to_radians`. |
| `Transform::from_scale(sx, sy)` | `ctx.scale(sx, sy)` |
| `paint.blend_mode = BlendMode::Multiply` | `ctx.set_blend_mode(BlendMode::Multiply)` |
| `mask` argument | `ctx.clip(&path)`, which intersects the clip for the following draws |
| `pixmap.encode_png()` (returns `Result`) | `pixmap.to_png()` (returns `Vec<u8>`; encoding does not fail) |
| `pixmap.pixels()` (premultiplied `u8`) | `pixmap.pixel(x, y)` (premultiplied `f32`, one pixel) or `pixmap.data` |
| `pixmap.data()` (premultiplied bytes) | `pixmap.to_rgba8()` (straight alpha, bytes) |

## Things that differ

- **Anti-aliasing.** tiny-skia has `paint.anti_alias`. The raster backend always
  anti-aliases, and there is no switch.
- **Fill rules.** tiny-skia's `Winding` is tpt-canvas's `NonZero`.
- **Blend modes.** tiny-skia has the Porter-Duff modes (`Source`, `DestinationIn`, `Clear`
  and so on) as well as the separable and non-separable ones. tpt-canvas has the twelve
  W3C blend modes, and no Porter-Duff modes. Use `clear_rect` to erase, and `clip` to
  restrict drawing.
- **Gradients.** tiny-skia's gradients take a spread mode and an extra transform. In
  tpt-canvas a gradient is a `Paint` variant, and the spread is `Pad` unless you construct
  the variant directly. Radial gradients are a single circle. tiny-skia takes a start circle and an end circle, which tpt-canvas cannot express.
- **Transforms.** tiny-skia passes a transform with every call. tpt-canvas keeps the
  transform in the context, so set it with `save`, `transform` and `restore`.
- **Masks.** tiny-skia's `Mask` is an 8-bit alpha buffer. tpt-canvas clips with paths,
  and it has no mask type.
- **Images.** tiny-skia draws `Pixmap` shaders. tpt-canvas draws images registered with
  `RasterBackend::add_image`, from a `Pixmap::from_rgba8`, with `draw_image` or an image
  pattern. There is no image decoder.
- **Text.** tiny-skia does not shape or draw text. tpt-canvas has `StrokeFont` for
  labels, and a `GlyphSource` seam for real fonts.
- **Output.** tiny-skia writes pixels and PNG. tpt-canvas also writes SVG, PDF and
  Canvas2D, from the same calls. Check `unsupported()` on the backend when a feature
  might not be drawn there.
- **Pixel readback.** tiny-skia stores premultiplied `u8`. tpt-canvas stores premultiplied
  `f32`, so reading values back gives more precision.

If something in your code has no row in the table above, look it up in the
[tour](tour.md) first, then in the API docs for `Context`.
