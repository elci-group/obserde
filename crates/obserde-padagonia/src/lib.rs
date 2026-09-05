//! Semantic identifiers and pluggable ontology resolution for Obserde
//! schemas — directive §44 Phase 5: "Introduce: semantic identifiers;
//! ontology references; semantic validation. Maintain clean separation
//! between structural and semantic concerns."
//!
//! # Why this crate has no dependency on the real `padagonia` crate
//!
//! Directive §6 frames Padagonia as Obserde's semantic authority
//! ("PADAGONIA: 'What is this?' / OBSERDE: 'Is this valid according to
//! its contract?'") and requires: "Obserde MUST NOT duplicate Padagonia's
//! ontology unnecessarily... Padagonia integration SHOULD nevertheless be
//! optional at the lowest framework layer so Obserde remains
//! independently usable."
//!
//! The real Padagonia project (a separate git repository) was inspected
//! directly before designing this crate, rather than assumed: it is a
//! full "ontology-native, immutable, provenance-rich graph store" with an
//! HTTP server (`axum`, `tokio`, `tower-http`), a metrics exporter, and
//! vector search (`fast-hnsw`) — and it depends on `bound-core` via an
//! **unpublished, unpinned, cross-repo relative path**
//! (`{ path = "../bound/crates/bound-core" }`) that only resolves if a
//! sibling `bound` repository happens to be checked out on the same
//! machine. Its `stable_external_id()` function produces opaque hash
//! strings (`"{kind}_{32 hex digits}"`), not the human-readable dotted
//! identifiers the directive's own illustrative example uses
//! (`UNI.Assessment.Score`) — confirming that example is architectural
//! illustration, not a literal format spec. A second Padagonia ontology
//! model (`software_ontology`) addresses its own graph nodes by plain,
//! model-specific `&str` — there is no single canonical Padagonia
//! identifier type to bind against, even within Padagonia itself.
//!
//! Taking a real dependency on `padagonia` — in `[dependencies]` *or*
//! `[dev-dependencies]` — would buy zero type-safety (Padagonia itself
//! just passes `String`/`&str` around) in exchange for a 10x+ dependency-
//! footprint increase and a fragile, unpublished cross-repo coupling.
//! Instead, this crate defines Obserde's own pluggable semantic-
//! integration interface: [`SemanticId`] and [`SemanticResolver`]. A real
//! Padagonia-backed adapter implementing [`SemanticResolver`] is squarely
//! possible for an adopter to write, outside this workspace — this crate
//! ships only [`StaticResolver`], a genuinely usable in-memory
//! implementation, as a concrete existence proof that the trait is
//! implementable and as a real tool for small deployments and tests.
//!
//! # What's deliberately not implemented
//!
//! - **Cross-checking structural and semantic constraints.** Directive
//!   §6's own worked example shows a `range → 0..100` edge on a Padagonia
//!   concept — already fully covered by
//!   `obserde_schema::Constraint::Range` from Phase 0, so
//!   [`FieldSemantics`] does not re-model it. But nothing here detects a
//!   *mismatch* between a field's own `Constraint::Range` and what a
//!   resolver might independently know about the same concept's valid
//!   range (e.g. Padagonia says `0..100`, the `Field` says `0..50`). The
//!   concrete shape a future increment would take: a
//!   `SemanticResolver::declared_range(id) -> Option<(f64, f64)>` method,
//!   cross-checked against `Constraint::Range` in `validate_semantic`.
//!   Not built now — it only covers `Range` (not directive §13's general
//!   "meaningful value" idea), adds trait surface for one narrow case,
//!   and risks quietly reintroducing the structural/semantic coupling
//!   this crate's boundary is designed to avoid.
//! - **"Value is meaningful within its declared semantic type"**
//!   (directive §13's third semantic-validation example, beyond the
//!   narrow slice above). There is no generic way to ask this without a
//!   much deeper ontology integration than an existence/permission check
//!   can express. Documented honestly as deferred, the same way
//!   `obserde_compat::CompatibilityLevel::Unknown`/`ConditionallyCompatible`
//!   and directive §20's "trusted path"/"incompatible path" were
//!   documented as reserved-but-unproduced rather than faked.

pub mod annotations;
pub mod error;
pub mod resolver;
pub mod semantic_id;
pub mod validate;

pub use annotations::{FieldSemantics, SemanticAnnotations};
pub use error::{ResolverError, SemanticError, SemanticIdError};
pub use resolver::{SemanticResolver, StaticResolver};
pub use semantic_id::SemanticId;
pub use validate::{validate_semantic, Severity, SemanticIssue, SemanticValidationResult};
