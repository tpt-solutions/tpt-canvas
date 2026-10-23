# Changelog

All notable changes to `tpt-canvas-diff` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Added
- `diff_raster_and_webgpu` renders one scene on the raster backend and the WebGPU reference executor, and reports coverage mismatches and per-channel deltas.
- `render_raster_and_webgpu` returns both pixel buffers. `diff_pixels` compares any two buffers of equal length.
- SVG and PDF are not compared yet, because the workspace has no rasterizer for them.
