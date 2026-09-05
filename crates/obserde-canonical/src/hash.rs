use obserde_schema::{Constraint, Field, Schema, Type};
use obserde_value::Document;
use sha2::{Digest, Sha256};

use crate::error::CanonicalisationError;

/// A SHA-256 content hash, identifying either a published `Schema`'s
/// structural definition or a `Document`'s canonical representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hash([u8; 32]);

impl Hash {
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for byte in self.0 {
            s.push_str(&format!("{byte:02x}"));
        }
        s
    }

    pub fn from_hex(s: &str) -> Result<Self, CanonicalisationError> {
        if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CanonicalisationError::InvalidHashHex { input: s.to_string() });
        }
        let mut bytes = [0u8; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
                .map_err(|_| CanonicalisationError::InvalidHashHex { input: s.to_string() })?;
        }
        Ok(Hash(bytes))
    }
}

impl serde::Serialize for Hash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> serde::Deserialize<'de> for Hash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        Hash::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

fn hash_str(s: &str, hasher: &mut Sha256) {
    hasher.update((s.len() as u64).to_be_bytes());
    hasher.update(s.as_bytes());
}

fn hash_opt_f64(v: Option<f64>, hasher: &mut Sha256) {
    match v {
        Some(x) => {
            hasher.update([1u8]);
            hasher.update(x.to_be_bytes());
        }
        None => hasher.update([0u8]),
    }
}

fn hash_opt_u64(v: Option<u64>, hasher: &mut Sha256) {
    match v {
        Some(x) => {
            hasher.update([1u8]);
            hasher.update(x.to_be_bytes());
        }
        None => hasher.update([0u8]),
    }
}

fn hash_type(ty: &Type, hasher: &mut Sha256) {
    match ty {
        Type::Bool => hasher.update([0u8]),
        Type::Integer => hasher.update([1u8]),
        Type::Float => hasher.update([2u8]),
        Type::String => hasher.update([3u8]),
        Type::Bytes => hasher.update([4u8]),
        Type::Timestamp => hasher.update([5u8]),
        Type::List(element) => {
            hasher.update([6u8]);
            hash_type(element, hasher);
        }
        Type::Map(key, value) => {
            hasher.update([7u8]);
            hash_type(key, hasher);
            hash_type(value, hasher);
        }
    }
}

fn hash_constraint(constraint: &Constraint, hasher: &mut Sha256) {
    match constraint {
        Constraint::Range { min, max } => {
            hasher.update([0u8]);
            hash_opt_f64(*min, hasher);
            hash_opt_f64(*max, hasher);
        }
        Constraint::Length { min, max } => {
            hasher.update([1u8]);
            hash_opt_u64(*min, hasher);
            hash_opt_u64(*max, hasher);
        }
        Constraint::Pattern { grammar } => {
            hasher.update([2u8]);
            hash_str(grammar, hasher);
        }
        Constraint::NonEmpty => hasher.update([3u8]),
        Constraint::Custom { name, description } => {
            hasher.update([4u8]);
            hash_str(name, hasher);
            hash_str(description, hasher);
        }
    }
}

fn hash_field(field: &Field, hasher: &mut Sha256) {
    hash_str(field.name(), hasher);
    hash_type(field.ty(), hasher);
    hasher.update([field.is_required() as u8]);
    hasher.update((field.constraints().len() as u64).to_be_bytes());
    for constraint in field.constraints() {
        hash_constraint(constraint, hasher);
    }
}

/// Hashes a `Schema`'s structural definition (contract identity, field
/// order, types, constraints) — identifies a *published schema*,
/// independent of any particular document. Two independently-constructed
/// `Schema`s with the same structure hash identically, which is how
/// "a published schema is immutable" is tested (no storage layer exists
/// yet to enforce it operationally).
pub fn schema_hash(schema: &Schema) -> Hash {
    let mut hasher = Sha256::new();
    hash_str(&schema.contract().to_string(), &mut hasher);
    hasher.update((schema.fields().len() as u64).to_be_bytes());
    for field in schema.fields() {
        hash_field(field, &mut hasher);
    }
    Hash(hasher.finalize().into())
}

fn hash_document(doc: &Document, hasher: &mut Sha256) {
    match doc {
        Document::Null => hasher.update([0u8]),
        Document::Bool(b) => {
            hasher.update([1u8]);
            hasher.update([*b as u8]);
        }
        Document::Integer(i) => {
            hasher.update([2u8]);
            hasher.update(i.to_be_bytes());
        }
        Document::Float(f) => {
            hasher.update([3u8]);
            hasher.update(f.to_be_bytes());
        }
        Document::String(s) => {
            hasher.update([4u8]);
            hash_str(s, hasher);
        }
        Document::Bytes(b) => {
            hasher.update([5u8]);
            hasher.update((b.len() as u64).to_be_bytes());
            hasher.update(b.as_slice());
        }
        Document::List(items) => {
            hasher.update([6u8]);
            hasher.update((items.len() as u64).to_be_bytes());
            for item in items {
                hash_document(item, hasher);
            }
        }
        Document::Map(entries) => {
            hasher.update([7u8]);
            hasher.update((entries.len() as u64).to_be_bytes());
            for (key, value) in entries {
                hash_str(key, hasher);
                hash_document(value, hasher);
            }
        }
    }
}

/// Hashes a `Document`'s canonical representation. Callers are expected to
/// pass the output of `canonicalize()`, not an arbitrary document — this
/// function does not itself normalize ordering, so hashing a
/// non-canonical document does not identify the same content as hashing
/// its canonical form.
pub fn document_hash(canonical_doc: &Document) -> Hash {
    let mut hasher = Sha256::new();
    hash_document(canonical_doc, &mut hasher);
    Hash(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let hash = document_hash(&Document::Integer(42));
        let hex = hash.to_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(Hash::from_hex(&hex).unwrap(), hash);
    }

    #[test]
    fn from_hex_rejects_wrong_length() {
        assert!(Hash::from_hex("abcd").is_err());
    }

    #[test]
    fn from_hex_rejects_non_hex_characters() {
        let bad = "g".repeat(64);
        assert!(Hash::from_hex(&bad).is_err());
    }

    #[test]
    fn structurally_different_documents_hash_differently() {
        let a = Document::Integer(1);
        let b = Document::Integer(2);
        assert_ne!(document_hash(&a), document_hash(&b));
    }

    #[test]
    fn equal_documents_hash_identically() {
        let a = Document::Map(vec![("x".to_string(), Document::Integer(1))]);
        let b = Document::Map(vec![("x".to_string(), Document::Integer(1))]);
        assert_eq!(document_hash(&a), document_hash(&b));
    }
}
