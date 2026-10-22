# tpt-canvas-text

Real text for [`tpt-canvas`](../tpt-canvas). [tpt-text](https://github.com/tpt-solutions/tpt-text)
shapes and lays out paragraphs, and this crate draws the result.

```rust,ignore
use tpt_canvas::{Color, Context, Paint};
use tpt_canvas_text::TextRuns;
use tpt_text::{layout_text, FontRegistry, Fx, TextStyle};

let registry = FontRegistry::new();
let face = registry.load_font(std::fs::read("NotoSans-Regular.ttf")?, 0)?;
let style = TextStyle::new(face.family(), Fx::from_int(24));
let layout = layout_text(&registry, "Hello, world", &style, Fx::from_int(300))?;

let runs = TextRuns::from_layout(&layout);
let mut ctx = Context::new(backend);
ctx.set_fill_paint(Paint::Solid(Color::BLACK));
runs.draw(&mut ctx);
```

- `TextRuns::from_layout` turns each run of each line into a `ShapedRun`. Glyph positions
  follow tpt-text's own SVG output: the pen starts at the line's offset, and each glyph sits at
  the pen plus its offset, on the baseline.
- `TextRuns::glyph_source` answers outline requests for the faces the runs use. Outlines are
  y up, as `GlyphSource` expects.
- `TextRuns::draw` installs that source and draws every run.

## Dependency

tpt-text is not on crates.io, so this crate depends on it from GitHub, pinned to commit
`dc8f578`. This crate is not published for the same reason.
