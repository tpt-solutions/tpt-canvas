# Swap guide: inlined stand-ins → real ecosystem crates

Until the ecosystem crates exist, tpt-canvas carries small stand-ins for them. They
live inside `tpt-canvas` as modules, so the public crates have no unpublished dependencies.
Each module keeps the names and semantics of the crate it stands in for, so a swap is
mostly a matter of changing the module's contents.

| Module | Stands in for | Provides | Swap notes |
|---|---|---|---|
| `tpt_canvas::math` | `tpt-math` | `Vec2`, `Rect`, `Affine2` | Composition is `a * b` = apply `b`, then `a`. Check that the real `Affine2` uses the same convention and field names (`a..f`). |
| `tpt_canvas::color` | `tpt-color` | `Color` (sRGB-encoded, straight alpha), `ColorSpace`, `Stop`, `interpolate`, `over`, `srgb_to_linear` | Interpolation spaces are `Srgb` and `LinearSrgb` only. If the real crate has more spaces, add variants to `ColorSpace` and extend `interpolate`. The helpers in `color::ColorExt` and `color::named` are tpt-canvas's own and stay put. |
| `tpt_canvas::text` | `tpt-text` | `ShapedRun`, `PositionedGlyph`, `GlyphId`, `GlyphSource` | Only glyph ids and origins are used. The real run type must expose positioned glyphs relative to the run origin. `GlyphSource` is tpt-canvas's own seam. |
| `tpt_canvas::raster` | `tpt-raster` | `Rasterizer` trait, `ScanlineRasterizer`, `Coverage` | Implement `Rasterizer` for the real rasterizer. The trait takes closed polygons and returns an 8-bit coverage mask. |
| `tpt_canvas::testkit` | `tpt-testkit` | `Rng`, `fuzz`, `check_golden` | Used only by tests. `check_golden` needs the `testkit` feature (std). Replace it with the real crate's golden and fuzz helpers. |
| `tpt_canvas::image` | `tpt-kinetix` (decoding) | `ImageId`, `ImageRef` | Decoding is out of scope. Image bytes will come from `tpt-kinetix` as a separate resource type. |

Each stand-in module is marked `#[allow(missing_docs)]` in `crates/tpt-canvas/src/lib.rs`,
because it is expected to be replaced. Documenting it is a separate task.

## Swap procedure

1. Replace the module's contents with the real crate's types, or add a dependency on the
   real crate and re-export its types from the same module path. Keep the module path, so
   downstream code does not change.
2. Run `cargo test --workspace`. The golden files in `crates/*/tests/golden/` will show
   any output change caused by math or formatting differences. Regenerate them only
   after you have checked the diff.
3. Run the `no_std` build checks for `wasm32-unknown-unknown` and `thumbv7em-none-eabihf`
   (see `.github/workflows/ci.yml`). Real crates must also be `no_std + alloc`.

## Known differences to resolve when swapping

- Golden SVG and PDF files store output produced with the stand-in math and formatting.
  Their bytes will change if the real `tpt-math` rounds differently.
- The stand-ins use `libm` for transcendental functions. Keep that choice in the real crates,
  or cross-platform determinism is lost.
