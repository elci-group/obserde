use std::fmt;

use crate::error::SemanticIdError;

/// A stable identifier referencing an external ontology concept (in
/// practice, a Padagonia concept — but this crate has no dependency on
/// the real `padagonia` crate; see this crate's `lib.rs` doc comment for
/// why).
///
/// Grammar: dot-separated segments, each starting with an ASCII letter
/// (**upper or lower case**), followed by ASCII letters, digits, or
/// underscores. This is deliberately more permissive than
/// `obserde_core::Contract`'s strict lowercase-only grammar — chosen so
/// it accepts both the governing directive's own illustrative PascalCase
/// style (`UNI.Assessment.Score`) and Padagonia's real
/// `stable_external_id()` output, which is a single opaque segment shaped
/// like `assessment_score_a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6` (one segment,
/// no dots, hex digits after the first letter — still a single valid
/// segment under this grammar).
///
/// Two notes worth stating explicitly rather than leaving implicit:
/// - **Case sensitivity**: unlike `Contract` (lowercase-only, so equality
///   is unambiguous), this type allows both cases per segment, so
///   `SemanticId::parse("UNI.Score")` and `SemanticId::parse("uni.score")`
///   are *different* values under derived `PartialEq`/`Hash`. Intentional:
///   directive-style and Padagonia-style identifiers are never expected
///   to cross-compare.
/// - **No length cap**, matching `Contract`'s own lack of one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SemanticId(String);

/// A dotted identifier segment: starts with an ASCII letter (either
/// case), followed by ASCII letters, digits, or underscores. Mirrors
/// `obserde_core::Contract`'s `validate_segment` structure exactly,
/// including its explicit empty-segment arm — that's what correctly
/// rejects `"UNI..Score"`, `".UNI.Score"`, and `"UNI.Score."` (each
/// produces an empty segment via `split('.')`), not just "doesn't start
/// with a letter."
fn validate_segment(segment: &str, input: &str) -> Result<(), SemanticIdError> {
    let invalid = |reason: String| SemanticIdError {
        input: input.to_string(),
        reason,
    };

    let mut chars = segment.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        Some(c) => {
            return Err(invalid(format!(
                "segment {segment:?} must start with an ASCII letter, found {c:?}"
            )))
        }
        None => return Err(invalid("segment must not be empty".to_string())),
    }

    if let Some(c) = chars.find(|c| !(c.is_ascii_alphanumeric() || *c == '_')) {
        return Err(invalid(format!(
            "segment {segment:?} contains disallowed character {c:?}"
        )));
    }

    Ok(())
}

impl SemanticId {
    pub fn parse(s: impl Into<String>) -> Result<Self, SemanticIdError> {
        let s = s.into();
        for segment in s.split('.') {
            validate_segment(segment, &s)?;
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SemanticId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_pascal_case_multi_segment() {
        let id = SemanticId::parse("UNI.Assessment.Score").unwrap();
        assert_eq!(id.as_str(), "UNI.Assessment.Score");
    }

    #[test]
    fn accepts_padagonia_shaped_single_segment_hash() {
        let id = SemanticId::parse("assessment_score_a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6").unwrap();
        assert_eq!(id.as_str(), "assessment_score_a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6");
    }

    #[test]
    fn rejects_empty_string() {
        assert!(SemanticId::parse("").is_err());
    }

    #[test]
    fn rejects_empty_segment_via_leading_dot() {
        assert!(SemanticId::parse(".Score").is_err());
    }

    #[test]
    fn rejects_empty_segment_via_trailing_dot() {
        assert!(SemanticId::parse("Score.").is_err());
    }

    #[test]
    fn rejects_empty_segment_via_consecutive_dots() {
        assert!(SemanticId::parse("UNI..Score").is_err());
    }

    #[test]
    fn rejects_non_ascii() {
        assert!(SemanticId::parse("Scoré").is_err());
    }

    #[test]
    fn rejects_digit_leading_segment() {
        assert!(SemanticId::parse("1Score").is_err());
    }

    #[test]
    fn rejects_underscore_leading_segment() {
        assert!(SemanticId::parse("_Score").is_err());
    }

    #[test]
    fn case_sensitivity_is_significant() {
        let upper = SemanticId::parse("UNI.Score").unwrap();
        let lower = SemanticId::parse("uni.score").unwrap();
        assert_ne!(upper, lower);
    }
}
