# tpt-canvas

A deterministic, pure-Rust 2D vector graphics API. One drawing API writes to several
backends: SVG, PDF, and a raster buffer. WebGPU and Canvas2D are planned.

- **`no_std` + `alloc`** core, so path math and backend traits run on WASM and embedded targets.
- **Zero C-FFI**. No crate compiles or links C. CI checks this. The one exception is `renderdoc-sys`, pure-Rust declarations that `wgpu-core` pulls in on native targets for the optional `tpt-canvas-wgpu` adapter.
- **Deterministic**. Flattening uses fixed-order arithmetic and `libm`. Output formatting uses fixed precision.
- **Dual mode**. Draw immediately through `Context`, or record into a `DisplayList` and replay later.

## Layout

| Crate | Role |
|---|---|
| `tpt-canvas` | Core: paths, arcs, flattening, stroker, styles, `Context`, `CanvasBackend`, `DisplayList` (`no_std`) |
| `tpt-canvas-svg` | SVG backend. Keeps curves and emits gradients, clips and transforms |
| `tpt-canvas-pdf` | PDF backend. Content streams, shadings, ExtGState alpha, xref |
| `tpt-canvas-raster` | Raster backend. Anti-aliased coverage, premultiplied RGBA f32 |
| `tpt-canvas-webgpu` | WebGPU backend, headless: stencil-then-cover command lists plus a CPU reference executor (no GPU dependency yet) |
| `tpt-canvas-canvas2d` | Canvas2D backend for the browser (wasm32 only) |
| `tpt-canvas-wgpu` | wgpu adapter: runs WebGPU draw lists on a real GPU (stencil-then-cover, R8 clip layers) |
| `tpt-math-stub`, `tpt-color-stub`, `tpt-text-stub`, `tpt-raster-stub`, `tpt-testkit-stub` | Stand-ins for ecosystem crates. See [docs/swap-guide.md](docs/swap-guide.md) |

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

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

Swap `SvgBackend` for `PdfBackend` or `RasterBackend` and the same drawing code produces that output.

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --target wasm32-unknown-unknown -p tpt-canvas
```

Regenerate golden files only after checking the diff:

```sh
TPT_UPDATE_GOLDEN=1 cargo test --workspace
```

## Status

See [TODO.md](TODO.md) for the phase checklist. Spec: [spec.txt](spec.txt).
