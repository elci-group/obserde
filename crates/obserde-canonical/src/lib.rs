//! Deterministic canonical form and content hashing.
//!
//! Owns all hashing in the Obserde workspace (`schema_hash`,
//! `document_hash`) rather than splitting it across `obserde-schema` and
//! here: `canonicalize()` needs `&Schema` from `obserde-schema`, so if
//! `obserde-schema` also depended on this crate for its own hash method,
//! the two crates would form a dependency cycle. Keeping hashing entirely
//! here avoids that outright.

pub mod canonical;
pub mod error;
pub mod hash;

pub use canonical::canonicalize;
pub use error::CanonicalisationError;
pub use hash::{document_hash, schema_hash, Hash};
