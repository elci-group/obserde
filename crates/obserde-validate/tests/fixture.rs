//! Exercises `validate()` against the shared `fixtures/contract-example/`
//! schema, covering both the "valid document satisfies its schema" and
//! the missing-field/out-of-range findings on the deliberately invalid
//! document.

use std::path::PathBuf;

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

#[test]
fn valid_fixture_satisfies_the_schema() {
    let schema = load_schema();
    let doc = load_document("valid.json");
    let result = validate(&schema, &doc).unwrap();
    assert!(result.is_valid(), "unexpected issues: {:?}", result.issues());
}

#[test]
fn invalid_fixture_reports_missing_field_and_range_violation() {
    let schema = load_schema();
    let doc = load_document("invalid.json");
    let result = validate(&schema, &doc).unwrap();
    assert!(!result.is_valid());

    let codes: Vec<&str> = result.issues().iter().map(|i| i.code.as_str()).collect();
    assert!(codes.contains(&"validate.field.missing"), "codes: {codes:?}");
    assert!(codes.contains(&"validate.constraint.range"), "codes: {codes:?}");

    let missing = result
        .issues()
        .iter()
        .find(|i| i.code == "validate.field.missing")
        .unwrap();
    assert_eq!(missing.path.to_string(), ".summary");

    let range = result
        .issues()
        .iter()
        .find(|i| i.code == "validate.constraint.range")
        .unwrap();
    assert_eq!(range.path.to_string(), ".score");
}
