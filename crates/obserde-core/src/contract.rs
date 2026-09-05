use std::fmt;

use crate::error::CoreError;
use crate::version::SchemaVersion;

/// Identity of a data contract: `namespace.name/version+revision`, e.g.
/// `elci.uni.snapshot/1.4.0+2`.
///
/// Contract identity is deliberately independent of Rust module paths,
/// crate names, or type names — renaming a Rust type or moving it between
/// modules MUST NOT change what contract it satisfies.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Contract {
    namespace: String,
    name: String,
    version: SchemaVersion,
    revision: u32,
}

/// A dotted-lowercase identifier segment: starts with a lowercase ASCII
/// letter, followed by lowercase ASCII letters, digits, or underscores.
fn validate_segment(segment: &str, input: &str) -> crate::error::Result<()> {
    let invalid = |reason: String| CoreError::InvalidContract {
        input: input.to_string(),
        reason,
    };

    let mut chars = segment.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        Some(c) => {
            return Err(invalid(format!(
                "segment {segment:?} must start with a lowercase ASCII letter, found {c:?}"
            )))
        }
        None => return Err(invalid("segment must not be empty".to_string())),
    }

    if let Some(c) = chars.find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')) {
        return Err(invalid(format!(
            "segment {segment:?} contains disallowed character {c:?}"
        )));
    }

    Ok(())
}

impl Contract {
    /// Constructs a `Contract`, validating `namespace` and `name` against
    /// the dotted-lowercase identifier grammar.
    pub fn new(
        namespace: impl Into<String>,
        name: impl Into<String>,
        version: SchemaVersion,
        revision: u32,
    ) -> crate::error::Result<Self> {
        let namespace = namespace.into();
        let name = name.into();
        let whole = format!("{namespace}.{name}");

        if namespace.is_empty() {
            return Err(CoreError::InvalidContract {
                input: whole,
                reason: "namespace must not be empty".to_string(),
            });
        }
        for segment in namespace.split('.') {
            validate_segment(segment, &whole)?;
        }
        validate_segment(&name, &whole)?;

        Ok(Self {
            namespace,
            name,
            version,
            revision,
        })
    }

    /// Parses a canonical contract identifier, e.g. `"elci.uni.snapshot/1.4.0+2"`.
    /// The `+revision` suffix is optional and defaults to `0`.
    pub fn parse(s: &str) -> crate::error::Result<Self> {
        let invalid = |reason: &str| CoreError::InvalidContract {
            input: s.to_string(),
            reason: reason.to_string(),
        };

        let mut top = s.splitn(2, '/');
        let path = top.next().ok_or_else(|| invalid("missing namespace.name segment"))?;
        let version_part = top
            .next()
            .ok_or_else(|| invalid("missing '/version' segment"))?;

        let (namespace, name) = path
            .rsplit_once('.')
            .ok_or_else(|| invalid("namespace.name must contain at least two dotted segments"))?;

        let (version_str, revision_str) = match version_part.split_once('+') {
            Some((v, r)) => (v, Some(r)),
            None => (version_part, None),
        };

        let version = SchemaVersion::parse(version_str)
            .map_err(|e| invalid(&format!("invalid version {version_str:?}: {e}")))?;
        let revision = match revision_str {
            Some(r) => r
                .parse::<u32>()
                .map_err(|_| invalid(&format!("{r:?} is not a valid non-negative revision")))?,
            None => 0,
        };

        Self::new(namespace, name, version, revision)
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

    pub fn revision(&self) -> u32 {
        self.revision
    }
}

impl fmt::Display for Contract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}/{}+{}",
            self.namespace, self.name, self.version, self.revision
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_display_round_trip() {
        let c = Contract::parse("elci.uni.snapshot/1.4.0+2").unwrap();
        assert_eq!(c.namespace(), "elci.uni");
        assert_eq!(c.name(), "snapshot");
        assert_eq!(c.version(), SchemaVersion::new(1, 4, 0));
        assert_eq!(c.revision(), 2);
        assert_eq!(c.to_string(), "elci.uni.snapshot/1.4.0+2");
    }

    #[test]
    fn parse_defaults_revision_to_zero() {
        let c = Contract::parse("elci.uni.snapshot/1.4.0").unwrap();
        assert_eq!(c.revision(), 0);
        assert_eq!(c.to_string(), "elci.uni.snapshot/1.4.0+0");
    }

    #[test]
    fn new_matches_parse() {
        let a = Contract::new("elci.uni", "snapshot", SchemaVersion::new(1, 4, 0), 2).unwrap();
        let b = Contract::parse("elci.uni.snapshot/1.4.0+2").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn rejects_missing_version_segment() {
        assert!(Contract::parse("elci.uni.snapshot").is_err());
    }

    #[test]
    fn rejects_single_segment_path() {
        assert!(Contract::parse("snapshot/1.0.0").is_err());
    }

    #[test]
    fn rejects_uppercase_segments() {
        assert!(Contract::parse("Elci.uni.snapshot/1.0.0").is_err());
        assert!(Contract::new("elci.uni", "Snapshot", SchemaVersion::new(1, 0, 0), 0).is_err());
    }

    #[test]
    fn rejects_invalid_characters() {
        assert!(Contract::parse("elci.uni.snap-shot/1.0.0").is_err());
    }

    #[test]
    fn rejects_empty_namespace() {
        assert!(Contract::new("", "snapshot", SchemaVersion::new(1, 0, 0), 0).is_err());
    }
}
