use std::fmt;
use std::str::FromStr;

use crate::error::CoreError;

/// An explicit MAJOR.MINOR.PATCH schema version.
///
/// A published schema version is immutable: there is deliberately no
/// in-place mutation method on this type. A new version of a schema is
/// represented by constructing a new `SchemaVersion`, never by mutating an
/// existing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct SchemaVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

impl SchemaVersion {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// Parses a `"MAJOR.MINOR.PATCH"` string, e.g. `"1.4.0"`.
    pub fn parse(s: &str) -> crate::error::Result<Self> {
        let invalid = |reason: &str| CoreError::InvalidVersion {
            input: s.to_string(),
            reason: reason.to_string(),
        };

        let mut parts = s.split('.');
        let major = parts.next().ok_or_else(|| invalid("missing major component"))?;
        let minor = parts.next().ok_or_else(|| invalid("missing minor component"))?;
        let patch = parts.next().ok_or_else(|| invalid("missing patch component"))?;
        if parts.next().is_some() {
            return Err(invalid("too many components, expected MAJOR.MINOR.PATCH"));
        }

        let parse_component = |component: &str| -> crate::error::Result<u32> {
            component
                .parse::<u32>()
                .map_err(|_| invalid(&format!("{component:?} is not a valid non-negative integer")))
        };

        Ok(Self {
            major: parse_component(major)?,
            minor: parse_component(minor)?,
            patch: parse_component(patch)?,
        })
    }

    pub fn major(&self) -> u32 {
        self.major
    }

    pub fn minor(&self) -> u32 {
        self.minor
    }

    pub fn patch(&self) -> u32 {
        self.patch
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for SchemaVersion {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_display_round_trip() {
        let v = SchemaVersion::parse("1.4.0").unwrap();
        assert_eq!(v, SchemaVersion::new(1, 4, 0));
        assert_eq!(v.to_string(), "1.4.0");
    }

    #[test]
    fn from_str_matches_parse() {
        let v: SchemaVersion = "2.10.3".parse().unwrap();
        assert_eq!(v, SchemaVersion::new(2, 10, 3));
    }

    #[test]
    fn ordering_is_numeric_not_lexicographic() {
        let a = SchemaVersion::new(1, 9, 0);
        let b = SchemaVersion::new(1, 10, 0);
        assert!(a < b);
    }

    #[test]
    fn rejects_missing_components() {
        assert!(SchemaVersion::parse("1.4").is_err());
        assert!(SchemaVersion::parse("1").is_err());
    }

    #[test]
    fn rejects_extra_components() {
        assert!(SchemaVersion::parse("1.4.0.1").is_err());
    }

    #[test]
    fn rejects_non_numeric_components() {
        assert!(SchemaVersion::parse("a.b.c").is_err());
        assert!(SchemaVersion::parse("1.4.x").is_err());
    }

    #[test]
    fn rejects_negative_components() {
        assert!(SchemaVersion::parse("-1.4.0").is_err());
    }
}
