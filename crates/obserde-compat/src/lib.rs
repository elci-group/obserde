//! Schema diff and compatibility analysis between two `Schema` versions.
//!
//! Two-stage design, mirroring the structural/semantic split
//! `obserde-validate` already uses for documents:
//!
//! - **`diff`** (`diff.rs`) is purely structural — it lists what changed
//!   between two `Schema`s, with no opinion on whether that matters.
//! - **`compatibility`** (`compatibility.rs`) adds judgment on top of the
//!   diff, classifying each change and producing an aggregate
//!   [`CompatibilityLevel`].
//!
//! There is no `error.rs` in this crate: comparing two already-constructed,
//! already-valid `Schema` values is total — no parsing, no I/O, nothing
//! that can fail. Every other crate in this workspace only has an error
//! type where something genuinely can fail; `obserde-compat` is the first
//! one without.
//!
//! [`CompatibilityLevel`] has five variants, matching the governing
//! directive's required model, but [`CompatibilityLevel::Unknown`] and
//! [`CompatibilityLevel::ConditionallyCompatible`] are never produced by
//! [`analyze`] today. See `compatibility.rs`'s module doc for why: every
//! change this crate can currently detect maps to a deterministic effect
//! in `obserde-validate`, and this crate does not consult `obserde-migrate`
//! (Phase 4, which does exist now) to check whether a registered migration
//! could make anything "conditionally" bridgeable. Both variants stay in
//! the enum for directive conformance and forward-compatibility.

pub mod compatibility;
pub mod diff;

pub use compatibility::{analyze, CompatibilityFinding, CompatibilityLevel, CompatibilityReport};
pub use diff::{diff, DiffEntry, DiffKind, SchemaDiff};
