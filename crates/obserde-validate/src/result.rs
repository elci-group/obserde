use obserde_value::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

/// One structural or constraint finding against a `Document`, at a
/// specific `Path`, with a stable machine-readable `code`.
///
/// `code` is `String`, not `&'static str`: every call site constructs it
/// from a `'static` literal, but `serde`'s derived `Deserialize` cannot
/// produce a borrowed `&'static str` from an arbitrary-lifetime
/// deserializer, so an owned `String` is what actually round-trips.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValidationIssue {
    pub path: Path,
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

/// The outcome of validating a `Document` against a `Schema`: zero or more
/// `ValidationIssue`s. A result with no `Severity::Error` issues is valid,
/// even if it carries warnings.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct ValidationResult {
    issues: Vec<ValidationIssue>,
}

impl ValidationResult {
    pub fn new(issues: Vec<ValidationIssue>) -> Self {
        Self { issues }
    }

    pub fn is_valid(&self) -> bool {
        !self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    pub fn issues(&self) -> &[ValidationIssue] {
        &self.issues
    }

    pub fn errors(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues.iter().filter(|i| i.severity == Severity::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(severity: Severity) -> ValidationIssue {
        ValidationIssue {
            path: Path::root(),
            code: "test.code".to_string(),
            severity,
            message: "message".to_string(),
            expected: None,
            actual: None,
        }
    }

    #[test]
    fn empty_result_is_valid() {
        assert!(ValidationResult::default().is_valid());
    }

    #[test]
    fn warnings_alone_are_still_valid() {
        let result = ValidationResult::new(vec![issue(Severity::Warning)]);
        assert!(result.is_valid());
        assert_eq!(result.errors().count(), 0);
    }

    #[test]
    fn any_error_makes_result_invalid() {
        let result = ValidationResult::new(vec![issue(Severity::Warning), issue(Severity::Error)]);
        assert!(!result.is_valid());
        assert_eq!(result.errors().count(), 1);
    }
}
