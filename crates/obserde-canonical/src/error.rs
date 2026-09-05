use obserde_core::ErrorCode;
use obserde_value::Path;

#[derive(Debug, thiserror::Error)]
pub enum CanonicalisationError {
    #[error("cannot canonicalize: {reason} at {path}")]
    Malformed { path: Path, reason: String },

    #[error("invalid hash hex string {input:?}")]
    InvalidHashHex { input: String },
}

impl ErrorCode for CanonicalisationError {
    fn code(&self) -> &'static str {
        match self {
            CanonicalisationError::Malformed { .. } => "canonical.document.malformed",
            CanonicalisationError::InvalidHashHex { .. } => "canonical.hash.invalid-hex",
        }
    }
}

pub type Result<T> = std::result::Result<T, CanonicalisationError>;
