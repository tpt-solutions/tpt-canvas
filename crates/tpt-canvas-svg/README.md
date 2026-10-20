# tpt-canvas-svg

SVG output backend for tpt-canvas. Curves are kept as curves, and the output is byte-stable.

An SVG backend for [`tpt-canvas`](https://crates.io/crates/tpt-canvas). Curves are written as `C` and `Q` commands and are not flattened. Gradients use `userSpaceOnUse` in `<defs>`, clips become `<clipPath>` groups, and transforms become nested `<g>` elements.

Numbers are printed at fixed precision with trailing zeros removed, so the same drawing always produces the same bytes.

## Example

```rust
use tpt_canvas::{Color, Context, Paint, PathBuilder};
use tpt_canvas_svg::SvgBackend;

let mut ctx = Context::new(SvgBackend::new(200.0, 100.0));
let mut b = PathBuilder::new();
b.circle(100.0, 50.0, 40.0);
ctx.set_fill_paint(Paint::Solid(Color::rgb(0.9, 0.3, 0.2)));
ctx.fill_path(&b.build());
print!("{}", ctx.into_backend().finish());
```

## Limitations

- Pattern paints and glyph outlines are not yet written. Patterns emit a placeholder, and glyph runs arrive as paths through `GlyphSource`.
- Images are written as `<image>` references to `tpt-image:<id>`, because image bytes are not part of the drawing model yet.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
