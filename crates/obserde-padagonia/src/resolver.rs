use std::collections::HashSet;

use crate::error::ResolverError;
use crate::semantic_id::SemanticId;

/// The pluggable boundary to an external ontology authority (in practice,
/// Padagonia — see this crate's `lib.rs` doc comment for why this trait
/// exists instead of a direct dependency on the real `padagonia` crate).
///
/// Fallible: a real implementation backed by a live ontology system
/// involves I/O that can genuinely fail. A resolver `Err` becomes a soft
/// [`crate::SemanticIssue`] inside [`crate::validate_semantic`]'s result,
/// not a propagated hard error — see `validate.rs`'s module doc.
pub trait SemanticResolver {
    /// Does this ontology concept exist?
    fn exists(&self, id: &SemanticId) -> Result<bool, ResolverError>;

    /// Is the named relation from `from` to `to` permitted by the
    /// ontology?
    fn relation_permitted(&self, from: &SemanticId, relation: &str, to: &SemanticId) -> Result<bool, ResolverError>;
}

/// A genuinely usable (not test-only) in-memory [`SemanticResolver`] —
/// useful for small deployments or tests without a live ontology system.
/// This is the only `SemanticResolver` implementation this crate ships;
/// a real Padagonia-backed adapter is documented, not built, in
/// `docs/ARCHITECTURE.md`.
#[derive(Debug, Clone, Default)]
pub struct StaticResolver {
    known: HashSet<SemanticId>,
    permitted_relations: HashSet<(SemanticId, String, SemanticId)>,
}

impl StaticResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_concept(mut self, id: SemanticId) -> Self {
        self.known.insert(id);
        self
    }

    /// Registers `relation` as permitted from `from` to `to`, and
    /// registers both endpoints as known concepts too (ergonomic
    /// convenience — a permitted relation between two concepts implies
    /// both exist).
    pub fn with_relation(mut self, from: SemanticId, relation: impl Into<String>, to: SemanticId) -> Self {
        self.known.insert(from.clone());
        self.known.insert(to.clone());
        self.permitted_relations.insert((from, relation.into(), to));
        self
    }
}

impl SemanticResolver for StaticResolver {
    fn exists(&self, id: &SemanticId) -> Result<bool, ResolverError> {
        Ok(self.known.contains(id))
    }

    fn relation_permitted(&self, from: &SemanticId, relation: &str, to: &SemanticId) -> Result<bool, ResolverError> {
        Ok(self
            .permitted_relations
            .contains(&(from.clone(), relation.to_string(), to.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> SemanticId {
        SemanticId::parse(s).unwrap()
    }

    #[test]
    fn with_concept_registers_existence() {
        let resolver = StaticResolver::new().with_concept(id("A"));
        assert_eq!(resolver.exists(&id("A")), Ok(true));
        assert_eq!(resolver.exists(&id("B")), Ok(false));
    }

    #[test]
    fn with_relation_auto_registers_both_endpoints() {
        let resolver = StaticResolver::new().with_relation(id("A"), "measures", id("B"));
        assert_eq!(resolver.exists(&id("A")), Ok(true));
        assert_eq!(resolver.exists(&id("B")), Ok(true));
        assert_eq!(resolver.relation_permitted(&id("A"), "measures", &id("B")), Ok(true));
    }

    #[test]
    fn relation_not_registered_is_not_permitted() {
        let resolver = StaticResolver::new().with_relation(id("A"), "measures", id("B"));
        assert_eq!(resolver.relation_permitted(&id("A"), "represents", &id("B")), Ok(false));
    }

    #[test]
    fn both_concepts_known_but_relation_never_registered_is_not_permitted() {
        // Built via two separate with_concept calls, deliberately never
        // calling with_relation — proves knowing both endpoints doesn't
        // implicitly permit a relation between them.
        let resolver = StaticResolver::new().with_concept(id("A")).with_concept(id("B"));
        assert_eq!(resolver.exists(&id("A")), Ok(true));
        assert_eq!(resolver.exists(&id("B")), Ok(true));
        assert_eq!(resolver.relation_permitted(&id("A"), "measures", &id("B")), Ok(false));
    }
}
