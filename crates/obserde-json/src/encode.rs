use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use obserde_value::{Document, Path};
use serde_json::{Map, Number, Value};

use crate::error::EncodeError;

/// Encodes a `Document` as JSON text.
///
/// Total over `Null`/`Bool`/`Integer`/`Float`/`String`/`List`/`Map`.
/// `Bytes` becomes a base64 string (standard alphabet, with padding, via
/// `base64::engine::general_purpose::STANDARD` — pinned explicitly since
/// `base64` ships several engines and picking the wrong one silently
/// breaks interop with anything else's base64 rather than erroring).
///
/// Rejects non-finite floats (`NaN`/infinite) rather than encoding them:
/// `serde_json`'s default behavior for a non-finite `f64` is to silently
/// emit JSON `null`, which would be an undetectable, data-corrupting
/// encode. See `EncodeError::NonFiniteFloat`.
///
/// A `Document::Map` with duplicate keys has them collapsed to
/// last-value-wins here, at encode time — not just on a later decode.
/// `serde_json::Map` (backing `Value::Object`) is a genuine map and
/// cannot represent duplicate keys even with the `preserve_order`
/// feature enabled (that feature only preserves *insertion order*, not
/// duplicate entries). See `decode`'s module docs for the fuller picture
/// of Obserde-JSON's non-bijective corners.
pub fn encode(doc: &Document) -> Result<String, EncodeError> {
    let value = document_to_value(doc, &Path::root())?;
    Ok(serde_json::to_string(&value)?)
}

fn document_to_value(doc: &Document, path: &Path) -> Result<Value, EncodeError> {
    match doc {
        Document::Null => Ok(Value::Null),
        Document::Bool(b) => Ok(Value::Bool(*b)),
        Document::Integer(i) => Ok(Value::Number(Number::from(*i))),
        Document::Float(f) => {
            if f.is_nan() {
                return Err(EncodeError::NonFiniteFloat {
                    path: path.clone(),
                    kind: "NaN",
                });
            }
            if f.is_infinite() {
                return Err(EncodeError::NonFiniteFloat {
                    path: path.clone(),
                    kind: "infinite",
                });
            }
            let number = Number::from_f64(*f).expect("finite f64 always produces a Number");
            Ok(Value::Number(number))
        }
        Document::String(s) => Ok(Value::String(s.clone())),
        Document::Bytes(b) => Ok(Value::String(STANDARD.encode(b))),
        Document::List(items) => {
            let mut values = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                values.push(document_to_value(item, &path.index(i))?);
            }
            Ok(Value::Array(values))
        }
        Document::Map(entries) => {
            let mut map = Map::with_capacity(entries.len());
            for (key, value) in entries {
                let encoded = document_to_value(value, &path.field(key))?;
                map.insert(key.clone(), encoded);
            }
            Ok(Value::Object(map))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_all_primitive_variants() {
        assert_eq!(encode(&Document::Null).unwrap(), "null");
        assert_eq!(encode(&Document::Bool(true)).unwrap(), "true");
        assert_eq!(encode(&Document::Integer(42)).unwrap(), "42");
        assert_eq!(encode(&Document::Float(1.5)).unwrap(), "1.5");
        assert_eq!(encode(&Document::String("hi".into())).unwrap(), "\"hi\"");
    }

    #[test]
    fn whole_number_float_keeps_a_decimal_point() {
        assert_eq!(encode(&Document::Float(5.0)).unwrap(), "5.0");
    }

    #[test]
    fn encodes_bytes_as_standard_base64() {
        let encoded = encode(&Document::Bytes(vec![0xde, 0xad, 0xbe, 0xef])).unwrap();
        assert_eq!(encoded, "\"3q2+7w==\"");
    }

    #[test]
    fn encodes_list_and_map() {
        let doc = Document::Map(vec![(
            "items".to_string(),
            Document::List(vec![Document::Integer(1), Document::Integer(2)]),
        )]);
        assert_eq!(encode(&doc).unwrap(), r#"{"items":[1,2]}"#);
    }

    #[test]
    fn rejects_top_level_nan() {
        let err = encode(&Document::Float(f64::NAN)).unwrap_err();
        match err {
            EncodeError::NonFiniteFloat { path, kind } => {
                assert_eq!(path.to_string(), ".");
                assert_eq!(kind, "NaN");
            }
            other => panic!("expected NonFiniteFloat, got {other:?}"),
        }
    }

    #[test]
    fn rejects_infinity_nested_inside_a_list() {
        let doc = Document::List(vec![Document::Integer(1), Document::Float(f64::INFINITY)]);
        let err = encode(&doc).unwrap_err();
        match err {
            EncodeError::NonFiniteFloat { path, kind } => {
                assert_eq!(path.to_string(), "[1]");
                assert_eq!(kind, "infinite");
            }
            other => panic!("expected NonFiniteFloat, got {other:?}"),
        }
    }

    #[test]
    fn rejects_nan_nested_inside_a_map() {
        let doc = Document::Map(vec![("score".to_string(), Document::Float(f64::NAN))]);
        let err = encode(&doc).unwrap_err();
        match err {
            EncodeError::NonFiniteFloat { path, .. } => assert_eq!(path.to_string(), ".score"),
            other => panic!("expected NonFiniteFloat, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_keys_collapse_to_last_value_at_encode_time() {
        let doc = Document::Map(vec![
            ("a".to_string(), Document::Integer(1)),
            ("a".to_string(), Document::Integer(2)),
        ]);
        assert_eq!(encode(&doc).unwrap(), r#"{"a":2}"#);
    }
}
