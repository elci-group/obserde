use obserde_core::ErrorCode;

use crate::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ValueError {
    #[error("expected {expected}, found {actual} at {path}")]
    TypeMismatch {
        expected: &'static str,
        actual: &'static str,
        path: Path,
    },
}

impl ErrorCode for ValueError {
    fn code(&self) -> &'static str {
        match self {
            ValueError::TypeMismatch { .. } => "value.type-mismatch",
        }
    }
}

pub type Result<T> = std::result::Result<T, ValueError>;
