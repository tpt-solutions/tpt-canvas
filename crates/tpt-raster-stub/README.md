# tpt-raster-stub

Stand-in for `tpt-raster`: a scanline anti-aliased coverage rasterizer behind a trait.

A minimal stand-in for the `tpt-raster` crate, which does not exist yet. It provides a `Rasterizer` trait, a `ScanlineRasterizer` that samples several sub-scanlines per pixel, and `Coverage` masks.

Replace this crate with `tpt-raster` once it is published. See the swap guide in the project repository.

## Example

```rust
use tpt_math_stub::Vec2;
use tpt_raster_stub::{Rasterizer, ScanlineRasterizer};

let square = vec![vec![
    Vec2::new(1.0, 1.0), Vec2::new(3.0, 1.0), Vec2::new(3.0, 3.0), Vec2::new(1.0, 3.0),
]];
let cov = ScanlineRasterizer::default().rasterize(&square, false, 5, 5);
assert_eq!(cov.at(2, 2), 255);
```

## Status

Stub. Not intended for use outside tpt-canvas.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
