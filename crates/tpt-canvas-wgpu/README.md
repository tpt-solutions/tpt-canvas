# tpt-canvas-wgpu

wgpu adapter for tpt-canvas. Runs WebGPU draw lists on a real GPU, using stencil-then-cover and R8 clip layers.

A [wgpu](https://crates.io/crates/wgpu) adapter for the draw lists built by [`tpt-canvas-webgpu`](https://crates.io/crates/tpt-canvas-webgpu). It submits the stencil passes, cover passes and clip layers, then reads the frame back as premultiplied RGBA8.

## Example

```rust,no_run
use tpt_canvas::{Color, CanvasBackend, FillRule, Paint, PathBuilder};
use tpt_canvas_webgpu::WebGpuBackend;
use tpt_canvas_wgpu::GpuRenderer;

let mut b = PathBuilder::new();
b.circle(32.0, 32.0, 20.0);
let mut be = WebGpuBackend::new(64, 64);
be.fill_path(&b.build(), &Paint::Solid(Color::BLACK), FillRule::NonZero);
let renderer = GpuRenderer::new().expect("GPU adapter");
let frame = renderer.render(be.draw_list()).expect("render");
```

## Testing

The GPU parity tests compare the adapter with the CPU reference. They skip with a message when no adapter is available. Run them on a machine with a GPU.

## Limitations

- Pattern paints are rejected with an error. Glyphs and images are not drawn.
- Gradients are limited to 16 stops.
- The adapter renders into an RGBA8 target, so colour is quantised to 8 bits.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
