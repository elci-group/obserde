/// A constraint evaluated against a field's value during validation
/// (`obserde-validate`), not by the schema itself.
///
/// `Custom` is an inspectable escape hatch for constraints this Phase 1
/// vocabulary doesn't cover. It is intentionally not evaluated by the
/// Phase 1 validator — a schema may declare one, but `obserde-validate`
/// documents it as a no-op rather than silently treating it as satisfied
/// without a record of why.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Constraint {
    /// Numeric bounds, e.g. `0 <= score <= 100`. Either bound may be absent
    /// for a one-sided range.
    Range { min: Option<f64>, max: Option<f64> },
    /// String/collection length bounds, e.g. `string length <= 200`.
    Length { min: Option<u64>, max: Option<u64> },
    /// A named grammar a string value must match, e.g. an identifier
    /// grammar. `grammar` is a human-readable name/description of the
    /// grammar, not necessarily a regular expression.
    Pattern { grammar: String },
    /// The value must not be empty (empty string, empty list, empty map).
    NonEmpty,
    /// An inspectable, not-yet-evaluated constraint.
    Custom { name: String, description: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_constraint_is_constructible_and_comparable() {
        let a = Constraint::Range { min: Some(0.0), max: Some(100.0) };
        let b = Constraint::Range { min: Some(0.0), max: Some(100.0) };
        assert_eq!(a, b);
    }

    #[test]
    fn one_sided_range_allows_absent_bound() {
        let c = Constraint::Range { min: Some(0.0), max: None };
        match c {
            Constraint::Range { min, max } => {
                assert_eq!(min, Some(0.0));
                assert_eq!(max, None);
            }
            _ => panic!("expected Range"),
        }
    }
}
