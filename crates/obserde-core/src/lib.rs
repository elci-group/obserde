//! Contract identity and schema versioning primitives.
//!
//! Every other Obserde crate depends on this one. It defines what a data
//! contract *is* (`Contract`) and how its evolution over time is tracked
//! (`SchemaVersion`), independent of any particular schema language, value
//! representation, or encoding.

pub mod contract;
pub mod error;
pub mod version;

pub use contract::Contract;
pub use error::{CoreError, ErrorCode, Result};
pub use version::SchemaVersion;
