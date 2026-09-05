use crate::constraint::Constraint;
use crate::ty::Type;

/// A single field within a `Schema`: its name, type, whether it's
/// required, and the constraints its value must satisfy.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Field {
    name: String,
    ty: Type,
    required: bool,
    constraints: Vec<Constraint>,
    description: Option<String>,
}

impl Field {
    /// Constructs a required field with no constraints and no description.
    /// Use the builder methods to adjust.
    pub fn new(name: impl Into<String>, ty: Type) -> Self {
        Self {
            name: name.into(),
            ty,
            required: true,
            constraints: Vec::new(),
            description: None,
        }
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn constraint(mut self, constraint: Constraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ty(&self) -> &Type {
        &self.ty
    }

    pub fn is_required(&self) -> bool {
        self.required
    }

    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    pub fn description_text(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_required_no_constraints() {
        let f = Field::new("score", Type::Integer);
        assert_eq!(f.name(), "score");
        assert!(f.is_required());
        assert!(f.constraints().is_empty());
        assert_eq!(f.description_text(), None);
    }

    #[test]
    fn builder_methods_compose() {
        let f = Field::new("score", Type::Integer)
            .required(false)
            .constraint(Constraint::Range { min: Some(0.0), max: Some(100.0) })
            .description("assessment score");
        assert!(!f.is_required());
        assert_eq!(f.constraints().len(), 1);
        assert_eq!(f.description_text(), Some("assessment score"));
    }
}
