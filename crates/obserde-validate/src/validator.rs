use obserde_schema::{Constraint, Field, Schema, Type};
use obserde_value::{Document, Path};

use crate::error::ValidateError;
use crate::result::{Severity, ValidationIssue, ValidationResult};

/// Validates `doc` against `schema`: every required field is present,
/// every present field's value matches its declared type (recursively for
/// `List`/`Map`), and every declared constraint on that value is
/// satisfied.
///
/// Never panics on malformed input — validation failures become
/// `ValidationIssue`s inside the returned `ValidationResult`.
/// `ValidateError` is reserved for conditions that prevent validation from
/// running at all, such as a `Constraint::Pattern` naming an unrecognized
/// grammar.
pub fn validate(schema: &Schema, doc: &Document) -> Result<ValidationResult, ValidateError> {
    let mut issues = Vec::new();
    let root = Path::root();

    match doc.as_map() {
        Some(_) => {
            for field in schema.fields() {
                let path = root.field(field.name());
                match doc.get(field.name()) {
                    Some(value) => validate_field(field, value, &path, &mut issues)?,
                    None if field.is_required() => issues.push(ValidationIssue {
                        path,
                        code: "validate.field.missing".to_string(),
                        severity: Severity::Error,
                        message: format!("required field {:?} is missing", field.name()),
                        expected: Some("present".to_string()),
                        actual: Some("absent".to_string()),
                    }),
                    None => {}
                }
            }
        }
        None => issues.push(ValidationIssue {
            path: root.clone(),
            code: "validate.document.not-a-map".to_string(),
            severity: Severity::Error,
            message: format!("expected a map document, found {}", doc.type_name()),
            expected: Some("map".to_string()),
            actual: Some(doc.type_name().to_string()),
        }),
    }

    Ok(ValidationResult::new(issues))
}

fn validate_field(
    field: &Field,
    value: &Document,
    path: &Path,
    issues: &mut Vec<ValidationIssue>,
) -> Result<(), ValidateError> {
    validate_type(field.ty(), value, path, issues);
    for constraint in field.constraints() {
        validate_constraint(constraint, value, path, issues)?;
    }
    Ok(())
}

fn validate_type(ty: &Type, value: &Document, path: &Path, issues: &mut Vec<ValidationIssue>) {
    match (ty, value) {
        (Type::Bool, Document::Bool(_))
        | (Type::Integer, Document::Integer(_))
        | (Type::Float, Document::Float(_))
        | (Type::String, Document::String(_))
        | (Type::Bytes, Document::Bytes(_)) => {}
        (Type::Timestamp, Document::String(s)) => {
            if !is_rfc3339(s) {
                issues.push(type_mismatch(
                    path,
                    "timestamp (RFC 3339)".to_string(),
                    format!("malformed timestamp string {s:?}"),
                ));
            }
        }
        (Type::List(element_ty), Document::List(items)) => {
            for (i, item) in items.iter().enumerate() {
                validate_type(element_ty, item, &path.index(i), issues);
            }
        }
        (Type::Map(_key_ty, value_ty), Document::Map(entries)) => {
            // Document map keys are always strings by construction, so
            // `_key_ty` has nothing independent to check against here; it
            // remains part of the schema purely for inspectability.
            for (k, v) in entries {
                validate_type(value_ty, v, &path.field(k), issues);
            }
        }
        (expected, actual) => {
            issues.push(type_mismatch(
                path,
                expected.to_string(),
                actual.type_name().to_string(),
            ));
        }
    }
}

fn type_mismatch(path: &Path, expected: String, actual: String) -> ValidationIssue {
    ValidationIssue {
        path: path.clone(),
        code: "validate.type.mismatch".to_string(),
        severity: Severity::Error,
        message: format!("expected {expected}, found {actual}"),
        expected: Some(expected),
        actual: Some(actual),
    }
}

