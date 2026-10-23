# tpt-canvas-snapshot

Visual regression tests for tpt-canvas scenes. `assert_render!` draws a scene on the raster backend and compares the pixels with a stored golden file. The raster output is deterministic, so the comparison is byte-exact on every platform.

This crate is not published. It is a test helper for the workspace.

## Example

```rust
use tpt_canvas_snapshot::assert_render;

#[test]
fn button() {
    assert_render!(64, 32, |ctx| ctx.fill_rect(0.0, 0.0, 64.0, 32.0), "button");
}
```

The golden is read from `snapshots/button.rgba` in the calling crate. Its format is an 8-byte header (width and height as little-endian `u32`), then straight-alpha RGBA8 pixels.

To create or update goldens, run the tests with `TPT_UPDATE_GOLDEN=1`. Review the new files before committing them.

`render_bytes` returns the encoded golden for a scene without comparing it, which is useful for generating fixtures.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
