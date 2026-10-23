# Changelog

All notable changes to `tpt-canvas-playground` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Added
- A small scene language (`rect`, `circle`, `text`, colours, shadows, blend modes, transforms), parsed with line-numbered errors and drawn into any `Context`.
- Wasm entry point `draw_script`, which draws onto a canvas through the Canvas2D backend.
- Browser tests run in CI, and `docs/playground/index.html` is built with `just playground`.
