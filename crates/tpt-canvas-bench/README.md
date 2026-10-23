# tpt-canvas-bench

Criterion benchmarks for the tpt-canvas drawing pipeline. They time flattening, stroking, tessellation and raster fill, each on a star path whose size scales with its point count.

This crate is not published. It exists to catch performance regressions in the core crates.

## Running

```sh
cargo bench -p tpt-canvas-bench --bench pipeline
```

The `justfile` wraps the regression workflow:

```sh
just bench-save            # store timings as the "main" baseline
just bench-check main 0.25 # fail if a benchmark is more than 25% slower
```

`bench-check` calls `scripts/bench_compare.py`, which exits with code 2 when no baseline matches. CI runs each benchmark once with `cargo bench -p tpt-canvas-bench -- --test` (the `bench-smoke` job). That checks that they still compile and do not panic, but it does not check timings.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
