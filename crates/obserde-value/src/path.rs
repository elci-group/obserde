use std::fmt;

/// One step in a `Path`: a map field name or a list index.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PathSegment {
    Field(String),
    Index(usize),
}

/// Addresses a location within a `Document`, e.g. `.scores.alice[2]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct Path(Vec<PathSegment>);

impl Path {
    pub fn root() -> Self {
        Self(Vec::new())
    }

    /// Returns a new path extended with a field segment.
    pub fn field(&self, name: impl Into<String>) -> Self {
        let mut segments = self.0.clone();
        segments.push(PathSegment::Field(name.into()));
        Self(segments)
    }

    /// Returns a new path extended with an index segment.
    pub fn index(&self, i: usize) -> Self {
        let mut segments = self.0.clone();
        segments.push(PathSegment::Index(i));
        Self(segments)
    }

    pub fn segments(&self) -> &[PathSegment] {
        &self.0
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return write!(f, ".");
        }
        for segment in &self.0 {
            match segment {
                PathSegment::Field(name) => write!(f, ".{name}")?,
                PathSegment::Index(i) => write!(f, "[{i}]")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_displays_as_dot() {
        assert_eq!(Path::root().to_string(), ".");
    }

    #[test]
    fn field_and_index_chain_display() {
        let path = Path::root().field("scores").field("alice").index(2);
        assert_eq!(path.to_string(), ".scores.alice[2]");
    }

    #[test]
    fn extension_does_not_mutate_original() {
        let root = Path::root();
        let extended = root.field("a");
        assert_eq!(root.to_string(), ".");
        assert_eq!(extended.to_string(), ".a");
    }
}
