# Changelog

All notable changes to `tpt-canvas` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Added
- Paths, `PathBuilder`, arc conversion (HTML5 and SVG forms), flattening, serialization.
- Stroker with caps, joins, miter limit and dashes.
- Styles: `Paint` (solid and gradients), `FillRule`, `StrokeStyle`, `SpreadMode`.
- `Context` with state stack, transforms, clips and immediate drawing.
- `CanvasBackend` trait and `DisplayList` with replay and binary serialization.
- Ear-clipping and fan tessellators.
- `GlyphSource` seam for drawing glyph runs as paths.