fn validate_constraint(
    constraint: &Constraint,
    value: &Document,
    path: &Path,
    issues: &mut Vec<ValidationIssue>,
) -> Result<(), ValidateError> {
    match constraint {
        Constraint::Range { min, max } => {
            let n = match value {
                Document::Integer(i) => Some(*i as f64),
                Document::Float(f) => Some(*f),
                _ => None, // type mismatch already reported by validate_type
            };
            if let Some(n) = n {
                if min.is_some_and(|min| n < min) || max.is_some_and(|max| n > max) {
                    issues.push(ValidationIssue {
                        path: path.clone(),
                        code: "validate.constraint.range".to_string(),
                        severity: Severity::Error,
                        message: format!(
                            "value {n} out of range [{}, {}]",
                            min.map_or("-inf".to_string(), |v| v.to_string()),
                            max.map_or("+inf".to_string(), |v| v.to_string()),
                        ),
                        expected: Some(format!(
                            "[{}, {}]",
                            min.map_or("-inf".to_string(), |v| v.to_string()),
                            max.map_or("+inf".to_string(), |v| v.to_string()),
                        )),
                        actual: Some(n.to_string()),
                    });
                }
            }
        }
        Constraint::Length { min, max } => {
            let len = match value {
                Document::String(s) => Some(s.chars().count() as u64),
                Document::List(items) => Some(items.len() as u64),
                Document::Bytes(b) => Some(b.len() as u64),
                Document::Map(entries) => Some(entries.len() as u64),
                _ => None,
            };
            if let Some(len) = len {
                if min.is_some_and(|min| len < min) || max.is_some_and(|max| len > max) {
                    issues.push(ValidationIssue {
                        path: path.clone(),
                        code: "validate.constraint.length".to_string(),
                        severity: Severity::Error,
                        message: format!(
                            "length {len} out of range [{}, {}]",
                            min.map_or("0".to_string(), |v| v.to_string()),
                            max.map_or("+inf".to_string(), |v| v.to_string()),
                        ),
                        expected: Some(format!(
                            "[{}, {}]",
                            min.map_or("0".to_string(), |v| v.to_string()),
                            max.map_or("+inf".to_string(), |v| v.to_string()),
                        )),
                        actual: Some(len.to_string()),
                    });
                }
            }
        }
        Constraint::Pattern { grammar } => {
            if let Document::String(s) = value {
                match grammar.as_str() {
                    "identifier" => {
                        if !is_identifier(s) {
                            issues.push(ValidationIssue {
                                path: path.clone(),
                                code: "validate.constraint.pattern".to_string(),
                                severity: Severity::Error,
                                message: format!("{s:?} does not match the identifier grammar"),
                                expected: Some("identifier".to_string()),
                                actual: Some(s.clone()),
                            });
                        }
                    }
                    other => {
                        return Err(ValidateError::InvalidPatternGrammar {
                            grammar: other.to_string(),
                            reason: "unrecognized grammar name; only \"identifier\" is supported in Phase 1".to_string(),
                        });
                    }
                }
            }
        }
        Constraint::NonEmpty => {
            let is_empty = match value {
                Document::String(s) => s.is_empty(),
                Document::List(items) => items.is_empty(),
                Document::Map(entries) => entries.is_empty(),
                Document::Bytes(b) => b.is_empty(),
                _ => false,
            };
            if is_empty {
                issues.push(ValidationIssue {
                    path: path.clone(),
                    code: "validate.constraint.non-empty".to_string(),
                    severity: Severity::Error,
                    message: "value must not be empty".to_string(),
                    expected: Some("non-empty".to_string()),
                    actual: Some("empty".to_string()),
                });
            }
        }
        Constraint::Custom { .. } => {
            // Inspectable but not evaluated in Phase 1 — see Constraint::Custom's doc comment.
        }
    }
    Ok(())
}

/// A dotted-lowercase identifier grammar: each `.`-separated segment
/// starts with a lowercase ASCII letter, followed by lowercase ASCII
/// letters, digits, or underscores. Matches the grammar `obserde-core`
/// uses for `Contract` namespace/name segments.
fn is_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.split('.').all(|segment| {
        let mut chars = segment.chars();
        match chars.next() {
            Some(c) if c.is_ascii_lowercase() => {}
            _ => return false,
        }
        chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    })
}

