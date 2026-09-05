//! Property test: `canonicalize(schema, canonicalize(schema, x)) ==
//! canonicalize(schema, x)` for arbitrary bounded `Document` trees, per
//! Obserde's formal invariant "a canonical document is idempotent".

use obserde_canonical::canonicalize;
use obserde_core::{Contract, SchemaVersion};
use obserde_schema::{Field, Schema, Type};
use obserde_value::Document;
use proptest::prelude::*;

fn fixture_schema() -> Schema {
    let contract = Contract::new("elci.test", "idempotency", SchemaVersion::new(1, 0, 0), 0).unwrap();
    let fields = vec![
        Field::new("b", Type::Integer).required(false),
        Field::new("a", Type::Integer).required(false),
    ];
    Schema::new(contract, fields).unwrap()
}

fn arb_document() -> impl Strategy<Value = Document> {
    let leaf = prop_oneof![
        Just(Document::Null),
        any::<bool>().prop_map(Document::Bool),
        any::<i64>().prop_map(Document::Integer),
        (-1000.0f64..1000.0f64).prop_map(Document::Float),
        "[a-z]{0,8}".prop_map(Document::String),
        proptest::collection::vec(any::<u8>(), 0..8).prop_map(Document::Bytes),
    ];
    leaf.prop_recursive(4, 64, 8, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..8).prop_map(Document::List),
            proptest::collection::vec(("[a-z]{1,6}", inner), 0..8).prop_map(Document::Map),
        ]
    })
}

proptest! {
    #[test]
    fn canonicalize_is_idempotent(doc in arb_document()) {
        let schema = fixture_schema();
        let once = canonicalize(&schema, &doc).unwrap();
        let twice = canonicalize(&schema, &once).unwrap();
        prop_assert_eq!(once, twice);
    }
}
