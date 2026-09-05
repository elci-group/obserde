//! Obserde's governing architectural directive (§16, Compatibility
//! Engine) gives a worked example almost verbatim:
//!
//! ```text
//! Assessment/v1 → Assessment/v2
//! + added optional field: provenance
//! + added required field: evidence
//! ~ changed constraint: score 0..100 → 0..10
//! - removed field: legacy_score
//! Result: BREAKING
//! ```
//!
//! This test builds that exact scenario and asserts `analyze()` reaches
//! the directive's own stated result, with each contributing finding
//! individually verified.

use obserde_compat::{analyze, CompatibilityLevel};
use obserde_core::{Contract, SchemaVersion};
use obserde_schema::{Constraint, Field, Schema, Type};

fn v1() -> Schema {
    let contract = Contract::new("elci.assessment", "assessment", SchemaVersion::new(1, 0, 0), 0).unwrap();
    Schema::new(
        contract,
        vec![
            Field::new("score", Type::Integer).constraint(Constraint::Range { min: Some(0.0), max: Some(100.0) }),
            Field::new("legacy_score", Type::Integer).required(false),
        ],
    )
    .unwrap()
}

fn v2() -> Schema {
    let contract = Contract::new("elci.assessment", "assessment", SchemaVersion::new(2, 0, 0), 0).unwrap();
    Schema::new(
        contract,
        vec![
            Field::new("provenance", Type::String).required(false),
            Field::new("evidence", Type::String).required(true),
            Field::new("score", Type::Integer).constraint(Constraint::Range { min: Some(0.0), max: Some(10.0) }),
        ],
    )
    .unwrap()
}

#[test]
fn directive_section_16_worked_example_is_breaking() {
    let report = analyze(&v1(), &v2());
    assert_eq!(report.level, CompatibilityLevel::Breaking);
    assert!(!report.is_compatible());

    let findings = report.findings();
    assert_eq!(findings.len(), 4, "expected exactly the four changes the directive names: {findings:#?}");

    let provenance = findings.iter().find(|f| f.path.to_string() == ".provenance").unwrap();
    assert_eq!(provenance.level, CompatibilityLevel::Compatible, "added optional field is compatible");

    let evidence = findings.iter().find(|f| f.path.to_string() == ".evidence").unwrap();
    assert_eq!(evidence.level, CompatibilityLevel::Breaking, "added required field is breaking");

    let score_bound = findings.iter().find(|f| f.path.to_string() == ".score.maximum").unwrap();
    assert_eq!(score_bound.level, CompatibilityLevel::Breaking, "narrowed range is breaking");
    assert_eq!(score_bound.previous.as_deref(), Some("integer [0, 100]"));
    assert_eq!(score_bound.new.as_deref(), Some("integer [0, 10]"));

    let legacy_score = findings.iter().find(|f| f.path.to_string() == ".legacy_score").unwrap();
    assert_eq!(legacy_score.level, CompatibilityLevel::Breaking, "removed field is breaking");
}

#[test]
fn reversing_the_transition_is_not_automatically_compatible_either() {
    // v2 -> v1 removes "provenance"/"evidence" (breaking) and widens
    // "score" back to [0,100] (compatible) and re-adds "legacy_score"
    // (breaking, since it's now a newly-added field from v2's
    // perspective) — still Breaking overall, for different reasons.
    let report = analyze(&v2(), &v1());
    assert_eq!(report.level, CompatibilityLevel::Breaking);
}
