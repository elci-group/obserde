use obserde_schema::Schema;
use obserde_value::{Document, Path};

use crate::annotations::SemanticAnnotations;
use crate::error::SemanticError;
use crate::resolver::SemanticResolver;
use crate::semantic_id::SemanticId;

/// This crate's own `Severity` — deliberately duplicated from
/// `obserde_validate::Severity` rather than imported. `obserde-padagonia`
/// has no dependency on `obserde-validate` at all, keeping structural and
/// semantic validation fully independent sibling passes despite both
/// having "validation" in their purpose — a fact enforced at the
/// dependency-graph level, matching how `obserde-json` enforces
/// "schema-agnostic" by simply not depending on `obserde-schema`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

/// One semantic finding against a `Document`, at a specific `Path`, with
/// a stable machine-readable `code`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SemanticIssue {
    pub path: Path,
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub semantic_id: Option<SemanticId>,
}

/// The outcome of [`validate_semantic`]: zero or more [`SemanticIssue`]s.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct SemanticValidationResult {
    issues: Vec<SemanticIssue>,
}

impl SemanticValidationResult {
    pub fn new(issues: Vec<SemanticIssue>) -> Self {
        Self { issues }
    }

    pub fn is_valid(&self) -> bool {
        !self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    pub fn issues(&self) -> &[SemanticIssue] {
        &self.issues
    }
}

/// Checks `doc` against `schema`'s `SemanticAnnotations`, via `resolver`.
///
/// **Two-part failure model, deliberately not symmetric with
/// `obserde_validate::validate`'s `ValidateError`:**
///
/// 1. Up front, before touching `resolver` or `doc` at all: every field
///    name in `annotations` must exist in `schema`. If not,
///    `Err(SemanticError::UnknownAnnotatedField)` — a deterministic
///    authoring mistake (the same `schema`+`annotations` fail the same
///    way on every call, regardless of document), exactly analogous to
///    `ValidateError::InvalidPatternGrammar`.
/// 2. Every other problem — an unknown concept, an unknown relation
///    target, a relation the ontology doesn't permit, or the resolver
///    itself failing (`Err(ResolverError)`, plausibly transient live I/O,
///    unlike case 1's deterministic mistake) — becomes a soft
///    [`SemanticIssue`] in the returned [`SemanticValidationResult`], not
///    a propagated `Err`. This matches `obserde_validate::validate_field`'s
///    own precedent: it runs every constraint unconditionally even after
///    a type mismatch, collecting every finding rather than stopping at
///    the first one. Concretely here: `exists(target)` failing does
///    **not** skip the following `relation_permitted(...)` check for the
///    same relation — both run unconditionally, so a target-unknown issue
///    and a not-permitted issue can legitimately co-occur and both
///    surface.
///
/// A field with no annotation is silently skipped (nothing to check). An
/// annotated field absent from `doc` is also silently skipped — that's
/// structural `validate()`'s job to flag, not semantic's; this function
/// assumes nothing about whether structural validation has already run.
pub fn validate_semantic(
    schema: &Schema,
    annotations: &SemanticAnnotations,
    resolver: &dyn SemanticResolver,
    doc: &Document,
) -> Result<SemanticValidationResult, SemanticError> {
    for field_name in annotations.field_names() {
        if schema.field(field_name).is_none() {
            return Err(SemanticError::UnknownAnnotatedField {
                field: field_name.to_string(),
            });
        }
    }

    let mut issues = Vec::new();
    let root = Path::root();

    for field in schema.fields() {
        let Some(semantics) = annotations.get(field.name()) else {
            continue;
        };
        if doc.get(field.name()).is_none() {
            continue;
        }
        let path = root.field(field.name());

        check_exists(&semantics.concept, &path, &mut issues, resolver, "semantic.concept.unknown", |id| {
            format!("field {:?} references unknown ontology concept {id}", field.name())
        });

        for (relation, target) in &semantics.relations {
            check_exists(target, &path, &mut issues, resolver, "semantic.relation.target-unknown", |id| {
                format!("field {:?}'s {relation:?} relation targets unknown ontology concept {id}", field.name())
            });

            match resolver.relation_permitted(&semantics.concept, relation, target) {
                Ok(true) => {}
                Ok(false) => issues.push(SemanticIssue {
                    path: path.clone(),
                    code: "semantic.relation.not-permitted".to_string(),
                    severity: Severity::Error,
                    message: format!(
                        "relation {relation:?} from {} to {target} is not permitted by the ontology",
                        semantics.concept
                    ),
                    semantic_id: Some(semantics.concept.clone()),
                }),
                Err(err) => issues.push(SemanticIssue {
                    path: path.clone(),
                    code: "semantic.resolver.unavailable".to_string(),
                    severity: Severity::Error,
                    message: format!("could not check permission for relation {relation:?}: {err}"),
                    semantic_id: Some(semantics.concept.clone()),
                }),
            }
        }
    }

    Ok(SemanticValidationResult::new(issues))
}

fn check_exists(
    id: &SemanticId,
    path: &Path,
    issues: &mut Vec<SemanticIssue>,
    resolver: &dyn SemanticResolver,
    unknown_code: &'static str,
    unknown_message: impl FnOnce(&SemanticId) -> String,
) {
    match resolver.exists(id) {
        Ok(true) => {}
        Ok(false) => issues.push(SemanticIssue {
            path: path.clone(),
            code: unknown_code.to_string(),
            severity: Severity::Error,
            message: unknown_message(id),
            semantic_id: Some(id.clone()),
        }),
        Err(err) => issues.push(SemanticIssue {
            path: path.clone(),
            code: "semantic.resolver.unavailable".to_string(),
            severity: Severity::Error,
            message: format!("could not check existence of {id}: {err}"),
            semantic_id: Some(id.clone()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotations::FieldSemantics;
    use crate::error::ResolverError;
    use obserde_core::{Contract, SchemaVersion};
    use obserde_schema::{Field, Type};

    fn id(s: &str) -> SemanticId {
        SemanticId::parse(s).unwrap()
    }

    fn schema(fields: Vec<Field>) -> Schema {
        let contract = Contract::new("elci.test", "semantic", SchemaVersion::new(1, 0, 0), 0).unwrap();
        Schema::new(contract, fields).unwrap()
    }

    fn doc_map(entries: Vec<(&str, Document)>) -> Document {
        Document::Map(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    struct AlwaysFailsResolver;
    impl SemanticResolver for AlwaysFailsResolver {
        fn exists(&self, _id: &SemanticId) -> Result<bool, ResolverError> {
            Err(ResolverError::new("simulated resolver outage"))
        }
        fn relation_permitted(&self, _from: &SemanticId, _relation: &str, _to: &SemanticId) -> Result<bool, ResolverError> {
            Err(ResolverError::new("simulated resolver outage"))
        }
    }

    #[test]
    fn unannotated_field_is_silently_skipped() {
        let schema = schema(vec![Field::new("score", Type::Integer)]);
        let annotations = SemanticAnnotations::new();
        let resolver = crate::resolver::StaticResolver::new();
        let doc = doc_map(vec![("score", Document::Integer(50))]);
        let result = validate_semantic(&schema, &annotations, &resolver, &doc).unwrap();
        assert!(result.is_valid());
        assert!(result.issues().is_empty());
    }

    #[test]
    fn annotated_field_absent_from_document_is_silently_skipped() {
        let schema = schema(vec![Field::new("score", Type::Integer).required(false)]);
        let mut annotations = SemanticAnnotations::new();
        annotations.annotate("score", FieldSemantics::new(id("Score")));
        let resolver = crate::resolver::StaticResolver::new(); // knows nothing
        let doc = doc_map(vec![]);
        let result = validate_semantic(&schema, &annotations, &resolver, &doc).unwrap();
        assert!(result.is_valid());
    }

    #[test]
    fn unknown_annotated_field_is_a_hard_error() {
        let schema = schema(vec![Field::new("score", Type::Integer)]);
        let mut annotations = SemanticAnnotations::new();
        annotations.annotate("typo_field", FieldSemantics::new(id("Score")));
        let resolver = crate::resolver::StaticResolver::new();
        let doc = doc_map(vec![("score", Document::Integer(50))]);
        let err = validate_semantic(&schema, &annotations, &resolver, &doc).unwrap_err();
        assert!(matches!(err, SemanticError::UnknownAnnotatedField { field } if field == "typo_field"));
    }

    #[test]
    fn unknown_concept_produces_an_issue() {
        let schema = schema(vec![Field::new("score", Type::Integer)]);
        let mut annotations = SemanticAnnotations::new();
        annotations.annotate("score", FieldSemantics::new(id("Score")));
        let resolver = crate::resolver::StaticResolver::new(); // "Score" not registered
        let doc = doc_map(vec![("score", Document::Integer(50))]);
        let result = validate_semantic(&schema, &annotations, &resolver, &doc).unwrap();
        assert!(!result.is_valid());
        assert_eq!(result.issues()[0].code, "semantic.concept.unknown");
    }

    #[test]
    fn unpermitted_relation_produces_an_issue() {
        let schema = schema(vec![Field::new("score", Type::Integer)]);
        let mut annotations = SemanticAnnotations::new();
        annotations.annotate("score", FieldSemantics::new(id("Score")).with_relation("measures", id("Domain")));
        let resolver = crate::resolver::StaticResolver::new()
            .with_concept(id("Score"))
            .with_concept(id("Domain")); // both known, relation never registered
        let doc = doc_map(vec![("score", Document::Integer(50))]);
        let result = validate_semantic(&schema, &annotations, &resolver, &doc).unwrap();
        assert!(!result.is_valid());
        assert!(result.issues().iter().any(|i| i.code == "semantic.relation.not-permitted"));
    }

    #[test]
    fn unknown_relation_target_and_not_permitted_can_both_surface_for_one_relation() {
        let schema = schema(vec![Field::new("score", Type::Integer)]);
        let mut annotations = SemanticAnnotations::new();
        annotations.annotate("score", FieldSemantics::new(id("Score")).with_relation("measures", id("Domain")));
        let resolver = crate::resolver::StaticResolver::new().with_concept(id("Score")); // "Domain" unknown, relation also never registered
        let doc = doc_map(vec![("score", Document::Integer(50))]);
        let result = validate_semantic(&schema, &annotations, &resolver, &doc).unwrap();
        let codes: Vec<&str> = result.issues().iter().map(|i| i.code.as_str()).collect();
        assert!(codes.contains(&"semantic.relation.target-unknown"), "codes: {codes:?}");
        assert!(codes.contains(&"semantic.relation.not-permitted"), "codes: {codes:?}");
    }

    #[test]
    fn directive_worked_example_field_passes_cleanly() {
        let schema = schema(vec![Field::new("score", Type::Integer)]);
        let mut annotations = SemanticAnnotations::new();
        annotations.annotate(
            "score",
            FieldSemantics::new(id("UNI.Assessment.Score"))
                .with_relation("measures", id("Assessment.Domain"))
                .with_relation("represents", id("ProjectQuality")),
        );
        let resolver = crate::resolver::StaticResolver::new()
            .with_relation(id("UNI.Assessment.Score"), "measures", id("Assessment.Domain"))
            .with_relation(id("UNI.Assessment.Score"), "represents", id("ProjectQuality"));
        let doc = doc_map(vec![("score", Document::Integer(85))]);
        let result = validate_semantic(&schema, &annotations, &resolver, &doc).unwrap();
        assert!(result.is_valid(), "unexpected issues: {:?}", result.issues());
    }

    #[test]
    fn resolver_failure_becomes_a_soft_issue_not_a_hard_abort() {
        let schema = schema(vec![
            Field::new("score", Type::Integer),
            Field::new("other", Type::Integer),
        ]);
        let mut annotations = SemanticAnnotations::new();
        annotations.annotate("score", FieldSemantics::new(id("Score")));
        annotations.annotate("other", FieldSemantics::new(id("Other")));
        let resolver = AlwaysFailsResolver;
        let doc = doc_map(vec![("score", Document::Integer(1)), ("other", Document::Integer(2))]);

        let result = validate_semantic(&schema, &annotations, &resolver, &doc).unwrap();
        assert!(!result.is_valid());
        // Both fields were still checked despite the resolver failing on
        // the first one — proves the call continues, not just that no
        // Err was returned.
        let paths: Vec<String> = result.issues().iter().map(|i| i.path.to_string()).collect();
        assert!(paths.contains(&".score".to_string()), "paths: {paths:?}");
        assert!(paths.contains(&".other".to_string()), "paths: {paths:?}");
        assert!(result.issues().iter().all(|i| i.code == "semantic.resolver.unavailable"));
    }
}
