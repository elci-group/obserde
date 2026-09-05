//! Generates `fixtures/contract-example/canonical.json` and `hash.txt`
//! from the real `canonicalize`/`document_hash` implementation, reading
//! `schema.json` and `valid.json` (hand-authored) as input.
//!
//! `schema.json`, `valid.json`, and `invalid.json` are hand-authored and
//! checked in directly; only the two *derived* files are written here.
//! Run with: `cargo run -p obserde-canonical --example gen_fixtures`

use std::fs;
use std::path::PathBuf;

use obserde_canonical::{canonicalize, document_hash};
use obserde_schema::Schema;
use obserde_value::Document;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/contract-example")
}

fn main() {
    let dir = fixtures_dir();

    let schema_json = fs::read_to_string(dir.join("schema.json")).expect("read schema.json");
    let schema: Schema = serde_json::from_str(&schema_json).expect("parse schema.json");

    let valid_json = fs::read_to_string(dir.join("valid.json")).expect("read valid.json");
    let valid: Document = serde_json::from_str(&valid_json).expect("parse valid.json");

    let canonical = canonicalize(&schema, &valid).expect("canonicalize valid.json");
    let canonical_json = serde_json::to_string_pretty(&canonical).expect("serialize canonical form");
    fs::write(dir.join("canonical.json"), format!("{canonical_json}\n")).expect("write canonical.json");

    let hash = document_hash(&canonical);
    fs::write(dir.join("hash.txt"), format!("{}\n", hash.to_hex())).expect("write hash.txt");

    println!("wrote canonical.json and hash.txt ({})", hash.to_hex());
}
