use obserde_core::ErrorCode;
use obserde_validate::{ValidateError, ValidationResult};

use crate::graph::MigrationPlan;
use crate::schema_id::SchemaId;

/// Failures from constructing, executing, or registering a migration.
///
/// Distinct from [`PlanningError`]: this crate follows the same split
/// `obserde-validate` established between `ValidateError` ("the operation
/// itself couldn't run") and `ValidationResult` ("here's the structured
/// outcome") — `MigrationError` is the former kind of thing here.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    /// `validate()` itself returned `Err` (e.g. an unrecognized `Pattern`
    /// grammar) rather than a `ValidationResult` — the pre/post-validation
    /// step could not run at all.
    #[error("migration {migration_id:?} {direction} {phase}-validation could not run: {source}")]
    ValidationUnavailable {
        migration_id: String,
        direction: &'static str,
        phase: &'static str,
        #[source]
        source: ValidateError,
    },

    #[error("migration {migration_id:?} {direction} pre-validation failed: input document does not satisfy the source schema")]
    PreValidationFailed {
        migration_id: String,
        direction: &'static str,
        issues: ValidationResult,
    },

    /// The concrete "no silent migrations" enforcement point: a transform
    /// that produces a document violating its target schema is a hard
    /// error, never a silently-accepted partial result.
    #[error("migration {migration_id:?} {direction} post-validation failed: transformed document does not satisfy the target schema")]
    PostValidationFailed {
        migration_id: String,
        direction: &'static str,
        issues: ValidationResult,
    },

    #[error("migration {migration_id:?} {direction} transform failed: {reason}")]
    TransformFailed {
        migration_id: String,
        direction: &'static str,
        reason: String,
    },

    #[error("migration {migration_id:?} has no reverse transform")]
    NotReversible { migration_id: String },

    /// Registering a `Migration` whose `source`/`target` `Schema` shares a
    /// [`SchemaId`] with an already-registered `Schema`, but disagrees on
    /// `fields()` — the register-time guard that makes `SchemaId`'s
    /// revision-dropping assumption safe (see `schema_id.rs`), turning
    /// what would otherwise be a confusing `execute()`-time failure two
    /// hops later into an immediate, precise authoring error.
    #[error("schema {schema_id} is registered with conflicting structure by migrations {migration_id:?} and {conflicting_migration_id:?}")]
    SchemaIdConflict {
        schema_id: String,
        migration_id: String,
        conflicting_migration_id: String,
    },
}

impl ErrorCode for MigrationError {
    fn code(&self) -> &'static str {
        match self {
            MigrationError::ValidationUnavailable { .. } => "migrate.validation.unavailable",
            MigrationError::PreValidationFailed { .. } => "migrate.validation.pre-failed",
            MigrationError::PostValidationFailed { .. } => "migrate.validation.post-failed",
            MigrationError::TransformFailed { .. } => "migrate.transform.failed",
            MigrationError::NotReversible { .. } => "migrate.reverse.unavailable",
            MigrationError::SchemaIdConflict { .. } => "migrate.graph.schema-id-conflict",
        }
    }
}

/// Failures from [`crate::MigrationGraph::plan`]. Carries borrowed
/// [`MigrationPlan`]s, unlike [`MigrationError`], since a planning
/// failure legitimately needs to hand back the candidate plans it found
/// (or their absence) — matching this codebase's "explain the cause,
/// don't just say no" doctrine established by `obserde-compat`'s
/// `CompatibilityFinding`.
#[derive(Debug, thiserror::Error)]
pub enum PlanningError<'g> {
    #[error("no migration path from {from} to {to}")]
    MissingMigration { from: SchemaId, to: SchemaId },

    #[error("ambiguous migration path from {from} to {to}: multiple equally short paths found")]
    AmbiguousMigration {
        from: SchemaId,
        to: SchemaId,
        candidates: Vec<MigrationPlan<'g>>,
    },
}

impl<'g> ErrorCode for PlanningError<'g> {
    fn code(&self) -> &'static str {
        match self {
            PlanningError::MissingMigration { .. } => "migrate.plan.missing",
            PlanningError::AmbiguousMigration { .. } => "migrate.plan.ambiguous",
        }
    }
}
