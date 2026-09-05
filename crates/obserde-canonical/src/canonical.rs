use std::cmp::Ordering;

use obserde_schema::Schema;
use obserde_value::Document;

use crate::error::CanonicalisationError;

/// Produces the canonical form of `doc` under `schema`.
///
/// Normalizes:
/// - **Map key order**: the top-level map's keys declared in `schema` come
///   first, in schema declaration order; any other keys (unknown fields,
///   or keys inside a nested map that has no per-key schema declaration)
///   are ordered lexicographically. Duplicate keys keep their relative
///   original order (stable sort).
/// - **Numeric representation**: `-0.0` collapses to `0.0`. `Integer`
///   values are left as-is (there is only one representation of an
///   `i64`).
/// - **Null vs. absent**: an explicit `Document::Null` entry is preserved
///   as-is; this function never adds or removes map entries, so the
///   distinction between "present and null" and "absent" is untouched.
/// - **List order**: preserved as-is — lists are ordered by definition.
///
/// String values (including ones that happen to hold a timestamp) are
/// passed through unchanged: real Unicode (NFC) normalization and
/// timestamp-to-UTC reformatting are deferred to whenever a real
/// date/time and Unicode-normalization dependency is introduced, rather
/// than hand-rolling that arithmetic here.
///
/// Idempotent: `canonicalize(schema, &canonicalize(schema, doc)?)? ==
/// canonicalize(schema, doc)?`.
pub fn canonicalize(schema: &Schema, doc: &Document) -> Result<Document, CanonicalisationError> {
    let top_level_order: Vec<&str> = schema.fields().iter().map(|f| f.name()).collect();
    Ok(canonicalize_value(doc, Some(&top_level_order)))
}

fn canonicalize_value(doc: &Document, field_order: Option<&[&str]>) -> Document {
    match doc {
        Document::Null => Document::Null,
        Document::Bool(b) => Document::Bool(*b),
        Document::Integer(i) => Document::Integer(*i),
        Document::Float(f) => Document::Float(normalize_float(*f)),
        Document::String(s) => Document::String(s.clone()),
        Document::Bytes(b) => Document::Bytes(b.clone()),
        Document::List(items) => {
            Document::List(items.iter().map(|item| canonicalize_value(item, None)).collect())
        }
        Document::Map(entries) => Document::Map(canonicalize_map(entries, field_order)),
    }
}

/// `-0.0 == 0.0` under IEEE 754 equality, but their bit patterns (and
/// hence their canonical hash) differ; collapsing `-0.0` to `0.0` here
/// keeps `document_hash` a function of logical value, not bit pattern.
/// `NaN` is passed through unchanged: there is no single canonical NaN
/// representation to collapse to without losing payload/signalling bits,
/// and Phase 1 has no requirement to canonicalize NaN specifically.
fn normalize_float(f: f64) -> f64 {
    if f == 0.0 {
        0.0
    } else {
        f
    }
}

fn canonicalize_map(entries: &[(String, Document)], field_order: Option<&[&str]>) -> Vec<(String, Document)> {
    let mut canon: Vec<(String, Document)> = entries
        .iter()
        .map(|(k, v)| (k.clone(), canonicalize_value(v, None)))
        .collect();

    canon.sort_by(|(a, _), (b, _)| compare_keys(a, b, field_order));
    canon
}

fn compare_keys(a: &str, b: &str, field_order: Option<&[&str]>) -> Ordering {
    match field_order {
        Some(order) => {
            let position = |k: &str| order.iter().position(|name| *name == k);
            match (position(a), position(b)) {
                (Some(ia), Some(ib)) => ia.cmp(&ib).then_with(|| a.cmp(b)),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => a.cmp(b),
            }
        }
        None => a.cmp(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use obserde_core::{Contract, SchemaVersion};
    use obserde_schema::{Field, Type};

    fn schema(field_names: &[&str]) -> Schema {
        let contract = Contract::new("elci.test", "fixture", SchemaVersion::new(1, 0, 0), 0).unwrap();
        let fields = field_names.iter().map(|n| Field::new(*n, Type::Integer)).collect();
        Schema::new(contract, fields).unwrap()
    }

    fn map(entries: Vec<(&str, Document)>) -> Document {
        Document::Map(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    #[test]
    fn declared_fields_ordered_before_unknown_extras() {
        let s = schema(&["b", "a"]);
        let doc = map(vec![
            ("extra", Document::Integer(0)),
            ("a", Document::Integer(1)),
            ("b", Document::Integer(2)),
        ]);
        let canon = canonicalize(&s, &doc).unwrap();
        let keys: Vec<&str> = canon
            .as_map()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(keys, vec!["b", "a", "extra"]);
    }

    #[test]
    fn nested_map_without_schema_is_ordered_lexicographically() {
        let s = schema(&["scores"]);
        let doc = map(vec![(
            "scores",
            map(vec![("bob", Document::Integer(1)), ("alice", Document::Integer(2))]),
        )]);
        let canon = canonicalize(&s, &doc).unwrap();
        let nested_keys: Vec<&str> = canon
            .get("scores")
            .unwrap()
            .as_map()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(nested_keys, vec!["alice", "bob"]);
    }

    #[test]
    fn negative_zero_normalizes_to_zero() {
        let s = schema(&[]);
        let canon = canonicalize(&s, &Document::Float(-0.0)).unwrap();
        assert_eq!(canon, Document::Float(0.0));
        match canon {
            Document::Float(f) => assert!(!f.is_sign_negative()),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn explicit_null_is_preserved_not_dropped() {
        let s = schema(&["note"]);
        let doc = map(vec![("note", Document::Null)]);
        let canon = canonicalize(&s, &doc).unwrap();
        assert_eq!(canon.get("note"), Some(&Document::Null));
    }

    #[test]
    fn list_order_is_preserved() {
        let s = schema(&[]);
        let doc = Document::List(vec![Document::Integer(3), Document::Integer(1), Document::Integer(2)]);
        let canon = canonicalize(&s, &doc).unwrap();
        assert_eq!(
            canon,
            Document::List(vec![Document::Integer(3), Document::Integer(1), Document::Integer(2)])
        );
    }

    #[test]
    fn canonicalize_is_idempotent_on_a_concrete_example() {
        let s = schema(&["b", "a"]);
        let doc = map(vec![
            ("extra", Document::Float(-0.0)),
            ("a", Document::Integer(1)),
            ("b", Document::Integer(2)),
        ]);
        let once = canonicalize(&s, &doc).unwrap();
        let twice = canonicalize(&s, &once).unwrap();
        assert_eq!(once, twice);
    }
}
