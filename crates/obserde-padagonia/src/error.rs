use obserde_core::ErrorCode;

#[derive(Debug, thiserror::Error)]
#[error("invalid semantic identifier {input:?}: {reason}")]
pub struct SemanticIdError {
    pub input: String,
    pub reason: String,
}

impl ErrorCode for SemanticIdError {
    fn code(&self) -> &'static str {
        "padagonia.semantic-id.invalid"
    }
}

/// A real [`crate::SemanticResolver`] implementation's own failure (I/O,
/// timeout, malformed response from a live ontology system, ...).
/// Deliberately minimal — a message only, no generic boxed `source`
/// chaining — to keep the trait boundary simple. Not a top-level crate
/// error in its own right: a resolver failure becomes a soft
/// [`crate::SemanticIssue`] inside a [`crate::SemanticValidationResult`],
/// not a propagated `Err` (see `validate.rs`'s module doc for why).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}")]
pub struct ResolverError {
    pub reason: String,
}

impl ResolverError {
    pub fn new(reason: impl Into<String>) -> Self {
        Self { reason: reason.into() }
    }
}

/// Failures from [`crate::validate_semantic`] itself — conditions that
/// prevent semantic validation from running at all, checkable without
/// ever touching a resolver. Distinct from a resolver's own failures
/// (live I/O, see [`ResolverError`]), which are soft, per-finding
/// [`crate::SemanticIssue`]s instead: an `UnknownAnnotatedField` is a
/// deterministic authoring mistake — the same `Schema` and
/// `SemanticAnnotations` fail the same way on every call regardless of
/// document, exactly analogous to `obserde_validate::ValidateError`'s
/// `InvalidPatternGrammar` — so aborting the whole call is correct here,
/// unlike a plausibly-transient resolver failure.
#[derive(Debug, thiserror::Error)]
pub enum SemanticError {
    #[error("semantic annotation names field {field:?}, which does not exist in the schema")]
    UnknownAnnotatedField { field: String },
}

impl ErrorCode for SemanticError {
    fn code(&self) -> &'static str {
        match self {
            SemanticError::UnknownAnnotatedField { .. } => "padagonia.validation.unknown-annotated-field",
        }
    }
}
