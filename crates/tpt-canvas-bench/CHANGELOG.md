# Changelog

All notable changes to `tpt-canvas-bench` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Added
- Criterion benchmarks for flatten, stroke, tessellate and raster fill (`benches/pipeline.rs`).
- `bench-smoke` CI job runs each benchmark once, so they keep compiling.
- `just bench-save` and `just bench-check`, driven by `scripts/bench_compare.py`, to fail on regressions against a saved baseline.
