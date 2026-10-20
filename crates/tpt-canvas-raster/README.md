# tpt-canvas-raster

CPU raster backend for tpt-canvas. Anti-aliased coverage and premultiplied RGBA compositing.

A CPU rasterizer for [`tpt-canvas`](https://crates.io/crates/tpt-canvas). Paths are flattened, rasterized with anti-aliasing, and composited into a premultiplied `f32` RGBA buffer. Clips are coverage masks.

Use it for thumbnails, tests and places where no GPU is available. It is also the reference for judging the GPU backends.

## Example

```rust
use tpt_canvas::{Color, Context, Paint, PathBuilder};
use tpt_canvas_raster::RasterBackend;

let mut ctx = Context::new(RasterBackend::new(64, 64));
let mut b = PathBuilder::new();
b.circle(32.0, 32.0, 20.0);
ctx.set_fill_paint(Paint::Solid(Color::BLACK));
ctx.fill_path(&b.build());
let pixels = ctx.into_backend().into_pixmap().to_rgba8();
```

## Limitations

- Pattern paints are ignored.
- Glyphs arrive as paths through `GlyphSource`. Images are not drawn yet.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
