# Changelog

All notable changes to `tpt-canvas-facade` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Added
- `tpt-canvas-facade` crate with backend feature flags: `raster`, `svg`, `pdf`, `webgpu`, `wgpu` and `canvas2d`.
- Re-exports the drawing API at the root and in `prelude`, and each enabled backend under its module name.
