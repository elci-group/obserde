use obserde_value::{Document, Path};
use serde_json::Value;

use crate::error::DecodeError;
use crate::limits::DecodeLimits;

/// Decodes JSON text into a `Document`, using `DecodeLimits::default()`.
/// See `decode_with_limits` for the full behavior and its documented
/// non-bijective corners against `encode`.
pub fn decode(json: &str) -> Result<Document, DecodeError> {
    decode_with_limits(json, &DecodeLimits::default())
}

/// Decodes JSON text into a `Document`, enforcing `limits`.
///
/// This is schema-agnostic: it has no way to know that a JSON string
/// should become a `Document::Bytes` rather than a `Document::String` —
/// that information only exists in a `Schema`, and taking one here would
/// re-introduce exactly the coupling `obserde-json` is designed to avoid
/// (see this crate's `lib.rs` doc comment). Concretely, `decode(encode(x))
/// == x` does not hold for every `Document`:
///
/// - **`Document::Bytes`** always decodes back as `Document::String` (the
///   base64 text), never `Bytes`.
/// - **Duplicate map keys** are already collapsed to last-value-wins by
///   `encode` itself (`serde_json::Map` is a genuine map and cannot
///   represent duplicates, even with `preserve_order`), so this isn't a
///   decode-specific asymmetry — a `Document::Map` with duplicate keys
///   never round-trips through JSON text in either direction.
/// - **Integer literals outside `i64`** are rejected explicitly (see
///   `DecodeError::IntegerOutOfRange`) rather than silently becoming an
///   approximate `Document::Float` — this only affects decoding
///   attacker/foreign-supplied JSON, not `Document`s that originated from
///   `Document::Integer(i64)` in the first place, which are always in
///   range by construction.
///
/// JSON numbers have one grammar; `Document` has two variants. This
/// function distinguishes them via `serde_json::Number::is_i64()` /
/// `is_u64()`, so `"5"` decodes to `Integer(5)` and `"5.0"` decodes to
/// `Float(5.0)` — matching `encode`'s choice to always print a decimal
/// point for `Document::Float`.
///
/// There is no decode-side mirror of `encode`'s `NonFiniteFloat` check:
/// JSON text has no lexical spelling for NaN/Infinity, and `serde_json`'s
/// own parser already rejects a numeric literal whose exponent would
/// overflow to infinity, surfacing as `DecodeError::Syntax` before this
/// function's own logic runs at all.
pub fn decode_with_limits(json: &str, limits: &DecodeLimits) -> Result<Document, DecodeError> {
    if json.len() > limits.max_input_bytes {
        return Err(DecodeError::InputTooLarge {
            limit: limits.max_input_bytes,
            actual: json.len(),
        });
    }

    let value: Value = serde_json::from_str(json).map_err(|e| DecodeError::Syntax {
        message: e.to_string(),
        line: e.line(),
        column: e.column(),
    })?;

    value_to_document(value, &Path::root(), 0, limits)
}

