//! Property test: `decode(encode(x)) == x`, scoped to the `Document`s
//! this crate documents as actually round-trippable through real JSON
//! text. Three deliberate exclusions from the generated strategy, each
//! also documented on `obserde_json::decode`:
//!
//! 1. **`Document::Bytes`** — always decodes back as `Document::String`
//!    (the base64 text), never `Bytes`; excluded from the leaf strategy
//!    entirely.
//! 2. **Duplicate map keys** — collapsed to last-value-wins already by
//!    `encode` (JSON objects are genuine maps); the map strategy below
//!    generates only unique keys.
//! 3. **Integers outside `i64`** — not reachable here at all, since
//!    `Document::Integer` is `i64`-typed by construction; noted for
//!    completeness, not because the generator needs special handling.

use obserde_json::{decode, encode};
use obserde_value::Document;
use proptest::prelude::*;
use std::collections::BTreeMap;

fn arb_document() -> impl Strategy<Value = Document> {
    let leaf = prop_oneof![
        Just(Document::Null),
        any::<bool>().prop_map(Document::Bool),
        any::<i64>().prop_map(Document::Integer),
        (-1000.0f64..1000.0f64).prop_map(Document::Float),
        "[a-z]{0,8}".prop_map(Document::String),
    ];
    leaf.prop_recursive(4, 64, 8, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..8).prop_map(Document::List),
            // BTreeMap<String, _> guarantees unique keys; converting to
            // Vec<(String, Document)> afterward keeps duplicate-key
            // documents out of the generated corpus entirely.
            proptest::collection::btree_map("[a-z]{1,6}", inner, 0..8)
                .prop_map(|m: BTreeMap<String, Document>| Document::Map(m.into_iter().collect())),
        ]
    })
}

proptest! {
    #[test]
    fn decode_of_encode_is_identity(doc in arb_document()) {
        let json = encode(&doc).unwrap();
        let decoded = decode(&json).unwrap();
        prop_assert_eq!(decoded, doc);
    }
}
