# fixtures/

`contract-example/` is the one worked example referenced by
`obserde-canonical`'s golden test, `obserde-validate`'s fixture-backed
test, and `obserde-json`'s pipeline test — not a populated corpus. It
exercises every constraint kind named in Obserde's architectural directive
at once: `Pattern` (identifier grammar), `Length`, `Range`, `NonEmpty`, a
`Map` field type, a `Timestamp` field type, and (as of Phase 2) a `Bytes`
field type.

- `schema.json`, `valid.json`, `invalid.json` — hand-authored input,
  checked in as static files. These are the literal `serde::Serialize`
  output of `obserde_value::Document` (and `schema.json` of
  `obserde_schema::Schema`) via `serde_json` — Obserde's own internal,
  tagged representation of these types (e.g. `Document::Integer(5)`
  serializes as `{"integer":5}`), used for convenient Rust-fixture
  round-tripping in Phase 0/1's tests. This is **not** the JSON a real
  consumer would see on the wire.
- `canonical.json`, `hash.txt` — **derived**, not hand-written. Regenerate
  with `cargo run -p obserde-canonical --example gen_fixtures` after
  changing `schema.json`/`valid.json` or the canonicalisation rules.
- `wire.json` — **derived**, not hand-written, added in Phase 2.
  Regenerate with `cargo run -p obserde-json --example gen_wire_fixture`.
  Unlike the files above, this *is* real, ordinary JSON — the actual
  output of `obserde_json::encode` on the canonical form of `valid.json`
  — showing what a real consumer of this contract would actually receive.
