# Swap guide: stub crates → real ecosystem crates

Each stub keeps the names and semantics of the crate it stands in for, so swapping
is a dependency change plus any path or type adjustments listed here.

| Stub crate | Stands in for | Provides | Swap notes |
|---|---|---|---|
| `tpt-math-stub` | `tpt-math` | `Vec2`, `Rect`, `Affine2` | Composition is `a * b` = apply `b`, then `a`. Check that the real `Affine2` uses the same convention and field names (`a..f`). |
| `tpt-color-stub` | `tpt-color` | `Color` (sRGB-encoded, straight alpha), `ColorSpace`, `Stop`, `interpolate`, `over` | Interpolation spaces are `Srgb` and `LinearSrgb` only. If the real crate has more spaces, add variants to `ColorSpace` and extend `interpolate`. |
| `tpt-text-stub` | `tpt-text` | `ShapedRun`, `PositionedGlyph`, `GlyphId` | Only glyph ids and origins are used. The real run type must expose positioned glyphs relative to the run origin. |
| `tpt-raster-stub` | `tpt-raster` | `Rasterizer` trait, `ScanlineRasterizer`, `Coverage` | Implement `Rasterizer` for the real backend and pass it to `tpt-canvas-raster`. The trait takes closed polygons and returns an 8-bit coverage mask. |
| `tpt-testkit-stub` | `tpt-testkit` | `Rng`, `fuzz`, `check_golden` | Used only by tests. Replace with the real crate's golden and fuzz helpers. |
| `tpt-canvas::image` | `tpt-kinetix` (decoding) | `ImageId`, `ImageRef` | Decoding is out of scope. Image bytes will come from `tpt-kinetix` as a separate resource type. |

## Swap procedure

1. Add the real crate to `[workspace.dependencies]` in the root `Cargo.toml`.
2. Point the dependent crates at it, and remove the stub from `members` once no crate uses it.
3. Run `cargo test --workspace`. The golden files in `crates/*/tests/golden/` will show
   any output change caused by math or formatting differences. Regenerate them only
   after you have checked the diff.
4. Run the `no_std` build checks for `wasm32-unknown-unknown` and `thumbv7em-none-eabihf`
   (see `.github/workflows/ci.yml`). Real crates must also be `no_std + alloc`.

## Known differences to resolve when swapping

- Golden SVG and PDF files store output produced with the stub math and formatting.
  Their bytes will change if the real `tpt-math` rounds differently.
- The stubs use `libm` for transcendental functions. Keep that choice in the real crates,
  or cross-platform determinism is lost.
