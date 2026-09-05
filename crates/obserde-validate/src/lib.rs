//! Structural and constraint validation of a `Document` against a `Schema`.
//!
//! Semantic validation (whether a value is *meaningful*, backed by
//! Padagonia's ontology) is explicitly out of scope here — that's Phase 5.
//! This crate only checks: does the document have the required fields,
//! do the present fields' values match their declared types, and do those
//! values satisfy their declared constraints.

pub mod error;
pub mod result;
pub mod validator;

pub use error::ValidateError;
pub use result::{Severity, ValidationIssue, ValidationResult};
pub use validator::validate;
