# tpt-canvas-playground

A browser playground for tpt-canvas. It parses a small scene language, one command per line, and draws the result onto a canvas element through the Canvas2D backend. The same parser and runner also work on any backend, so scripts can be tested natively.

This crate is not published.

## Scene language

Blank lines and lines starting with `#` are ignored.

| Command | Meaning |
|---|---|
| `fill-color #rrggbb` | Sets the fill colour |
| `stroke-color #rrggbb` | Sets the stroke colour |
| `line-width w` | Sets the stroke width |
| `alpha a` | Sets the global alpha, from 0 to 1 |
| `blend <mode>` | Sets the blend mode, for example `multiply` or `hue` |
| `shadow <blur> <dx> <dy> [#rrggbb]` / `shadow off` | Sets or removes a drop shadow |
| `rect x y w h`, `stroke-rect x y w h` | Fills or strokes a rectangle |
| `circle cx cy r` | Fills a circle |
| `text size x y words...` | Draws a label with the built-in stroke font |
| `translate x y`, `rotate radians`, `scale sx sy` | Changes the transform |
| `save`, `restore` | Pushes and pops the drawing state |

Parse errors report the 1-based line number.

## Example

```rust
use tpt_canvas::Context;
use tpt_canvas_playground::{parse, run};
use tpt_canvas_raster::RasterBackend;

let commands = parse("fill-color #3366cc\nrect 10 10 80 40\ncircle 60 80 20").unwrap();
let mut ctx = Context::new(RasterBackend::new(120, 120));
run(&mut ctx, &commands);
```

## Browser build

```sh
just playground
```

This builds the crate for `wasm32-unknown-unknown` and writes the bindings to `docs/playground/pkg`. It needs `wasm-bindgen-cli` 0.2.129. Open `docs/playground/index.html` to try it. The wasm entry point is `draw_script(canvas, script)`.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
