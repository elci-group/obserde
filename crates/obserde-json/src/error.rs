use obserde_core::ErrorCode;
use obserde_value::Path;

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("cannot encode non-finite float ({kind}) at {path}")]
    NonFiniteFloat { path: Path, kind: &'static str },

    /// Practically unreachable once non-finite floats are pre-rejected —
    /// converting an already-finite `Document` to JSON text has no other
    /// failure mode. Kept so `encode` stays total (no internal
    /// `.unwrap()`) rather than assuming `serde_json::to_string` can
    /// never fail, matching `CanonicalisationError::Malformed`'s
    /// precedent of a typed error over a hidden panic.
    #[error("internal JSON encoding error: {0}")]
    Internal(#[from] serde_json::Error),
}

impl ErrorCode for EncodeError {
    fn code(&self) -> &'static str {
        match self {
            EncodeError::NonFiniteFloat { .. } => "json.encode.non-finite-float",
            EncodeError::Internal(_) => "json.encode.internal",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("input too large: {actual} bytes exceeds the {limit} byte limit")]
    InputTooLarge { limit: usize, actual: usize },

    #[error("malformed JSON at line {line}, column {column}: {message}")]
    Syntax {
        message: String,
        line: usize,
        column: usize,
    },

    #[error("nesting depth exceeds the limit of {limit} at {path}")]
    DepthExceeded { path: Path, limit: usize },

    #[error("string length {actual} exceeds the limit of {limit} at {path}")]
    StringTooLong {
        path: Path,
        limit: usize,
        actual: usize,
    },

    #[error("collection length {actual} exceeds the limit of {limit} at {path}")]
    CollectionTooLarge {
        path: Path,
        limit: usize,
        actual: usize,
    },

    /// A JSON integer literal outside `i64::MIN..=i64::MAX` — specifically
    /// the recoverable `(i64::MAX, u64::MAX]` band, where `serde_json`
    /// still knows the exact value. Rather than silently downgrading this
    /// to a lossy `Document::Float` (the mirror image of the precision
    /// loss `EncodeError::NonFiniteFloat` exists to prevent on the encode
    /// side), decoding fails explicitly. Integer literals beyond
    /// `u64::MAX` are already lossily approximated to `f64` inside
    /// `serde_json` itself before this crate's code ever runs (no
    /// `arbitrary_precision` feature enabled) — an unrecoverable, decode
    /// correctness limitation documented in `docs/ARCHITECTURE.md`, not
    /// something this error variant can catch.
    #[error("integer literal {literal:?} at {path} does not fit in a 64-bit signed integer")]
    IntegerOutOfRange { path: Path, literal: String },
}

impl ErrorCode for DecodeError {
    fn code(&self) -> &'static str {
        match self {
            DecodeError::InputTooLarge { .. } => "json.decode.input-too-large",
            DecodeError::Syntax { .. } => "json.decode.syntax",
            DecodeError::DepthExceeded { .. } => "json.decode.depth-exceeded",
            DecodeError::StringTooLong { .. } => "json.decode.string-too-long",
            DecodeError::CollectionTooLarge { .. } => "json.decode.collection-too-large",
            DecodeError::IntegerOutOfRange { .. } => "json.decode.integer-out-of-range",
        }
    }
}
