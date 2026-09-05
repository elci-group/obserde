//! The Obserde schema language.
//!
//! A `Schema` is a versioned, ordered collection of `Field`s, each with a
//! `Type` and zero or more `Constraint`s. Schemas are inspectable and
//! hashable without executing any application code.

pub mod constraint;
pub mod error;
pub mod field;
pub mod schema;
pub mod ty;

pub use constraint::Constraint;
pub use error::SchemaError;
pub use field::Field;
pub use schema::Schema;
pub use ty::Type;
