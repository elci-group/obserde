//! Migration definition, execution, and graph-based planning between
//! `Schema` versions.
//!
//! The first crate in this workspace to depend on `obserde-validate` as a
//! real (non-dev) dependency: `Migration::apply`/`apply_reverse` need real
//! `validate()` calls to implement the pre-validate → transform →
//! post-validate flow the directive requires — "no silent migrations"
//! means a transform that produces a document violating its target
//! schema is a hard `Err`, never a silently-accepted partial result.
//!
//! - [`SchemaId`] (`schema_id.rs`) is the migration graph's node identity
//!   — a `Contract` with its `revision` deliberately dropped, since
//!   `revision` is a non-structural build stamp and migrations transition
//!   between structurally different schema versions.
//! - [`Migration`] (`migration.rs`) is one edge: a source `Schema`, a
//!   target `Schema`, an identity/version of its own, a validation
//!   policy, and a forward transform with an optional reverse.
//! - [`MigrationGraph`] (`graph.rs`) collects registered migrations and
//!   answers planning questions: [`MigrationGraph::available_paths`] (all
//!   simple paths) and [`MigrationGraph::plan`] (the shortest one,
//!   erroring on missing or ambiguous routes).
//!
//! Directive §20's "trusted path" and "incompatible path" planner
//! capabilities are **not** implemented — there is no trust/signing/
//! provenance concept anywhere else in this codebase to hang "trusted"
//! off of, and every registered [`Migration`] is a binary yes/no graph
//! edge, with no partial-compatibility grading to distinguish
//! "incompatible" from simply "missing." This mirrors how
//! `obserde-compat` documents `CompatibilityLevel::Unknown`/
//! `ConditionallyCompatible` as reserved-but-unproduced rather than
//! silently omitted.

pub mod error;
pub mod graph;
pub mod migration;
pub mod schema_id;

pub use error::{MigrationError, PlanningError};
pub use graph::{MigrationGraph, MigrationPlan, MigrationStep};
pub use migration::{Migration, ValidationPolicy};
pub use schema_id::SchemaId;
