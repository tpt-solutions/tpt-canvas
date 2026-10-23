# tpt-canvas-gallery

Runnable examples for tpt-canvas. Each one draws a scene and writes the result to disk, so you can see what the library produces without writing any code first.

This crate is not published. It holds only examples.

## Examples

| Example | What it shows | Output |
|---|---|---|
| `hero` | Gradients, a blurred shadow and multiply blending. This is the picture in the top-level README. | `docs/images/hero.svg`, `hero.png` |
| `all_backends` | One drawing routine rendered to SVG, PDF and PNG | `gallery-out/` |
| `analog_clock` | A clock face at a fixed time (10:08:30), so output is the same on every run | `gallery-out/analog_clock.svg` |
| `bar_chart` | A bar chart with axes and gridlines | `gallery-out/bar_chart.svg` |
| `pdf_table` | A ruled invoice-style table. Text is not drawn yet, so only the layout appears. | `gallery-out/pdf_table.pdf` |
| `labels` | Text labels with the built-in stroke font | `gallery-out/` |
| `display_dump` | Prints each operation recorded into a `DisplayList` | stdout |
| `display_diff` | Diffs two recorded drawings op by op | stdout |

Run any of them from the workspace root:

```sh
cargo run -p tpt-canvas-gallery --example hero
```

The `just examples` recipe runs the main ones in turn. CI runs them too and uploads `gallery-out/` as an artifact.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
