# tpt-canvas-canvas2d

Canvas2D backend for tpt-canvas in the browser. Maps drawing calls onto a CanvasRenderingContext2D.

A backend that draws [`tpt-canvas`](https://crates.io/crates/tpt-canvas) into a browser `CanvasRenderingContext2D`. It is only compiled for `wasm32-unknown-unknown`.

Transform and clip pushes become `save()` plus the operation, and pops become `restore()`, so the browser's own state stack stays balanced.

## Example

```rust,ignore
use tpt_canvas::{Color, Context, Paint, PathBuilder};
use tpt_canvas_canvas2d::Canvas2dBackend;

let ctx2d = /* a CanvasRenderingContext2d from a <canvas> element */;
let mut ctx = Context::new(Canvas2dBackend::new(ctx2d));
let mut b = PathBuilder::new();
b.circle(50.0, 50.0, 30.0);
ctx.set_fill_paint(Paint::Solid(Color::rgb(0.1, 0.5, 0.9)));
ctx.fill_path(&b.build());
```

## Testing

Browser tests read pixels back with `getImageData` and run in headless Chrome:

```sh
WASM_BINDGEN_USE_BROWSER=1 CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test -p tpt-canvas-canvas2d --target wasm32-unknown-unknown --test browser
```

## Limitations

- Canvas gradients support only pad spreading. Repeat and reflect are drawn as pad.
- Per-stop alpha is applied through the colour string.
- Pattern paints, glyphs and images are not drawn yet.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
