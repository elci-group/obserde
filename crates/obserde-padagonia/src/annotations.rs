use std::collections::HashMap;

use crate::semantic_id::SemanticId;

/// One field's semantic annotation: the ontology concept it represents,
/// plus its outgoing relations to other concepts (directive §6's
/// `measures →`, `represents →` edges).
///
/// Directive §6's `range → 0..100` edge is deliberately **not** modeled
/// here — it's already fully covered by `obserde_schema::Constraint::Range`
/// from Phase 0. The directive's own diagram illustrates Padagonia and
/// Obserde describing one concept from complementary angles, not asking
/// this type to re-encode a value constraint Obserde already enforces
/// structurally. See this crate's `lib.rs` doc comment for the one named
/// consequence of that scoping (no cross-check between a field's
/// structural `Constraint::Range` and a resolver's semantic facts about
/// the same concept).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FieldSemantics {
    pub concept: SemanticId,
    pub relations: Vec<(String, SemanticId)>,
}

impl FieldSemantics {
    pub fn new(concept: SemanticId) -> Self {
        Self {
            concept,
            relations: Vec::new(),
        }
    }

    pub fn with_relation(mut self, relation: impl Into<String>, target: SemanticId) -> Self {
        self.relations.push((relation.into(), target));
        self
    }
}

/// A map from `Schema` field name to its [`FieldSemantics`], built
/// entirely external to `Schema`/`Field` — never merged into Phase 0's
/// types. This is what makes "clean separation between structural and
/// semantic concerns" a literal, checkable, type-level fact rather than
/// just a convention: `obserde_schema::Schema` has no field, method, or
/// dependency that knows this type exists.
///
/// One `SemanticAnnotations` value can validly be reused across schema
/// versions whose field names are stable — field/schema-name consistency
/// is checked inside `validate_semantic`, not at construction time here.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct SemanticAnnotations {
    fields: HashMap<String, FieldSemantics>,
}

impl SemanticAnnotations {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn annotate(&mut self, field_name: impl Into<String>, semantics: FieldSemantics) -> &mut Self {
        self.fields.insert(field_name.into(), semantics);
        self
    }

    pub fn get(&self, field_name: &str) -> Option<&FieldSemantics> {
        self.fields.get(field_name)
    }

    pub fn field_names(&self) -> impl Iterator<Item = &str> {
        self.fields.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> SemanticId {
        SemanticId::parse(s).unwrap()
    }

    #[test]
    fn annotate_and_get_round_trip() {
        let mut annotations = SemanticAnnotations::new();
        annotations.annotate(
            "score",
            FieldSemantics::new(id("UNI.Assessment.Score"))
                .with_relation("measures", id("Assessment.Domain"))
                .with_relation("represents", id("ProjectQuality")),
        );

        let semantics = annotations.get("score").unwrap();
        assert_eq!(semantics.concept, id("UNI.Assessment.Score"));
        assert_eq!(semantics.relations.len(), 2);
        assert!(annotations.get("missing").is_none());
    }

    #[test]
    fn field_names_lists_every_annotated_field() {
        let mut annotations = SemanticAnnotations::new();
        annotations.annotate("a", FieldSemantics::new(id("A")));
        annotations.annotate("b", FieldSemantics::new(id("B")));
        let mut names: Vec<&str> = annotations.field_names().collect();
        names.sort_unstable();
        assert_eq!(names, vec!["a", "b"]);
    }
}
