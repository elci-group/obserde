//! Shared error convention for every Obserde crate.

/// Implemented by every crate-level error enum in the Obserde workspace.
///
/// Returns a short, stable, dotted, machine-and-human-readable discriminant
/// (e.g. `"core.contract.invalid"`) rather than a numeric registry code.
/// Once shipped, a variant's code is itself part of the crate's public
/// contract and must not change.
pub trait ErrorCode {
    fn code(&self) -> &'static str;
}

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("invalid contract identifier {input:?}: {reason}")]
    InvalidContract { input: String, reason: String },

    #[error("invalid schema version {input:?}: {reason}")]
    InvalidVersion { input: String, reason: String },
}

impl ErrorCode for CoreError {
    fn code(&self) -> &'static str {
        match self {
            CoreError::InvalidContract { .. } => "core.contract.invalid",
            CoreError::InvalidVersion { .. } => "core.version.invalid",
        }
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
