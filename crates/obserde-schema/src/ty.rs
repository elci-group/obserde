use std::fmt;

/// The Obserde type system: primitive types plus composite `List`/`Map`
/// types built from them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Type {
    Bool,
    Integer,
    Float,
    String,
    Bytes,
    Timestamp,
    List(Box<Type>),
    Map(Box<Type>, Box<Type>),
}

impl Type {
    pub fn list(element: Type) -> Type {
        Type::List(Box::new(element))
    }

    pub fn map(key: Type, value: Type) -> Type {
        Type::Map(Box::new(key), Box::new(value))
    }

    pub fn is_primitive(&self) -> bool {
        !self.is_composite()
    }

    pub fn is_composite(&self) -> bool {
        matches!(self, Type::List(_) | Type::Map(_, _))
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Bool => write!(f, "bool"),
            Type::Integer => write!(f, "integer"),
            Type::Float => write!(f, "float"),
            Type::String => write!(f, "string"),
            Type::Bytes => write!(f, "bytes"),
            Type::Timestamp => write!(f, "timestamp"),
            Type::List(element) => write!(f, "list<{element}>"),
            Type::Map(key, value) => write!(f, "map<{key}, {value}>"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_display() {
        assert_eq!(Type::Bool.to_string(), "bool");
        assert_eq!(Type::Timestamp.to_string(), "timestamp");
    }

    #[test]
    fn composite_display_matches_directive_example() {
        let ty = Type::map(Type::String, Type::Integer);
        assert_eq!(ty.to_string(), "map<string, integer>");
    }

    #[test]
    fn nested_list_display() {
        let ty = Type::list(Type::list(Type::Float));
        assert_eq!(ty.to_string(), "list<list<float>>");
    }

    #[test]
    fn is_composite_vs_primitive() {
        assert!(Type::Bool.is_primitive());
        assert!(!Type::Bool.is_composite());
        assert!(Type::list(Type::Bool).is_composite());
        assert!(!Type::list(Type::Bool).is_primitive());
    }
}