fn value_to_document(
    value: Value,
    path: &Path,
    depth: usize,
    limits: &DecodeLimits,
) -> Result<Document, DecodeError> {
    if depth > limits.max_depth {
        return Err(DecodeError::DepthExceeded {
            path: path.clone(),
            limit: limits.max_depth,
        });
    }

    match value {
        Value::Null => Ok(Document::Null),
        Value::Bool(b) => Ok(Document::Bool(b)),
        Value::Number(n) => {
            if n.is_i64() {
                Ok(Document::Integer(n.as_i64().expect("is_i64 implies as_i64 succeeds")))
            } else if n.is_u64() {
                Err(DecodeError::IntegerOutOfRange {
                    path: path.clone(),
                    literal: n.to_string(),
                })
            } else {
                Ok(Document::Float(n.as_f64().expect("non-integer Number is representable as f64")))
            }
        }
        Value::String(s) => {
            let len = s.chars().count();
            if len > limits.max_string_len {
                return Err(DecodeError::StringTooLong {
                    path: path.clone(),
                    limit: limits.max_string_len,
                    actual: len,
                });
            }
            Ok(Document::String(s))
        }
        Value::Array(items) => {
            if items.len() > limits.max_collection_len {
                return Err(DecodeError::CollectionTooLarge {
                    path: path.clone(),
                    limit: limits.max_collection_len,
                    actual: items.len(),
                });
            }
            let mut out = Vec::with_capacity(items.len());
            for (i, item) in items.into_iter().enumerate() {
                out.push(value_to_document(item, &path.index(i), depth + 1, limits)?);
            }
            Ok(Document::List(out))
        }
        Value::Object(entries) => {
            if entries.len() > limits.max_collection_len {
                return Err(DecodeError::CollectionTooLarge {
                    path: path.clone(),
                    limit: limits.max_collection_len,
                    actual: entries.len(),
                });
            }
            let mut out = Vec::with_capacity(entries.len());
            for (key, value) in entries.into_iter() {
                let child_path = path.field(&key);
                out.push((key, value_to_document(value, &child_path, depth + 1, limits)?));
            }
            Ok(Document::Map(out))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_all_primitive_variants() {
        assert_eq!(decode("null").unwrap(), Document::Null);
        assert_eq!(decode("true").unwrap(), Document::Bool(true));
        assert_eq!(decode("\"hi\"").unwrap(), Document::String("hi".to_string()));
    }

    #[test]
    fn integer_literal_decodes_as_integer() {
        assert_eq!(decode("5").unwrap(), Document::Integer(5));
        assert_eq!(decode("-5").unwrap(), Document::Integer(-5));
    }

    #[test]
    fn decimal_literal_decodes_as_float() {
        assert_eq!(decode("5.0").unwrap(), Document::Float(5.0));
    }

    #[test]
    fn decodes_list_and_map() {
        let doc = decode(r#"{"items":[1,2]}"#).unwrap();
        assert_eq!(
            doc,
            Document::Map(vec![(
                "items".to_string(),
                Document::List(vec![Document::Integer(1), Document::Integer(2)])
            )])
        );
    }

    #[test]
    fn integer_beyond_i64_max_is_rejected_not_downgraded() {
        let err = decode("9223372036854775808").unwrap_err();
        match err {
            DecodeError::IntegerOutOfRange { literal, .. } => {
                assert_eq!(literal, "9223372036854775808");
            }
            other => panic!("expected IntegerOutOfRange, got {other:?}"),
        }
    }

    #[test]
    fn malformed_json_is_a_syntax_error_with_location() {
        let err = decode("{not valid json").unwrap_err();
        match err {
            DecodeError::Syntax { line, column, .. } => {
                assert_eq!(line, 1);
                assert!(column > 0);
            }
            other => panic!("expected Syntax, got {other:?}"),
        }
    }

    #[test]
    fn input_too_large_is_rejected_before_parsing() {
        let limits = DecodeLimits {
            max_input_bytes: 4,
            ..DecodeLimits::default()
        };
        let err = decode_with_limits("123456", &limits).unwrap_err();
        match err {
            DecodeError::InputTooLarge { limit, actual } => {
                assert_eq!(limit, 4);
                assert_eq!(actual, 6);
            }
            other => panic!("expected InputTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn depth_exceeded_reports_the_offending_path() {
        let limits = DecodeLimits {
            max_depth: 1,
            ..DecodeLimits::default()
        };
        let err = decode_with_limits(r#"{"a":{"b":1}}"#, &limits).unwrap_err();
        match err {
            DecodeError::DepthExceeded { path, limit } => {
                assert_eq!(limit, 1);
                assert_eq!(path.to_string(), ".a.b");
            }
            other => panic!("expected DepthExceeded, got {other:?}"),
        }
    }

    #[test]
    fn string_too_long_reports_char_count_not_byte_count() {
        let limits = DecodeLimits {
            max_string_len: 2,
            ..DecodeLimits::default()
        };
        // 3 multi-byte characters: more bytes than chars, so this proves
        // the check counts chars, not UTF-8 bytes.
        let err = decode_with_limits("\"日本語\"", &limits).unwrap_err();
        match err {
            DecodeError::StringTooLong { limit, actual, .. } => {
                assert_eq!(limit, 2);
                assert_eq!(actual, 3);
            }
            other => panic!("expected StringTooLong, got {other:?}"),
        }
    }

    #[test]
    fn collection_too_large_reports_the_offending_path() {
        let limits = DecodeLimits {
            max_collection_len: 2,
            ..DecodeLimits::default()
        };
        let err = decode_with_limits(r#"{"items":[1,2,3]}"#, &limits).unwrap_err();
        match err {
            DecodeError::CollectionTooLarge { path, limit, actual } => {
                assert_eq!(limit, 2);
                assert_eq!(actual, 3);
                assert_eq!(path.to_string(), ".items");
            }
            other => panic!("expected CollectionTooLarge, got {other:?}"),
        }
    }
}
