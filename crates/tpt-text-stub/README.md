# tpt-text-stub

Stand-in for `tpt-text`: shaped glyph runs for tpt-canvas. Text shaping itself is not included.

A minimal stand-in for the `tpt-text` crate, which does not exist yet. It provides `ShapedRun` and `PositionedGlyph`, the data that tpt-canvas draws. Shaping and font loading are not part of this crate.

Glyph outlines come from `tpt_canvas::GlyphSource`. A real `tpt-text` would implement that trait.

## Status

Stub. Not intended for use outside tpt-canvas.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
