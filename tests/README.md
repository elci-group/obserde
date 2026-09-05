# tests/

The root `Cargo.toml` is a virtual workspace manifest, so there is no root-level
test target to compile here. Integration and golden-fixture tests live inside
the crates that own the behavior being tested (e.g.
`crates/obserde-canonical/tests/`). Run the full suite with:

```
cargo test --workspace
```

This directory exists to keep the repository layout described in the Obserde
architectural directive visible end-to-end, even where a directory currently
has no content of its own.
