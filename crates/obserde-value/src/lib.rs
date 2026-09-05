//! The Obserde `Document` value tree.
//!
//! `Document` is the format-agnostic intermediate representation all
//! encodings decode into and all validation/canonicalisation operates
//! over. It is neither a wire format (that's an encoding backend's
//! concern) nor a Rust domain struct (that's the application's concern) —
//! see the "wire types vs domain types" separation in Obserde's
//! architecture doc.

pub mod document;
pub mod error;
pub mod path;

pub use document::Document;
pub use error::ValueError;
pub use path::{Path, PathSegment};
