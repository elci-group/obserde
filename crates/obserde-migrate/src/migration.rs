use std::fmt;

use obserde_core::SchemaVersion;
use obserde_schema::Schema;
use obserde_validate::validate;
use obserde_value::Document;

use crate::error::MigrationError;

/// A transform closure: `Ok(new_document)` on success, `Err(reason)` on
/// failure. Decoupled from `MigrationError` so migration authors don't
/// need to construct crate-internal error variants themselves.
type TransformFn = dyn Fn(&Document) -> Result<Document, String> + Send + Sync;
type Transform = Box<TransformFn>;

/// How strictly a [`Migration`] validates around its transform.
///
/// Post-validation is **never** skippable under either policy, in either
/// direction — that's the concrete "no silent migrations" enforcement
/// point. `PostOnly` only ever skips the *pre*-validation step, and only
/// on the forward direction (`Migration::apply_reverse` always behaves as
/// `Strict`, regardless of this field — see its doc comment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ValidationPolicy {
    /// Validate against the source schema before transforming, and
    /// against the target schema after. Matches directive §19's flow
    /// exactly. The default.
    Strict,
    /// Skip pre-validation (e.g. because the caller already validated
    /// upstream and wants to avoid redundant work); still always
    /// post-validates.
    PostOnly,
}

/// One migration: a transform from documents satisfying `source` to
/// documents satisfying `target`, with its own identity and version,
/// independent of the schema versions it bridges (directive §18).
///
/// `reverse` models reversibility as a *capability*, not a bare flag:
/// `None` means genuinely irreversible; `Some(f)` means `f` is the real
/// reverse transform (directive §18: "reversible where possible;
/// explicitly irreversible where not").
pub struct Migration {
    id: String,
    version: SchemaVersion,
    source: Schema,
    target: Schema,
    policy: ValidationPolicy,
    forward: Transform,
    reverse: Option<Transform>,
}

