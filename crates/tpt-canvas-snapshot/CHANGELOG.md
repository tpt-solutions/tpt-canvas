# Changelog

All notable changes to `tpt-canvas-snapshot` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Added
- `assert_render!(width, height, scene, "name")` renders a scene on the raster backend and compares it byte-exact with `snapshots/<name>.rgba`.
- `TPT_UPDATE_GOLDEN=1` writes the golden files.
- `render_bytes` returns the encoded golden for a scene.
