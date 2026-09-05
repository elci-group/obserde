use obserde_core::ErrorCode;

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("duplicate field {name:?} in schema {contract}")]
    DuplicateField { contract: String, name: String },

    #[error("invalid field name {name:?}: {reason}")]
    InvalidFieldName { name: String, reason: String },

    #[error("invalid type expression {expr:?}: {reason}")]
    InvalidType { expr: String, reason: String },
}

impl ErrorCode for SchemaError {
    fn code(&self) -> &'static str {
        match self {
            SchemaError::DuplicateField { .. } => "schema.field.duplicate",
            SchemaError::InvalidFieldName { .. } => "schema.field.invalid-name",
            SchemaError::InvalidType { .. } => "schema.type.invalid",
        }
    }
}

pub type Result<T> = std::result::Result<T, SchemaError>;
