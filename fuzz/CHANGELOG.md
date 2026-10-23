# Changelog

All notable changes to `tpt-canvas-fuzz` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Added
- `svg_path` and `path_deserialize` fuzz targets. The targets compile; the fuzzer has not been run locally.
- Nightly `cargo-fuzz` job (`.github/workflows/fuzz.yml`) runs both targets.