impl Migration {
    /// Constructs a `Migration` with `ValidationPolicy::Strict` and no
    /// reverse transform (irreversible by default). Does not reject
    /// `source == target` — a same-`SchemaId` self-loop migration is a
    /// legitimate tool for bridging two differently-structured revisions
    /// of the "same" declared version.
    pub fn new(
        id: impl Into<String>,
        version: SchemaVersion,
        source: Schema,
        target: Schema,
        forward: impl Fn(&Document) -> Result<Document, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            version,
            source,
            target,
            policy: ValidationPolicy::Strict,
            forward: Box::new(forward),
            reverse: None,
        }
    }

    pub fn with_policy(mut self, policy: ValidationPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn with_reverse(mut self, reverse: impl Fn(&Document) -> Result<Document, String> + Send + Sync + 'static) -> Self {
        self.reverse = Some(Box::new(reverse));
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn version(&self) -> SchemaVersion {
        self.version
    }

    pub fn source(&self) -> &Schema {
        &self.source
    }

    pub fn target(&self) -> &Schema {
        &self.target
    }

    pub fn policy(&self) -> ValidationPolicy {
        self.policy
    }

    pub fn is_reversible(&self) -> bool {
        self.reverse.is_some()
    }

    /// Forward: pre-validates against `source` (unless `policy` is
    /// `PostOnly`), runs the forward transform, then always
    /// post-validates against `target`.
    pub fn apply(&self, doc: &Document) -> Result<Document, MigrationError> {
        let skip_pre = matches!(self.policy, ValidationPolicy::PostOnly);
        self.run(doc, "forward", &self.source, &self.target, skip_pre, self.forward.as_ref())
    }

    /// Reverse: fails immediately with `NotReversible` if no reverse
    /// transform was registered, before doing any validation work.
    /// Otherwise **always** pre-validates against `target` and
    /// post-validates against `source`, regardless of `self.policy` — the
    /// reverse/rollback path is rarer and lower-trust than the forward
    /// hot path, so skipping its pre-validation is not offered.
    pub fn apply_reverse(&self, doc: &Document) -> Result<Document, MigrationError> {
        let reverse = self
            .reverse
            .as_deref()
            .ok_or_else(|| MigrationError::NotReversible { migration_id: self.id.clone() })?;
        self.run(doc, "reverse", &self.target, &self.source, false, reverse)
    }

    fn run(
        &self,
        doc: &Document,
        direction: &'static str,
        pre_schema: &Schema,
        post_schema: &Schema,
        skip_pre: bool,
        transform: &TransformFn,
    ) -> Result<Document, MigrationError> {
        if !skip_pre {
            let pre = validate(pre_schema, doc).map_err(|source| MigrationError::ValidationUnavailable {
                migration_id: self.id.clone(),
                direction,
                phase: "pre",
                source,
            })?;
            if !pre.is_valid() {
                return Err(MigrationError::PreValidationFailed {
                    migration_id: self.id.clone(),
                    direction,
                    issues: pre,
                });
            }
        }

        let transformed = transform(doc).map_err(|reason| MigrationError::TransformFailed {
            migration_id: self.id.clone(),
            direction,
            reason,
        })?;

        let post = validate(post_schema, &transformed).map_err(|source| MigrationError::ValidationUnavailable {
            migration_id: self.id.clone(),
            direction,
            phase: "post",
            source,
        })?;
        if !post.is_valid() {
            return Err(MigrationError::PostValidationFailed {
                migration_id: self.id.clone(),
                direction,
                issues: post,
            });
        }

        Ok(transformed)
    }
}

impl fmt::Debug for Migration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Migration")
            .field("id", &self.id)
            .field("version", &self.version)
            .field("source", &self.source.contract().to_string())
            .field("target", &self.target.contract().to_string())
            .field("policy", &self.policy)
            .field("reversible", &self.is_reversible())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use obserde_core::Contract;
    use obserde_schema::{Constraint, Field, Type};

    fn schema(version: SchemaVersion, fields: Vec<Field>) -> Schema {
        let contract = Contract::new("elci.test", "migrate", version, 0).unwrap();
        Schema::new(contract, fields).unwrap()
    }

    fn v1() -> Schema {
        schema(SchemaVersion::new(1, 0, 0), vec![Field::new("score", Type::Integer)])
    }

    fn v2() -> Schema {
        schema(
            SchemaVersion::new(2, 0, 0),
            vec![
                Field::new("score", Type::Integer),
                Field::new("note", Type::String).required(false),
            ],
        )
    }

    fn doc_map(entries: Vec<(&str, Document)>) -> Document {
        Document::Map(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    fn add_note(doc: &Document) -> Result<Document, String> {
        match doc {
            Document::Map(entries) => {
                let mut entries = entries.clone();
                entries.push(("note".to_string(), Document::String("migrated".to_string())));
                Ok(Document::Map(entries))
            }
            _ => Err("expected a map document".to_string()),
        }
    }

    fn remove_note(doc: &Document) -> Result<Document, String> {
        match doc {
            Document::Map(entries) => Ok(Document::Map(entries.iter().filter(|(k, _)| k != "note").cloned().collect())),
            _ => Err("expected a map document".to_string()),
        }
    }

    fn always_fails(_doc: &Document) -> Result<Document, String> {
        Err("boom".to_string())
    }

    /// Like `add_note`, but also defaults a missing "score" to 0 — used
    /// to demonstrate that `PostOnly` skipping pre-validation lets a
    /// document that would have failed pre-validation still succeed,
    /// provided the transform itself repairs what pre-validation would
    /// have flagged.
    fn add_note_and_default_score(doc: &Document) -> Result<Document, String> {
        match doc {
            Document::Map(entries) => {
                let mut entries = entries.clone();
                if !entries.iter().any(|(k, _)| k == "score") {
                    entries.push(("score".to_string(), Document::Integer(0)));
                }
                entries.push(("note".to_string(), Document::String("migrated".to_string())));
                Ok(Document::Map(entries))
            }
            _ => Err("expected a map document".to_string()),
        }
    }

    fn drop_score(_doc: &Document) -> Result<Document, String> {
        Ok(Document::Map(vec![]))
    }

    #[test]
    fn happy_path_forward_adds_note() {
        let migration = Migration::new("m1", SchemaVersion::new(1, 0, 0), v1(), v2(), add_note);
        let doc = doc_map(vec![("score", Document::Integer(5))]);
        let result = migration.apply(&doc).unwrap();
        assert_eq!(result.get("note"), Some(&Document::String("migrated".to_string())));
    }

    #[test]
    fn happy_path_reverse_removes_note() {
        let migration = Migration::new("m1", SchemaVersion::new(1, 0, 0), v1(), v2(), add_note).with_reverse(remove_note);
        let forward_doc = doc_map(vec![("score", Document::Integer(5))]);
        let migrated = migration.apply(&forward_doc).unwrap();
        let reversed = migration.apply_reverse(&migrated).unwrap();
        assert_eq!(reversed, doc_map(vec![("score", Document::Integer(5))]));
    }

    #[test]
    fn pre_validation_failed_when_input_missing_required_field() {
        let migration = Migration::new("m1", SchemaVersion::new(1, 0, 0), v1(), v2(), add_note);
        let bad_doc = doc_map(vec![]); // missing required "score"
        let err = migration.apply(&bad_doc).unwrap_err();
        assert!(matches!(err, MigrationError::PreValidationFailed { direction: "forward", .. }));
    }

    #[test]
    fn transform_failed_propagates_reason() {
        let migration = Migration::new("m1", SchemaVersion::new(1, 0, 0), v1(), v2(), always_fails);
        let doc = doc_map(vec![("score", Document::Integer(5))]);
        let err = migration.apply(&doc).unwrap_err();
        match err {
            MigrationError::TransformFailed { reason, direction: "forward", .. } => assert_eq!(reason, "boom"),
            other => panic!("expected TransformFailed, got {other:?}"),
        }
    }

    #[test]
    fn post_validation_failed_is_the_no_silent_migrations_proof() {
        // drop_score produces a document missing "score", which v2 still
        // requires — this must hard-fail, not silently succeed with data
        // loss.
        let migration = Migration::new("m1", SchemaVersion::new(1, 0, 0), v1(), v2(), drop_score);
        let doc = doc_map(vec![("score", Document::Integer(5))]);
        let err = migration.apply(&doc).unwrap_err();
        assert!(matches!(err, MigrationError::PostValidationFailed { direction: "forward", .. }));
    }

    #[test]
    fn post_only_skips_pre_validation_and_succeeds_when_output_is_valid() {
        let migration = Migration::new("m1", SchemaVersion::new(1, 0, 0), v1(), v2(), add_note_and_default_score)
            .with_policy(ValidationPolicy::PostOnly);
        // Missing "score" — would fail Strict pre-validation against v1,
        // but the transform itself defaults it, so the PostOnly-skipped
        // pre-check never gets in the way and the result still satisfies
        // v2's post-validation.
        let doc = doc_map(vec![]);
        let result = migration.apply(&doc).unwrap();
        assert_eq!(result.get("score"), Some(&Document::Integer(0)));
        assert_eq!(result.get("note"), Some(&Document::String("migrated".to_string())));
    }

    #[test]
    fn post_only_still_enforces_post_validation() {
        // Pre-validation is skipped, but drop_score's output still fails
        // post-validation against v2 — proving PostOnly never relaxes the
        // post-validation guarantee.
        let migration =
            Migration::new("m1", SchemaVersion::new(1, 0, 0), v1(), v2(), drop_score).with_policy(ValidationPolicy::PostOnly);
        let bad_doc = doc_map(vec![]); // would also fail Strict pre-validation, but that's skipped here
        let err = migration.apply(&bad_doc).unwrap_err();
        assert!(matches!(err, MigrationError::PostValidationFailed { direction: "forward", .. }));
    }

    #[test]
    fn not_reversible_when_no_reverse_transform_registered() {
        let migration = Migration::new("m1", SchemaVersion::new(1, 0, 0), v1(), v2(), add_note);
        let doc = doc_map(vec![("score", Document::Integer(5)), ("note", Document::String("x".into()))]);
        let err = migration.apply_reverse(&doc).unwrap_err();
        assert!(matches!(err, MigrationError::NotReversible { .. }));
    }

    #[test]
    fn reverse_is_always_strict_even_when_policy_is_post_only() {
        let migration = Migration::new("m1", SchemaVersion::new(1, 0, 0), v1(), v2(), add_note)
            .with_policy(ValidationPolicy::PostOnly)
            .with_reverse(remove_note);
        // Missing required "score" — apply_reverse pre-validates against
        // `target` (v2), which also requires "score", so this must fail
        // pre-validation despite the migration's PostOnly policy.
        let bad_doc = doc_map(vec![("note", Document::String("x".into()))]);
        let err = migration.apply_reverse(&bad_doc).unwrap_err();
        assert!(matches!(err, MigrationError::PreValidationFailed { direction: "reverse", .. }));
    }

    #[test]
    fn validation_unavailable_propagates_unrecognized_pattern_grammar() {
        let source = schema(
            SchemaVersion::new(1, 0, 0),
            vec![Field::new("id", Type::String).constraint(Constraint::Pattern { grammar: "email".to_string() })],
        );
        let migration = Migration::new("m1", SchemaVersion::new(1, 0, 0), source, v2(), add_note);
        let doc = doc_map(vec![("id", Document::String("x".into()))]);
        let err = migration.apply(&doc).unwrap_err();
        assert!(matches!(
            err,
            MigrationError::ValidationUnavailable { direction: "forward", phase: "pre", .. }
        ));
    }
}
