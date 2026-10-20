# tpt-canvas-pdf

PDF output backend for tpt-canvas. Vector content streams with native curves, shadings and alpha.

A PDF backend for [`tpt-canvas`](https://crates.io/crates/tpt-canvas). It writes a single-page PDF with a content stream that keeps every curve as a Bézier, so output stays small and scales cleanly.

- Solid fills and strokes use `rg`/`RG`, with alpha through an ExtGState.
- Gradients are shading dictionaries (`sh`) with Type 2 or stitched Type 3 functions. A gradient stroke clips to its outline and then shades.
- The file has a valid xref table. Its offsets are checked in the test suite.

## Example

```rust
use tpt_canvas::{Color, Context, Paint, PathBuilder};
use tpt_canvas_pdf::PdfBackend;

let mut ctx = Context::new(PdfBackend::new(200.0, 100.0));
let mut b = PathBuilder::new();
b.rect(10.0, 10.0, 180.0, 80.0);
ctx.set_fill_paint(Paint::Solid(Color::rgb(0.2, 0.6, 0.3)));
ctx.fill_path(&b.build());
std::fs::write("out.pdf", ctx.into_backend().finish()).unwrap();
```

## Limitations

- `Repeat` and `Reflect` gradient spreads are drawn as `Pad`.
- Per-stop alpha in gradients is ignored.
- Pattern paints, glyphs and images are not written yet. Glyphs arrive as paths through `GlyphSource`.
- Output has not yet been opened in a PDF viewer such as qpdf or Acrobat. The structural checks are in the test suite.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
