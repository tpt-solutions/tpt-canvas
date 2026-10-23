# Contributing to tpt-canvas

Thanks for looking at this. Contributions are handled through issues only: open an issue
to report a bug, ask a question, or propose a change. Pull requests are not accepted.

## Getting set up

The toolchain is pinned in `rust-toolchain.toml`. Build and test the whole workspace with:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

CI also checks these, so run them before you push:

- `no_std` builds for `wasm32-unknown-unknown` and `thumbv7em-none-eabihf`
- the Canvas2D backend for `wasm32`, and its browser tests (headless Chrome)
- cross-target determinism (wasm32 under Node)
- a dependency audit that rejects C-FFI crates (see the README for the two exceptions)

## Golden files

Some tests compare SVG and PDF output against files in `tests/golden/`. Regenerate them
only after you have read the diff:

```sh
TPT_UPDATE_GOLDEN=1 cargo test --workspace
git diff -- crates/*/tests/golden
```

A golden change in a pull request needs a sentence in the description saying why the
output changed.

## Rules the code follows

- **`tpt-canvas` is `no_std` + `alloc`.** Do not add `std` imports to the core crate or
  to any crate it depends on. Use `libm` for transcendental functions, so results match
  on every platform.
- **Determinism is a feature.** Do not introduce a source of nondeterminism (hash
  iteration order, threads, the system clock) into drawing output.
- **Unsupported calls are counted, not dropped silently.** When a backend cannot draw
  something yet, it must increment its `unsupported()` counter and add a test for it.
- **Public items get doc comments.** `#![warn(missing_docs)]` is set on the real crates.
  The stand-in modules (`math`, `color`, `text`, `raster`, `testkit`) are exempt for now.
- **No separate stub crates.** The stand-ins for ecosystem crates live inside `tpt-canvas`,
  so every public crate can be published without unpublished dependencies. See
  [docs/swap-guide.md](docs/swap-guide.md) for how they will be replaced.

## Adding a backend

Implement `tpt_canvas::CanvasBackend`. The contract is in the trait docs. The two
points people miss:

- `push_clip` and `push_transform` must be balanced by their `pop_*`. `Context` already
  pops in the right order, so a backend only has to undo the most recent push.
- Geometry arrives in user space. Keep your own transform stack; do not assume the
  device transform is identity.

Then add the backend's crate to the workspace, copy the structure of an existing
backend's tests (the unsupported-call test is a good template), and add it to the
README's layout table.

## Issues

- Search existing issues first to avoid duplicates.
- For a bug, include the steps to reproduce it, the expected and actual output, and your
  toolchain version and target.
- For a feature or behaviour change, describe the problem you are trying to solve, not only
  the solution you have in mind.

Any issue you open is accepted under the same terms as the project: MIT OR Apache-2.0.
