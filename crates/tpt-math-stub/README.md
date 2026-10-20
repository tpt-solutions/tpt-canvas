# tpt-math-stub

Stand-in for `tpt-math`: 2D vectors, rectangles and affine transforms for tpt-canvas.

A minimal stand-in for the `tpt-math` crate, which does not exist yet. It provides `Vec2`, `Rect` and `Affine2`. Composition follows `a * b`, which means "apply `b`, then `a`".

Once `tpt-math` is published, replace this crate and keep the same names. See the swap guide in the project repository.

## Example

```rust
use tpt_math_stub::{Affine2, Vec2};

let m = Affine2::translate(10.0, 0.0) * Affine2::scale(2.0, 2.0);
assert_eq!(m.apply(Vec2::new(1.0, 1.0)), Vec2::new(12.0, 2.0));
```

## Status

Stub. Not intended for use outside tpt-canvas.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
