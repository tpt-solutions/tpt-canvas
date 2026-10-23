# tpt-canvas-facade

One dependency for tpt-canvas. Turn on the backends you need as Cargo features, and the drawing API is re-exported at the root and in `prelude`.

This crate is not published. Until a release exists, depend on it by path or git.

## Features

| Feature | Enables | Default |
|---|---|---|
| `raster` | `tpt_canvas_facade::raster` (CPU raster backend) | yes |
| `svg` | `tpt_canvas_facade::svg` (SVG backend) | yes |
| `pdf` | `tpt_canvas_facade::pdf` (PDF backend) | yes |
| `webgpu` | WebGPU command-list backend and its CPU reference executor | no |
| `wgpu` | Real GPU execution through wgpu (implies `webgpu`) | no |
| `canvas2d` | Browser Canvas2D backend (wasm32 only; a no-op elsewhere) | no |

## Example

```toml
[dependencies]
tpt-canvas-facade = { path = "crates/tpt-canvas-facade", features = ["svg"] }
```

```rust
use tpt_canvas_facade::prelude::*;
use tpt_canvas_facade::svg::SvgBackend;

let mut ctx = Context::new(SvgBackend::new(100.0, 100.0));
ctx.set_fill_paint(Paint::Solid(Color::BLACK));
ctx.fill_rect(10.0, 10.0, 80.0, 80.0);
let svg = ctx.into_backend().finish();
```

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
