use std::collections::HashSet;

use obserde_core::Contract;

use crate::error::SchemaError;
use crate::field::Field;

/// A versioned, ordered collection of `Field`s, identified by a `Contract`.
///
/// Field declaration order is preserved (never silently reordered) and is
/// itself part of what identifies this schema's structure — canonical
/// re-ordering, if any, is `obserde-canonical`'s concern when it processes
/// a `Document` against this schema, not this type's.
///
/// There is deliberately no mutation API: a published schema is immutable,
/// and evolving it means constructing a new `Schema` with a new
/// `SchemaVersion` inside its `Contract`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Schema {
    contract: Contract,
    fields: Vec<Field>,
}

impl Schema {
    /// Constructs a `Schema`, rejecting duplicate field names.
    pub fn new(contract: Contract, fields: Vec<Field>) -> Result<Self, SchemaError> {
        let mut seen = HashSet::with_capacity(fields.len());
        for field in &fields {
            if !seen.insert(field.name().to_string()) {
                return Err(SchemaError::DuplicateField {
                    contract: contract.to_string(),
                    name: field.name().to_string(),
                });
            }
        }

        Ok(Self { contract, fields })
    }

    pub fn contract(&self) -> &Contract {
        &self.contract
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ty::Type;
    use obserde_core::SchemaVersion;

    fn contract() -> Contract {
        Contract::new("elci.uni", "snapshot", SchemaVersion::new(1, 0, 0), 0).unwrap()
    }

    #[test]
    fn rejects_duplicate_field_names() {
        let fields = vec![
            Field::new("score", Type::Integer),
            Field::new("score", Type::Float),
        ];
        let err = Schema::new(contract(), fields).unwrap_err();
        assert!(err.to_string().contains("score"));
    }

    #[test]
    fn preserves_declaration_order() {
        let fields = vec![
            Field::new("b", Type::Integer),
            Field::new("a", Type::Integer),
        ];
        let schema = Schema::new(contract(), fields).unwrap();
        let names: Vec<&str> = schema.fields().iter().map(Field::name).collect();
        assert_eq!(names, vec!["b", "a"]);
    }

    #[test]
    fn field_lookup_by_name() {
        let fields = vec![Field::new("score", Type::Integer)];
        let schema = Schema::new(contract(), fields).unwrap();
        assert!(schema.field("score").is_some());
        assert!(schema.field("missing").is_none());
    }

    #[test]
    fn two_independently_constructed_identical_schemas_are_equal() {
        let a = Schema::new(contract(), vec![Field::new("score", Type::Integer)]).unwrap();
        let b = Schema::new(contract(), vec![Field::new("score", Type::Integer)]).unwrap();
        assert_eq!(a, b);
    }
}
