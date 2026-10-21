# Project templates

These are [cargo-generate](https://github.com/cargo-generate/cargo-generate) templates. Each
one is a small starting project for a common job.

| Template | What you get |
|---|---|
| `cli-renderer` | A command that renders one scene to `.png`, `.svg` or `.pdf`, chosen by extension. |
| `custom-backend` | A `CanvasBackend` skeleton that records calls and counts what it cannot draw, with tests. |
| `chart-library` | A bar chart that draws onto any backend, with tests. |
| `wasm-app` | A browser drawing on a canvas through the Canvas2D backend, with an `index.html`. |

Generate one with:

```sh
cargo generate --git https://github.com/tpt-solutions/tpt-canvas --subfolder templates/cli-renderer
```

or, from a checkout:

```sh
cargo generate --path templates/cli-renderer
```

The generated `Cargo.toml` depends on tpt-canvas through its git repository, because the
crates are not on crates.io yet. Inside a checkout, change those lines to path dependencies.
