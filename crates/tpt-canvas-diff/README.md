# tpt-canvas-diff

Cross-backend comparison for tpt-canvas. It draws one scene through the raster backend and the WebGPU reference executor, then reports where the two disagree.

The raster backend is the reference for anti-aliased output. The WebGPU executor samples coverage at points, so edge pixels differ by design. The result is therefore a set of measurements (mismatched coverage count, largest and mean channel delta) rather than a pass or fail.

This crate is not published. It is a development tool for the workspace.

## Example

```rust
use tpt_canvas::{Color, FillRule, Paint, PathBuilder};
use tpt_canvas_diff::diff_raster_and_webgpu;

let diff = diff_raster_and_webgpu(64, 64, |backend| {
    let mut b = PathBuilder::new();
    b.circle(32.0, 32.0, 20.0);
    backend.fill_path(&b.build(), &Paint::Solid(Color::BLACK), FillRule::NonZero);
});
println!("{} of {} pixels disagree on coverage", diff.coverage_mismatch, diff.pixels);
```

`render_raster_and_webgpu` returns both pixel buffers if you want to inspect them directly.

## Limitations

- SVG and PDF output cannot be rasterized inside this workspace, so they are not compared.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
