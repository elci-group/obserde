//! The directive's explicit Phase 2 demonstration: schema → validation →
//! canonicalisation → encoding → decoding → hashing, using the shared
//! `fixtures/contract-example/` fixture.

use std::path::PathBuf;

use obserde_canonical::{canonicalize, document_hash};
use obserde_json::{decode, encode};
use obserde_schema::Schema;
use obserde_validate::validate;
use obserde_value::Document;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/contract-example")
}

fn load_schema() -> Schema {
    serde_json::from_str(&std::fs::read_to_string(fixtures_dir().join("schema.json")).unwrap()).unwrap()
}

fn load_document(name: &str) -> Document {
    serde_json::from_str(&std::fs::read_to_string(fixtures_dir().join(name)).unwrap()).unwrap()
}

/// Removes the fixture's one `Bytes`-typed field before the strict
/// identity assertions below. `signature` deliberately does not survive
/// a schema-agnostic decode unchanged (see
/// `bytes_field_does_not_round_trip_through_schema_agnostic_decode`) —
/// this helper keeps that already-documented, already-separately-tested
/// limitation from making the *rest* of the pipeline's round-trip
/// identity look broken when it isn't.
fn without_signature(doc: &Document) -> Document {
    match doc {
        Document::Map(entries) => {
            Document::Map(entries.iter().filter(|(k, _)| k != "signature").cloned().collect())
        }
        other => other.clone(),
    }
}

#[test]
fn schema_validate_canonicalize_encode_decode_hash() {
    let schema = load_schema();
    let original = load_document("valid.json");

    // schema -> validation
    let validation = validate(&schema, &original).unwrap();
    assert!(validation.is_valid(), "unexpected issues: {:?}", validation.issues());

    // -> canonicalisation
    let canonical = canonicalize(&schema, &original).unwrap();
    let canonical_without_bytes = without_signature(&canonical);
    let expected_hash = document_hash(&canonical_without_bytes);

    // -> encoding
    let wire = encode(&canonical).unwrap();

    // -> decoding
    let decoded = decode(&wire).unwrap();

    // -> hashing: the round trip through real JSON text preserves the
    // canonical document's identity, for every field except the one
    // documented Bytes exception stripped above.
    let recanonicalized = canonicalize(&schema, &decoded).unwrap();
    assert_eq!(without_signature(&recanonicalized), canonical_without_bytes);
    assert_eq!(document_hash(&without_signature(&recanonicalized)), expected_hash);
}

#[test]
fn bytes_field_does_not_round_trip_through_schema_agnostic_decode() {
    // Documents the deliberate, schema-agnostic-JSON limitation described
    // on `obserde_json::decode`: a `Document::Bytes` field becomes a
    // base64 `Document::String` after a bare decode, not `Bytes` again —
    // only a schema-aware caller (which this test also demonstrates) can
    // tell the two apart.
    let schema = load_schema();
    let original = load_document("valid.json");
    let canonical = canonicalize(&schema, &original).unwrap();

    let original_signature = canonical.get("signature").unwrap();
    assert!(matches!(original_signature, Document::Bytes(_)));

    let wire = encode(&canonical).unwrap();
    let decoded = decode(&wire).unwrap();
    let decoded_signature = decoded.get("signature").unwrap();
    assert!(matches!(decoded_signature, Document::String(_)));

    // The schema still knows "signature" is declared Bytes, even though
    // the schema-agnostic Document does not — validating the decoded
    // document against the schema surfaces the mismatch explicitly rather
    // than silently accepting a String where Bytes was declared.
    let validation = validate(&schema, &decoded).unwrap();
    assert!(!validation.is_valid());
    let issue = validation
        .issues()
        .iter()
        .find(|i| i.path.to_string() == ".signature")
        .unwrap();
    assert_eq!(issue.code, "validate.type.mismatch");
}
