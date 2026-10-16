# Synthetic associative fixtures

`r2010-v2-inline-group.dwg` is generated entirely from hand-encoded synthetic
data by `examples/generate_r2010_inline_constraint_fixture.rs`. It contains a
line and an R2010 version-2 constraint group with two inline nodes (IDs 1 and
4, next ID 9), status bytes, a connection in each direction and an active
constraint. It contains no data copied from a user or third-party drawing.

Regenerate from the repository root:

```sh
cargo run --example generate_r2010_inline_constraint_fixture
```

The fixture and generator are covered by the repository's MPL-2.0 license.
