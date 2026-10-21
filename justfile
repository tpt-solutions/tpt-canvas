# Developer tasks. Install `just` (https://github.com/casey/just) to use them.
# Each recipe is a plain cargo command, so `cargo` works the same way without `just`.

default:
    @just --list

# Format check, clippy, tests and rustdoc, as in CI's `check` job.
check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Run the workspace tests.
test:
    cargo test --workspace

# Rewrite the golden SVG and PDF files. Review the diff before committing.
golden:
    TPT_UPDATE_GOLDEN=1 cargo test -p tpt-canvas-svg -p tpt-canvas-pdf

# Run the million-path tessellation fuzz test (ignored by default; use release mode).
fuzz-long:
    cargo test --release -p tpt-canvas --test tessellate -- --ignored

# Build the no_std crates for wasm and bare metal, and the Canvas2D backend for wasm.
wasm:
    cargo build --target wasm32-unknown-unknown -p tpt-canvas
    cargo build --target thumbv7em-none-eabihf -p tpt-canvas
    cargo build --target wasm32-unknown-unknown -p tpt-canvas-canvas2d

# Run the gallery examples. Outputs go to gallery-out/.
examples:
    cargo run -p tpt-gallery --example all_backends
    cargo run -p tpt-gallery --example analog_clock
    cargo run -p tpt-gallery --example bar_chart
    cargo run -p tpt-gallery --example pdf_table
    cargo run -p tpt-gallery --example hero
    cargo run -p tpt-gallery --example labels