/// A minimal, dependency-free RFC 3339 timestamp check:
/// `YYYY-MM-DDTHH:MM:SS(.fraction)?(Z|+HH:MM|-HH:MM)`. Not a full
/// calendar-aware validator (e.g. it tolerates day 31 in every month and a
/// leap second up to :60) — sufficient to catch malformed input without
/// pulling in a date/time dependency for Phase 1.
fn is_rfc3339(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 20 {
        return false;
    }
    let digit = |b: u8| b.is_ascii_digit();
    let digits = |slice: &[u8]| slice.iter().all(|&b| digit(b));

    if !digits(&bytes[0..4]) || bytes[4] != b'-' {
        return false;
    }
    if !digits(&bytes[5..7]) || bytes[7] != b'-' {
        return false;
    }
    if !digits(&bytes[8..10]) || !matches!(bytes[10], b'T' | b't') {
        return false;
    }
    if !digits(&bytes[11..13]) || bytes[13] != b':' {
        return false;
    }
    if !digits(&bytes[14..16]) || bytes[16] != b':' {
        return false;
    }
    if !digits(&bytes[17..19]) {
        return false;
    }

    let month: u32 = s[5..7].parse().unwrap();
    let day: u32 = s[8..10].parse().unwrap();
    let hour: u32 = s[11..13].parse().unwrap();
    let minute: u32 = s[14..16].parse().unwrap();
    let second: u32 = s[17..19].parse().unwrap();
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return false;
    }

    let mut rest = &bytes[19..];
    if !rest.is_empty() && rest[0] == b'.' {
        let mut i = 1;
        while i < rest.len() && digit(rest[i]) {
            i += 1;
        }
        if i == 1 {
            return false;
        }
        rest = &rest[i..];
    }

    if rest == b"Z" || rest == b"z" {
        return true;
    }
    if rest.len() == 6 && (rest[0] == b'+' || rest[0] == b'-') {
        return digits(&rest[1..3]) && rest[3] == b':' && digits(&rest[4..6]);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use obserde_core::{Contract, SchemaVersion};

    fn contract() -> Contract {
        Contract::new("elci.test", "fixture", SchemaVersion::new(1, 0, 0), 0).unwrap()
    }

    fn doc_map(entries: Vec<(&str, Document)>) -> Document {
        Document::Map(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    #[test]
    fn missing_required_field_is_reported() {
        let schema = Schema::new(contract(), vec![Field::new("score", Type::Integer)]).unwrap();
        let result = validate(&schema, &doc_map(vec![])).unwrap();
        assert!(!result.is_valid());
        assert_eq!(result.issues()[0].code, "validate.field.missing");
    }

    #[test]
    fn missing_optional_field_is_fine() {
        let schema = Schema::new(
            contract(),
            vec![Field::new("score", Type::Integer).required(false)],
        )
        .unwrap();
        let result = validate(&schema, &doc_map(vec![])).unwrap();
        assert!(result.is_valid());
    }

    #[test]
    fn nested_list_type_mismatch_is_reported_with_index_path() {
        let schema = Schema::new(
            contract(),
            vec![Field::new("scores", Type::list(Type::Integer))],
        )
        .unwrap();
        let doc = doc_map(vec![(
            "scores",
            Document::List(vec![Document::Integer(1), Document::String("x".into())]),
        )]);
        let result = validate(&schema, &doc).unwrap();
        assert!(!result.is_valid());
        let issue = &result.issues()[0];
        assert_eq!(issue.code, "validate.type.mismatch");
        assert_eq!(issue.path.to_string(), ".scores[1]");
    }

    #[test]
    fn nested_map_type_mismatch_is_reported_with_field_path() {
        let schema = Schema::new(
            contract(),
            vec![Field::new("scores", Type::map(Type::String, Type::Integer))],
        )
        .unwrap();
        let doc = doc_map(vec![(
            "scores",
            Document::Map(vec![("alice".to_string(), Document::String("not-a-number".into()))]),
        )]);
        let result = validate(&schema, &doc).unwrap();
        assert!(!result.is_valid());
        assert_eq!(result.issues()[0].path.to_string(), ".scores.alice");
    }

    #[test]
    fn range_constraint_pass_fail_boundary() {
        let schema = Schema::new(
            contract(),
            vec![Field::new("score", Type::Integer)
                .constraint(Constraint::Range { min: Some(0.0), max: Some(100.0) })],
        )
        .unwrap();

        let at_boundary = doc_map(vec![("score", Document::Integer(100))]);
        assert!(validate(&schema, &at_boundary).unwrap().is_valid());

        let over = doc_map(vec![("score", Document::Integer(101))]);
        let result = validate(&schema, &over).unwrap();
        assert!(!result.is_valid());
        assert_eq!(result.issues()[0].code, "validate.constraint.range");
    }

    #[test]
    fn length_constraint_pass_fail() {
        let schema = Schema::new(
            contract(),
            vec![Field::new("name", Type::String)
                .constraint(Constraint::Length { min: None, max: Some(3) })],
        )
        .unwrap();

        assert!(validate(&schema, &doc_map(vec![("name", Document::String("abc".into()))]))
            .unwrap()
            .is_valid());

        let result = validate(&schema, &doc_map(vec![("name", Document::String("abcd".into()))])).unwrap();
        assert!(!result.is_valid());
        assert_eq!(result.issues()[0].code, "validate.constraint.length");
    }

    #[test]
    fn pattern_constraint_pass_fail() {
        let schema = Schema::new(
            contract(),
            vec![Field::new("id", Type::String)
                .constraint(Constraint::Pattern { grammar: "identifier".to_string() })],
        )
        .unwrap();

        assert!(validate(&schema, &doc_map(vec![("id", Document::String("elci.uni".into()))]))
            .unwrap()
            .is_valid());

        let result = validate(&schema, &doc_map(vec![("id", Document::String("Not Valid".into()))])).unwrap();
        assert!(!result.is_valid());
        assert_eq!(result.issues()[0].code, "validate.constraint.pattern");
    }

    #[test]
    fn pattern_constraint_unrecognized_grammar_is_a_validate_error() {
        let schema = Schema::new(
            contract(),
            vec![Field::new("id", Type::String)
                .constraint(Constraint::Pattern { grammar: "email".to_string() })],
        )
        .unwrap();
        let err = validate(&schema, &doc_map(vec![("id", Document::String("x".into()))])).unwrap_err();
        assert!(err.to_string().contains("email"));
    }

    #[test]
    fn non_empty_constraint_pass_fail() {
        let schema = Schema::new(
            contract(),
            vec![Field::new("tags", Type::list(Type::String)).constraint(Constraint::NonEmpty)],
        )
        .unwrap();

        assert!(validate(
            &schema,
            &doc_map(vec![("tags", Document::List(vec![Document::String("a".into())]))])
        )
        .unwrap()
        .is_valid());

        let result = validate(&schema, &doc_map(vec![("tags", Document::List(vec![]))])).unwrap();
        assert!(!result.is_valid());
        assert_eq!(result.issues()[0].code, "validate.constraint.non-empty");
    }

    #[test]
    fn timestamp_type_pass_fail() {
        let schema = Schema::new(contract(), vec![Field::new("at", Type::Timestamp)]).unwrap();

        assert!(validate(
            &schema,
            &doc_map(vec![("at", Document::String("2026-09-05T12:00:00Z".into()))])
        )
        .unwrap()
        .is_valid());

        let result = validate(&schema, &doc_map(vec![("at", Document::String("not-a-timestamp".into()))])).unwrap();
        assert!(!result.is_valid());
        assert_eq!(result.issues()[0].code, "validate.type.mismatch");
    }

    #[test]
    fn valid_document_satisfies_schema_end_to_end() {
        let schema = Schema::new(
            contract(),
            vec![
                Field::new("score", Type::Integer)
                    .constraint(Constraint::Range { min: Some(0.0), max: Some(100.0) }),
                Field::new("name", Type::String).constraint(Constraint::NonEmpty),
            ],
        )
        .unwrap();
        let doc = doc_map(vec![
            ("score", Document::Integer(50)),
            ("name", Document::String("alice".into())),
        ]);
        assert!(validate(&schema, &doc).unwrap().is_valid());
    }
}
