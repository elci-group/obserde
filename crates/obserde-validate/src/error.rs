use obserde_core::ErrorCode;

/// Conditions that prevent validation from running at all (e.g. a
/// malformed constraint declaration). Distinct from `ValidationIssue`,
/// which represents one *finding* inside a successful `ValidationResult` —
/// a document failing validation is not an error, it's the expected
/// outcome of a `ValidationResult` with `Severity::Error` issues.
#[derive(Debug, thiserror::Error)]
pub enum ValidateError {
    #[error("unparseable pattern grammar {grammar:?}: {reason}")]
    InvalidPatternGrammar { grammar: String, reason: String },
}

impl ErrorCode for ValidateError {
    fn code(&self) -> &'static str {
        match self {
            ValidateError::InvalidPatternGrammar { .. } => "validate.pattern.invalid-grammar",
        }
    }
}

pub type Result<T> = std::result::Result<T, ValidateError>;
