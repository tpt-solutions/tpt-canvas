# Changelog

All notable changes to `tpt-canvas-text` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Added
- New crate `tpt-canvas-text`: real text from [tpt-text](https://github.com/tpt-solutions/tpt-text), pinned to commit `dc8f578`.
- `TextRuns::from_layout` converts a tpt-text paragraph layout into `ShapedRun`s. `TextRuns::glyph_source` supplies outlines through `GlyphSource`, with outlines flipped to y up. `TextRuns::draw` draws every run, so raster and WebGPU both draw real fonts.
- Tests check glyph layout, outline orientation, wrapping, and that text lands inside its layout box on raster. The test font is Noto Sans Regular, under the SIL Open Font License.
- Not published: tpt-text is not on crates.io, and git dependencies cannot be published.
