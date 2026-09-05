//! Obserde's governing architectural directive (§6, Relationship With
//! Padagonia) gives a worked example almost verbatim:
//!
//! ```text
//! Padagonia:
//! UNI.Assessment.Score
//!         │
//!         ├── measures → Assessment.Domain
//!         ├── range → 0..100
//!         └── represents → ProjectQuality
//! ```
//!
//! The `range → 0..100` edge is already fully covered by Obserde's own
//! `Constraint::Range` from Phase 0 — this test builds it that way, not
//! as a `FieldSemantics` relation (see `lib.rs`'s doc comment for why).
//! `measures`/`represents` are the genuinely new Phase 5 capability.

use obserde_core::{Contract, SchemaVersion};
use obserde_padagonia::{validate_semantic, FieldSemantics, SemanticAnnotations, SemanticId, StaticResolver};
use obserde_schema::{Constraint, Field, Schema, Type};
use obserde_value::Document;

fn id(s: &str) -> SemanticId {
    SemanticId::parse(s).unwrap()
}

fn schema() -> Schema {
    let contract = Contract::new("elci.uni", "assessment", SchemaVersion::new(1, 0, 0), 0).unwrap();
    Schema::new(
        contract,
        vec![
            Field::new("score", Type::Integer).constraint(Constraint::Range { min: Some(0.0), max: Some(100.0) }),
            Field::new("notes", Type::String).required(false), // deliberately unannotated
        ],
    )
    .unwrap()
}

fn annotations() -> SemanticAnnotations {
    let mut annotations = SemanticAnnotations::new();
    annotations.annotate(
        "score",
        FieldSemantics::new(id("UNI.Assessment.Score"))
            .with_relation("measures", id("Assessment.Domain"))
            .with_relation("represents", id("ProjectQuality")),
    );
    annotations
}

fn doc(score: i64) -> Document {
    Document::Map(vec![("score".to_string(), Document::Integer(score))])
}

#[test]
fn directive_section_6_worked_example_passes_cleanly() {
    let schema = schema();
    let annotations = annotations();
    let resolver = StaticResolver::new()
        .with_relation(id("UNI.Assessment.Score"), "measures", id("Assessment.Domain"))
        .with_relation(id("UNI.Assessment.Score"), "represents", id("ProjectQuality"));

    let result = validate_semantic(&schema, &annotations, &resolver, &doc(85)).unwrap();
    assert!(result.is_valid(), "unexpected issues: {:?}", result.issues());
}

#[test]
fn unknown_concept_produces_an_issue() {
    let schema = schema();
    let annotations = annotations();
    // Resolver knows nothing at all — "UNI.Assessment.Score" itself is unknown.
    let resolver = StaticResolver::new();

    let result = validate_semantic(&schema, &annotations, &resolver, &doc(85)).unwrap();
    assert!(!result.is_valid());
    assert!(result.issues().iter().any(|i| i.code == "semantic.concept.unknown"));
}

#[test]
fn unpermitted_relation_produces_an_issue() {
    let schema = schema();
    let annotations = annotations();
    // Every concept is known, but no relation is registered as permitted.
    let resolver = StaticResolver::new()
        .with_concept(id("UNI.Assessment.Score"))
        .with_concept(id("Assessment.Domain"))
        .with_concept(id("ProjectQuality"));

    let result = validate_semantic(&schema, &annotations, &resolver, &doc(85)).unwrap();
    assert!(!result.is_valid());
    let not_permitted = result.issues().iter().filter(|i| i.code == "semantic.relation.not-permitted").count();
    assert_eq!(not_permitted, 2, "both measures and represents should be flagged: {:?}", result.issues());
}

#[test]
fn unknown_relation_target_and_not_permitted_can_both_surface_for_one_relation() {
    let schema = schema();
    let mut annotations = SemanticAnnotations::new();
    annotations.annotate(
        "score",
        FieldSemantics::new(id("UNI.Assessment.Score")).with_relation("measures", id("Assessment.Domain")),
    );
    // Only the source concept is known; the target is unknown, and the
    // relation was never registered as permitted either.
    let resolver = StaticResolver::new().with_concept(id("UNI.Assessment.Score"));

    let result = validate_semantic(&schema, &annotations, &resolver, &doc(85)).unwrap();
    let codes: Vec<&str> = result.issues().iter().map(|i| i.code.as_str()).collect();
    assert!(codes.contains(&"semantic.relation.target-unknown"), "codes: {codes:?}");
    assert!(codes.contains(&"semantic.relation.not-permitted"), "codes: {codes:?}");
}

#[test]
fn unannotated_field_is_silently_skipped() {
    let schema = schema();
    let annotations = annotations(); // "notes" has no annotation
    let resolver = StaticResolver::new()
        .with_relation(id("UNI.Assessment.Score"), "measures", id("Assessment.Domain"))
        .with_relation(id("UNI.Assessment.Score"), "represents", id("ProjectQuality"));
    let document = Document::Map(vec![
        ("score".to_string(), Document::Integer(85)),
        ("notes".to_string(), Document::String("anything goes here".to_string())),
    ]);

    let result = validate_semantic(&schema, &annotations, &resolver, &document).unwrap();
    assert!(result.is_valid(), "unexpected issues: {:?}", result.issues());
}

#[test]
fn annotated_field_absent_from_document_is_silently_skipped() {
    let schema = schema();
    let annotations = annotations();
    let resolver = StaticResolver::new(); // knows nothing — would fail if "score" were checked
    let document = Document::Map(vec![]); // "score" absent entirely

    let result = validate_semantic(&schema, &annotations, &resolver, &document).unwrap();
    assert!(result.is_valid(), "unexpected issues: {:?}", result.issues());
}

#[test]
fn annotation_naming_an_unknown_schema_field_is_a_hard_error() {
    let schema = schema();
    let mut annotations = SemanticAnnotations::new();
    annotations.annotate("scoer", FieldSemantics::new(id("Typo"))); // misspelled field name
    let resolver = StaticResolver::new();

    let err = validate_semantic(&schema, &annotations, &resolver, &doc(85)).unwrap_err();
    match err {
        obserde_padagonia::SemanticError::UnknownAnnotatedField { field } => assert_eq!(field, "scoer"),
    }
}

#[test]
fn resolver_failure_becomes_a_soft_issue_not_a_hard_abort() {
    struct AlwaysFailsResolver;
    impl obserde_padagonia::SemanticResolver for AlwaysFailsResolver {
        fn exists(&self, _id: &SemanticId) -> Result<bool, obserde_padagonia::ResolverError> {
            Err(obserde_padagonia::ResolverError::new("simulated outage"))
        }
        fn relation_permitted(
            &self,
            _from: &SemanticId,
            _relation: &str,
            _to: &SemanticId,
        ) -> Result<bool, obserde_padagonia::ResolverError> {
            Err(obserde_padagonia::ResolverError::new("simulated outage"))
        }
    }

    let schema = schema();
    let annotations = annotations();
    let resolver = AlwaysFailsResolver;

    // No hard Err — the call completes, reporting soft issues instead,
    // and both relations were still checked despite the concept check
    // failing first.
    let result = validate_semantic(&schema, &annotations, &resolver, &doc(85)).unwrap();
    assert!(!result.is_valid());
    assert!(result.issues().len() >= 3, "expected the concept check plus both relation checks to all run: {:?}", result.issues());
    assert!(result.issues().iter().all(|i| i.code == "semantic.resolver.unavailable"));
}
