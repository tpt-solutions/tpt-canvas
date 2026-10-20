# tpt-testkit-stub

Stand-in for `tpt-testkit`: golden-file checks and deterministic random inputs for tests.

A test helper crate standing in for `tpt-testkit`, which does not exist yet. It provides:

- `check_golden`: compares output with a golden file. Set `TPT_UPDATE_GOLDEN=1` to rewrite the file.
- `Rng`: a SplitMix64 generator. It produces the same sequence on every platform.
- `fuzz`: runs a closure many times with seeded random input.

## Example

```rust
use tpt_testkit_stub::{fuzz, Rng};

fuzz(42, 1000, |rng: &mut Rng| {
    let x = rng.range(-1.0, 1.0);
    assert!(x >= -1.0 && x < 1.0);
});
```

## Status

Stub. Used only by tests in this workspace.

## License

Dual-licensed under [MIT](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-MIT) or [Apache-2.0](https://github.com/tpt-solutions/tpt-canvas/blob/master/LICENSE-APACHE), at your option.

Part of the [tpt-canvas](https://github.com/tpt-solutions/tpt-canvas) project.
