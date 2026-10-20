# tpt-canvas-webgpu

Headless WebGPU backend for tpt-canvas. Builds stencil-then-cover draw lists and includes a CPU executor, with no GPU needed.

The headless half of a WebGPU backend for [`tpt-canvas`](https://crates.io/crates/tpt-canvas). Drawing calls become a `DrawList` of triangles and commands, which a GPU adapter then submits. No GPU is needed to build a list.

- Fills use **stencil-then-cover**. Fan triangles accumulate a winding number per pixel, and a cover pass applies nonzero or even-odd. This is exact for self-intersecting and overlapping paths.
- Clips are mask layers that are AND-ed with their parent.
- `reference::execute` runs the same list on the CPU, so the algorithm can be checked without hardware.

For running the list on a GPU, use [`tpt-canvas-wgpu`](https://crates.io/crates/tpt-canvas-wgpu).

## Example

```rust
use tpt_canvas::{Color, CanvasBackend, FillRule, Paint, PathBuilder};
use tpt_canvas_webgpu::{reference::execute, WebGpuBackend};

let mut b = PathBuilder::new();
b.circle(32.0, 32.0, 20.0);
let mut be = WebGpuBackend::new(64, 64);
be.fill_path(&b.build(), &Paint::Solid(Color::BLACK), FillRule::NonZero);
let image = execute(be.draw_list());
```

## Limitations

- Pattern paints, glyphs and images are not supported. They are counted in `DrawList::unsupported`.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
