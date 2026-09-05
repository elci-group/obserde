//! Golden fixture test: loads `fixtures/contract-example/` and asserts
//! `canonicalize`/`document_hash` reproduce the checked-in derived files.
//! Regenerate those derived files with:
//! `cargo run -p obserde-canonical --example gen_fixtures`

use std::path::PathBuf;

use obserde_canonical::{canonicalize, document_hash, Hash};
use obserde_schema::Schema;
use obserde_value::Document;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/contract-example")
}

#[test]
fn canonical_and_hash_match_the_real_implementation() {
    let dir = fixtures_dir();

    let schema: Schema =
        serde_json::from_str(&std::fs::read_to_string(dir.join("schema.json")).unwrap()).unwrap();
    let valid: Document =
        serde_json::from_str(&std::fs::read_to_string(dir.join("valid.json")).unwrap()).unwrap();
    let expected_canonical: Document =
        serde_json::from_str(&std::fs::read_to_string(dir.join("canonical.json")).unwrap()).unwrap();
    let expected_hash_hex = std::fs::read_to_string(dir.join("hash.txt")).unwrap();
    let expected_hash = Hash::from_hex(expected_hash_hex.trim()).unwrap();

    let actual_canonical = canonicalize(&schema, &valid).unwrap();
    assert_eq!(actual_canonical, expected_canonical);

    let actual_hash = document_hash(&actual_canonical);
    assert_eq!(actual_hash, expected_hash);
}

#[test]
fn canonicalize_is_idempotent_on_the_golden_fixture() {
    let dir = fixtures_dir();
    let schema: Schema =
        serde_json::from_str(&std::fs::read_to_string(dir.join("schema.json")).unwrap()).unwrap();
    let valid: Document =
        serde_json::from_str(&std::fs::read_to_string(dir.join("valid.json")).unwrap()).unwrap();

    let once = canonicalize(&schema, &valid).unwrap();
    let twice = canonicalize(&schema, &once).unwrap();
    assert_eq!(once, twice);
}
