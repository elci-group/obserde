use std::fmt;

use obserde_core::{Contract, SchemaVersion};

/// A migration graph node's identity: a `Contract` with its `revision`
/// deliberately dropped.
///
/// Migrations transition between *structurally different* schema
/// versions; `revision` is a non-structural build/implementation stamp on
/// a `Contract` (nothing else in this codebase claims otherwise, but
/// nothing enforces it either — `MigrationGraph::register` makes this
/// assumption safe by rejecting two `Schema`s that share a `SchemaId` but
/// structurally disagree, rather than leaving it an implicit hazard).
/// Two `Contract`s differing only in `revision` are the same graph node.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct SchemaId {
    namespace: String,
    name: String,
    version: SchemaVersion,
}

impl SchemaId {
    pub fn new(namespace: impl Into<String>, name: impl Into<String>, version: SchemaVersion) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
            version,
        }
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> SchemaVersion {
        self.version
    }
}

impl From<&Contract> for SchemaId {
    fn from(contract: &Contract) -> Self {
        Self {
            namespace: contract.namespace().to_string(),
            name: contract.name().to_string(),
            version: contract.version(),
        }
    }
}

impl fmt::Display for SchemaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}/{}", self.namespace, self.name, self.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(revision: u32) -> Contract {
        Contract::new("elci.test", "fixture", SchemaVersion::new(1, 0, 0), revision).unwrap()
    }

    #[test]
    fn from_contract_drops_revision() {
        let id = SchemaId::from(&contract(7));
        assert_eq!(id.namespace(), "elci.test");
        assert_eq!(id.name(), "fixture");
        assert_eq!(id.version(), SchemaVersion::new(1, 0, 0));
    }

    #[test]
    fn contracts_differing_only_in_revision_produce_equal_schema_ids() {
        assert_eq!(SchemaId::from(&contract(0)), SchemaId::from(&contract(1)));
    }

    #[test]
    fn display_format() {
        let id = SchemaId::from(&contract(0));
        assert_eq!(id.to_string(), "elci.test.fixture/1.0.0");
    }
}
