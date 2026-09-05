//! The JSON encoding backend: `Document` <-> JSON text.
//!
//! This crate is deliberately schema-agnostic — its `[dependencies]` never
//! include `obserde-schema`, so `encode`/`decode` cannot take a `&Schema`
//! even if a future change wanted them to. Schema-aware orchestration
//! (validate, canonicalize, then encode; decode, then re-canonicalize and
//! compare) is the caller's job — see `tests/pipeline.rs` for the worked
//! example the directive's Phase 2 goal asks for.
//!
//! `encode`/`decode` are not perfect inverses of each other for every
//! `Document` — see the module docs on `decode` for the specific,
//! deliberate exclusions (bytes, duplicate map keys, out-of-range
//! integers) and why each one is a schema-agnostic-JSON limitation rather
//! than a bug.

pub mod decode;
pub mod encode;
pub mod error;
pub mod limits;

pub use decode::{decode, decode_with_limits};
pub use encode::encode;
pub use error::{DecodeError, EncodeError};
pub use limits::DecodeLimits;
