# tpt-color-stub

Stand-in for `tpt-color`: colours, gradient stop interpolation and source-over compositing.

A minimal stand-in for the `tpt-color` crate, which does not exist yet. It provides sRGB-encoded `Color` values with straight alpha, `ColorSpace` (sRGB or linear-light), gradient `Stop`s, `interpolate` and `over`.

Replace this crate with `tpt-color` once it is published. See the swap guide in the project repository.

## Example

```rust
use tpt_color_stub::{interpolate, Color, ColorSpace, Stop};

let stops = [
    Stop { offset: 0.0, color: Color::BLACK },
    Stop { offset: 1.0, color: Color::WHITE },
];
let mid = interpolate(&stops, 0.5, ColorSpace::Srgb);
```

## Status

Stub. Not intended for use outside tpt-canvas.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
