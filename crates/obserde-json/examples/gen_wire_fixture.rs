//! Generates `fixtures/contract-example/wire.json` from the real
//! `canonicalize`/`encode` implementation — the first fixture file that's
//! actually realistic JSON a real consumer would see, unlike
//! `valid.json`'s internal tagged `Document` shape (see
//! `fixtures/README.md`). Run with:
//! `cargo run -p obserde-json --example gen_wire_fixture`

use std::fs;
use std::path::PathBuf;

use obserde_canonical::canonicalize;
use obserde_json::encode;
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
    let wire = encode(&canonical).expect("encode canonical document as JSON");

    let pretty: serde_json::Value = serde_json::from_str(&wire).expect("re-parse encoded JSON for pretty-printing");
    let pretty_json = serde_json::to_string_pretty(&pretty).expect("pretty-print wire JSON");
    fs::write(dir.join("wire.json"), format!("{pretty_json}\n")).expect("write wire.json");

    println!("wrote wire.json");
}
