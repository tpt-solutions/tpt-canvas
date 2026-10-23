# tpt-canvas-fuzz

Fuzz targets for tpt-canvas, built with [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) and libFuzzer. Each target checks that malformed input returns an error rather than panicking.

| Target | Input | Checks |
|---|---|---|
| `svg_path` | UTF-8 text | `Path::from_svg_d` never panics |
| `path_deserialize` | Raw bytes | `Path::deserialize` never panics, and any path it decodes round-trips through `serialize` |

This crate is not published. It sits outside the root workspace because cargo-fuzz builds it separately.

## Running

```sh
cargo install cargo-fuzz
cd fuzz
cargo +nightly fuzz run svg_path
cargo +nightly fuzz run path_deserialize
```

A nightly toolchain is required by libFuzzer. The `fuzz.yml` workflow runs both targets daily for 10 minutes each. The million-path stress test is separate; it is `--ignored` and runs through `just fuzz-long`.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
