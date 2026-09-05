use std::fmt;

use obserde_schema::Schema;
use obserde_value::{Path, PathSegment};

use crate::diff::{diff, effective_length, effective_range, DiffEntry, DiffKind, SchemaDiff};

/// The compatibility verdict for one change, or the aggregate verdict for
/// a whole [`CompatibilityReport`].
///
/// All reasoning in this crate is **structural/hypothetical, not
/// data-driven**: `analyze` never sees real historical documents, only
/// the two `Schema` definitions, matching how real-world Avro/Protobuf
/// compatibility checkers work. "Breaking" means a hypothetical document
/// that satisfied `before` could structurally fail `after` — not that any
/// specific real document has been observed to do so.
///
/// `Unknown` and `ConditionallyCompatible` are real variants (the
/// governing directive requires a 5-state model) that [`analyze`] never
/// currently produces:
/// - `Unknown` would mean "we cannot determine the effect of this
///   change." Every change this crate can detect today maps to a
///   deterministic effect in `obserde-validate` once `Pattern`'s real
///   behavior is accounted for (see `classify_level`'s `ConstraintAdded`
///   handling) — there is no case left that's genuinely unknowable.
/// - `ConditionallyCompatible` would mean "breaking, but a registered
///   migration bridges it." `obserde-migrate` (Phase 4) now exists, but
///   `analyze` does not consult it — nothing here is automatically
///   checked against a real migration registry yet, so this crate still
///   never asserts a bridge exists.
///
/// Both stay in the enum for directive conformance and forward
/// compatibility (a future evaluated non-`"identifier"` grammar, Phase
/// 5's semantic constructs, or wiring `analyze` up to a `MigrationGraph`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum CompatibilityLevel {
    Identical,
    Compatible,
    ConditionallyCompatible,
    Unknown,
    Breaking,
}

/// Severity ordering used to aggregate a report's findings into one
/// overall level. Deliberately an explicit function, not a derived `Ord`
/// on the enum's declaration order: the governing directive's own prose
/// lists "breaking" before "unknown", which would silently invert the
/// real severity relationship if `Ord` were derived naively.
fn severity_rank(level: CompatibilityLevel) -> u8 {
    match level {
        CompatibilityLevel::Compatible => 0,
        CompatibilityLevel::ConditionallyCompatible => 1,
        CompatibilityLevel::Unknown => 2,
        CompatibilityLevel::Breaking => 3,
        CompatibilityLevel::Identical => 0, // never appears as a per-finding level; ranked with Compatible for completeness only
    }
}

/// One classified, explained change — directive §42's Reason/Previous/
/// New/Impact/Required-action shape.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CompatibilityFinding {
    pub path: Path,
    pub level: CompatibilityLevel,
    pub reason: String,
    pub previous: Option<String>,
    pub new: Option<String>,
    pub impact: String,
    pub required_action: Option<String>,
}

/// The full compatibility analysis from `before` to `after`: an aggregate
/// [`CompatibilityLevel`], the underlying [`SchemaDiff`], and one
/// [`CompatibilityFinding`] per diff entry.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CompatibilityReport {
    pub level: CompatibilityLevel,
    diff: SchemaDiff,
    findings: Vec<CompatibilityFinding>,
}

impl CompatibilityReport {
    pub fn diff(&self) -> &SchemaDiff {
        &self.diff
    }

    pub fn findings(&self) -> &[CompatibilityFinding] {
        &self.findings
    }

    pub fn is_compatible(&self) -> bool {
        matches!(self.level, CompatibilityLevel::Identical | CompatibilityLevel::Compatible)
    }
}

/// Computes the diff from `before` to `after`, classifies every entry,
/// and aggregates the result.
pub fn analyze(before: &Schema, after: &Schema) -> CompatibilityReport {
    let schema_diff = diff(before, after);
    let findings: Vec<CompatibilityFinding> = schema_diff
        .entries()
        .iter()
        .map(|entry| describe(entry, before, after))
        .collect();

    let level = if schema_diff.is_identical() {
        CompatibilityLevel::Identical
    } else {
        findings
            .iter()
            .map(|f| f.level)
            .max_by_key(|level| severity_rank(*level))
            .unwrap_or(CompatibilityLevel::Identical)
    };

    CompatibilityReport {
        level,
        diff: schema_diff,
        findings,
    }
}

fn field_name_at(path: &Path, index: usize) -> &str {
    match path.segments().get(index) {
        Some(PathSegment::Field(name)) => name.as_str(),
        _ => "",
    }
}

fn is_minimum_path(path: &Path) -> bool {
    matches!(path.segments().last(), Some(PathSegment::Field(name)) if name == "minimum")
}

fn parse_bound(s: &str, is_minimum: bool) -> f64 {
    if s == "none" {
        if is_minimum {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }
    } else {
        s.parse::<f64>().unwrap_or(if is_minimum { f64::NEG_INFINITY } else { f64::INFINITY })
    }
}

/// The pure classification step: decides a `CompatibilityLevel` from a
/// `DiffEntry` alone, using only the entry's own `kind`/`before`/`after`/
/// `path` — no schema lookups. This is possible because every rendered
/// `before`/`after` string in `diff.rs` is self-authored, crate-internal,
/// unambiguous text (never user input), safe to pattern-match on here.
fn classify_level(entry: &DiffEntry) -> CompatibilityLevel {
    use CompatibilityLevel::{Breaking, Compatible};
    match entry.kind {
        DiffKind::FieldAdded => {
            if entry.after.as_deref().is_some_and(|s| s.ends_with("(required)")) {
                Breaking
            } else {
                Compatible
            }
        }
        DiffKind::FieldRemoved => Breaking,
        DiffKind::TypeChanged => Breaking,
        DiffKind::RequiredChanged => {
            if entry.after.as_deref() == Some("true") {
                Breaking
            } else {
                Compatible
            }
        }
        DiffKind::ConstraintAdded => {
            if entry.after.as_deref().is_some_and(|s| s.starts_with("Custom(")) {
                Compatible
            } else {
                Breaking
            }
        }
        DiffKind::ConstraintRemoved => Compatible,
        DiffKind::ConstraintChanged => {
            // Only ever a numeric .minimum/.maximum bound change (Range
            // or Length — narrowing logic is identical either way, so no
            // need to distinguish which one produced this entry).
            let is_minimum = is_minimum_path(&entry.path);
            let before = parse_bound(entry.before.as_deref().unwrap_or("none"), is_minimum);
            let after = parse_bound(entry.after.as_deref().unwrap_or("none"), is_minimum);
            let narrower = if is_minimum { after > before } else { after < before };
            if narrower {
                Breaking
            } else {
                Compatible
            }
        }
        DiffKind::DescriptionChanged => Compatible,
    }
}

fn required_action_for(level: CompatibilityLevel) -> Option<String> {
    matches!(level, CompatibilityLevel::Breaking | CompatibilityLevel::Unknown).then(|| {
        "A migration registry (obserde-migrate) now exists but this report does not consult it — \
         bridging this change currently requires either a manually-run obserde-migrate migration \
         or a manual, out-of-band data migration."
            .to_string()
    })
}

fn render_bound_f64(v: Option<f64>) -> String {
    v.map_or_else(|| "none".to_string(), |x| x.to_string())
}

fn render_bound_u64(v: Option<u64>) -> String {
    v.map_or_else(|| "none".to_string(), |x| x.to_string())
}

/// Reconstructs directive §42-style `Previous`/`New` text — type plus
/// *full* effective envelope (both bounds), not just the one bound that
/// changed — by looking up the field's real constraints on both schemas.
fn range_or_length_envelope_text(field_name: &str, before: &Schema, after: &Schema) -> (Option<String>, Option<String>) {
    let (Some(before_field), Some(after_field)) = (before.field(field_name), after.field(field_name)) else {
        return (None, None);
    };

    if let (Some(b), Some(a)) = (
        effective_range(before_field.constraints()),
        effective_range(after_field.constraints()),
    ) {
        return (
            Some(format!(
                "{} [{}, {}]",
                before_field.ty(),
                render_bound_f64(b.0),
                render_bound_f64(b.1)
            )),
            Some(format!(
                "{} [{}, {}]",
                after_field.ty(),
                render_bound_f64(a.0),
                render_bound_f64(a.1)
            )),
        );
    }

    if let (Some(b), Some(a)) = (
        effective_length(before_field.constraints()),
        effective_length(after_field.constraints()),
    ) {
        return (
            Some(format!(
                "{} length [{}, {}]",
                before_field.ty(),
                render_bound_u64(b.0),
                render_bound_u64(b.1)
            )),
            Some(format!(
                "{} length [{}, {}]",
                after_field.ty(),
                render_bound_u64(a.0),
                render_bound_u64(a.1)
            )),
        );
    }

    (None, None)
}

fn constraint_added_text(entry: &DiffEntry, field_name: &str) -> (String, String) {
    let after = entry.after.as_deref().unwrap_or("");
    if after.starts_with("Custom(") {
        (
            format!("a Custom constraint was added to field {field_name:?}"),
            "Custom constraints are inspectable but never evaluated by obserde-validate; no \
             validate() outcome can change."
                .to_string(),
        )
    } else if after == "Pattern(\"identifier\")" {
        (
            format!("a Pattern(\"identifier\") constraint was added to field {field_name:?}"),
            "values not matching the identifier grammar, previously unconstrained, will now be \
             rejected."
                .to_string(),
        )
    } else if after.starts_with("Pattern(") {
        (
            format!(
                "a {after} constraint was added to field {field_name:?}, naming a grammar \
                 obserde-validate does not recognize"
            ),
            "obserde-validate will return a hard ValidateError::InvalidPatternGrammar (not a \
             rejected document) for any value present at this path, previously unconstrained."
                .to_string(),
        )
    } else if after.starts_with("NonEmpty") {
        (
            format!("a NonEmpty constraint was added to field {field_name:?}"),
            "empty values, previously accepted, will now be rejected.".to_string(),
        )
    } else {
        (
            format!("a {after} constraint was added to field {field_name:?}"),
            "values previously unconstrained on this dimension may now be rejected.".to_string(),
        )
    }
}

fn describe(entry: &DiffEntry, before: &Schema, after: &Schema) -> CompatibilityFinding {
    let level = classify_level(entry);
    let field_name = field_name_at(&entry.path, 0);

    match entry.kind {
        DiffKind::FieldAdded => CompatibilityFinding {
            path: entry.path.clone(),
            level,
            reason: format!("field {field_name:?} was added"),
            previous: None,
            new: entry.after.clone(),
            impact: if level == CompatibilityLevel::Breaking {
                "documents that satisfied the previous schema without this field will now fail \
                 structural validation (missing required field)."
                    .to_string()
            } else {
                "documents that satisfied the previous schema without this field remain valid; \
                 the field is optional."
                    .to_string()
            },
            required_action: required_action_for(level),
        },
        DiffKind::FieldRemoved => CompatibilityFinding {
            path: entry.path.clone(),
            level,
            reason: format!("field {field_name:?} was removed"),
            previous: entry.before.clone(),
            new: None,
            impact: "documents relying on this field will silently lose it under the new contract."
                .to_string(),
            required_action: required_action_for(level),
        },
        DiffKind::TypeChanged => CompatibilityFinding {
            path: entry.path.clone(),
            level,
            reason: format!("field {field_name:?} changed type"),
            previous: entry.before.clone(),
            new: entry.after.clone(),
            impact: "values of the previous type are not accepted as the new type; documents with \
                      this field will fail structural validation."
                .to_string(),
            required_action: required_action_for(level),
        },
        DiffKind::RequiredChanged => CompatibilityFinding {
            path: entry.path.clone(),
            level,
            reason: format!(
                "field {field_name:?} became {}",
                if entry.after.as_deref() == Some("true") {
                    "required"
                } else {
                    "optional"
                }
            ),
            previous: entry.before.clone(),
            new: entry.after.clone(),
            impact: if level == CompatibilityLevel::Breaking {
                "documents that omitted this optional field will now fail structural validation."
                    .to_string()
            } else {
                "documents that included this field remain valid; it is no longer mandatory."
                    .to_string()
            },
            required_action: required_action_for(level),
        },
        DiffKind::ConstraintAdded => {
            let (reason, impact) = constraint_added_text(entry, field_name);
            CompatibilityFinding {
                path: entry.path.clone(),
                level,
                reason,
                previous: None,
                new: entry.after.clone(),
                impact,
                required_action: required_action_for(level),
            }
        }
        DiffKind::ConstraintRemoved => CompatibilityFinding {
            path: entry.path.clone(),
            level,
            reason: format!("a constraint was removed from field {field_name:?}"),
            previous: entry.before.clone(),
            new: None,
            impact: "removing a constraint only widens what is accepted; no previously-valid \
                      document is affected."
                .to_string(),
            required_action: None,
        },
        DiffKind::ConstraintChanged => {
            let bound = if is_minimum_path(&entry.path) { "minimum" } else { "maximum" };
            let (previous, new) = range_or_length_envelope_text(field_name, before, after);
            CompatibilityFinding {
                path: entry.path.clone(),
                level,
                reason: format!("field {field_name:?}'s {bound} bound changed"),
                previous: previous.or_else(|| entry.before.clone()),
                new: new.or_else(|| entry.after.clone()),
                impact: format!(
                    "existing values previously accepted at the {bound} bound may no longer be \
                     representable."
                ),
                required_action: required_action_for(level),
            }
        }
        DiffKind::DescriptionChanged => CompatibilityFinding {
            path: entry.path.clone(),
            level,
            reason: format!("field {field_name:?}'s description changed"),
            previous: entry.before.clone(),
            new: entry.after.clone(),
            impact: "cosmetic only; no effect on validation.".to_string(),
            required_action: None,
        },
    }
}

fn level_word(level: CompatibilityLevel) -> &'static str {
    match level {
        CompatibilityLevel::Identical => "IDENTICAL",
        CompatibilityLevel::Compatible => "COMPATIBLE",
        CompatibilityLevel::ConditionallyCompatible => "CONDITIONALLY COMPATIBLE",
        CompatibilityLevel::Unknown => "UNKNOWN",
        CompatibilityLevel::Breaking => "BREAKING",
    }
}

impl fmt::Display for CompatibilityReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", level_word(self.level))?;
        for finding in &self.findings {
            match finding.level {
                CompatibilityLevel::Breaking | CompatibilityLevel::Unknown => {
                    writeln!(f)?;
                    writeln!(f, "Reason:")?;
                    writeln!(f, "  {}", finding.path)?;
                    writeln!(f, "  {}", finding.reason)?;
                    if let Some(previous) = &finding.previous {
                        writeln!(f, "Previous:")?;
                        writeln!(f, "  {previous}")?;
                    }
                    if let Some(new) = &finding.new {
                        writeln!(f, "New:")?;
                        writeln!(f, "  {new}")?;
                    }
                    writeln!(f, "Impact:")?;
                    writeln!(f, "  {}", finding.impact)?;
                    if let Some(action) = &finding.required_action {
                        writeln!(f, "Required action:")?;
                        writeln!(f, "  {action}")?;
                    }
                }
                CompatibilityLevel::Compatible | CompatibilityLevel::ConditionallyCompatible | CompatibilityLevel::Identical => {
                    writeln!(f, "  {} — {}", finding.path, finding.reason)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use obserde_core::{Contract, SchemaVersion};
    use obserde_schema::{Constraint, Field, Type};

    fn contract(version: (u32, u32, u32)) -> Contract {
        Contract::new("elci.test", "fixture", SchemaVersion::new(version.0, version.1, version.2), 0).unwrap()
    }

    fn schema(version: (u32, u32, u32), fields: Vec<Field>) -> Schema {
        Schema::new(contract(version), fields).unwrap()
    }

    #[test]
    fn identical_schemas_report_identical() {
        let s = schema((1, 0, 0), vec![Field::new("x", Type::Integer)]);
        let report = analyze(&s, &s);
        assert_eq!(report.level, CompatibilityLevel::Identical);
        assert!(report.is_compatible());
        assert!(report.findings().is_empty());
    }

    #[test]
    fn field_added_required_is_breaking_optional_is_compatible() {
        let before = schema((1, 0, 0), vec![]);
        let required = schema((2, 0, 0), vec![Field::new("x", Type::Integer)]);
        assert_eq!(analyze(&before, &required).level, CompatibilityLevel::Breaking);

        let optional = schema((2, 0, 0), vec![Field::new("x", Type::Integer).required(false)]);
        let report = analyze(&before, &optional);
        assert_eq!(report.level, CompatibilityLevel::Compatible);
        assert!(report.is_compatible());
    }

    #[test]
    fn field_removed_is_always_breaking() {
        let before = schema((1, 0, 0), vec![Field::new("legacy", Type::Integer).required(false)]);
        let after = schema((2, 0, 0), vec![]);
        assert_eq!(analyze(&before, &after).level, CompatibilityLevel::Breaking);
    }

    #[test]
    fn type_changed_is_breaking() {
        let before = schema((1, 0, 0), vec![Field::new("x", Type::Integer)]);
        let after = schema((2, 0, 0), vec![Field::new("x", Type::Float)]);
        assert_eq!(analyze(&before, &after).level, CompatibilityLevel::Breaking);
    }

    #[test]
    fn map_key_only_type_change_is_compatible() {
        let before = schema((1, 0, 0), vec![Field::new("m", Type::map(Type::String, Type::Integer))]);
        let after = schema((2, 0, 0), vec![Field::new("m", Type::map(Type::Integer, Type::Integer))]);
        let report = analyze(&before, &after);
        assert_eq!(report.level, CompatibilityLevel::Identical);
    }

    #[test]
    fn required_tightened_is_breaking_relaxed_is_compatible() {
        let optional = schema((1, 0, 0), vec![Field::new("x", Type::Integer).required(false)]);
        let required = schema((2, 0, 0), vec![Field::new("x", Type::Integer).required(true)]);
        assert_eq!(analyze(&optional, &required).level, CompatibilityLevel::Breaking);
        assert_eq!(analyze(&required, &optional).level, CompatibilityLevel::Compatible);
    }

    #[test]
    fn range_length_non_empty_added_are_breaking() {
        let bare = schema((1, 0, 0), vec![Field::new("x", Type::Integer)]);
        let with_range = schema(
            (2, 0, 0),
            vec![Field::new("x", Type::Integer).constraint(Constraint::Range { min: Some(0.0), max: Some(100.0) })],
        );
        assert_eq!(analyze(&bare, &with_range).level, CompatibilityLevel::Breaking);

        let bare_str = schema((1, 0, 0), vec![Field::new("s", Type::String)]);
        let with_length = schema(
            (2, 0, 0),
            vec![Field::new("s", Type::String).constraint(Constraint::Length { min: None, max: Some(10) })],
        );
        assert_eq!(analyze(&bare_str, &with_length).level, CompatibilityLevel::Breaking);

        let with_non_empty = schema((2, 0, 0), vec![Field::new("s", Type::String).constraint(Constraint::NonEmpty)]);
        assert_eq!(analyze(&bare_str, &with_non_empty).level, CompatibilityLevel::Breaking);
    }

    #[test]
    fn pattern_identifier_added_is_breaking() {
        let bare = schema((1, 0, 0), vec![Field::new("id", Type::String)]);
        let with_pattern = schema(
            (2, 0, 0),
            vec![Field::new("id", Type::String).constraint(Constraint::Pattern { grammar: "identifier".to_string() })],
        );
        assert_eq!(analyze(&bare, &with_pattern).level, CompatibilityLevel::Breaking);
    }

    #[test]
    fn pattern_unrecognized_grammar_added_is_breaking_with_distinct_reason() {
        let bare = schema((1, 0, 0), vec![Field::new("id", Type::String)]);
        let with_pattern = schema(
            (2, 0, 0),
            vec![Field::new("id", Type::String).constraint(Constraint::Pattern { grammar: "email".to_string() })],
        );
        let report = analyze(&bare, &with_pattern);
        assert_eq!(report.level, CompatibilityLevel::Breaking);
        assert!(report.findings()[0].reason.contains("does not recognize"));
    }

    #[test]
    fn custom_constraint_changes_are_always_compatible() {
        let bare = schema((1, 0, 0), vec![Field::new("x", Type::Integer)]);
        let with_custom = schema(
            (2, 0, 0),
            vec![Field::new("x", Type::Integer).constraint(Constraint::Custom {
                name: "checksum".to_string(),
                description: "must match".to_string(),
            })],
        );
        assert_eq!(analyze(&bare, &with_custom).level, CompatibilityLevel::Compatible);
        assert_eq!(analyze(&with_custom, &bare).level, CompatibilityLevel::Compatible);
    }

    #[test]
    fn constraint_removed_is_always_compatible() {
        let with_range = schema(
            (1, 0, 0),
            vec![Field::new("x", Type::Integer).constraint(Constraint::Range { min: Some(0.0), max: Some(100.0) })],
        );
        let bare = schema((2, 0, 0), vec![Field::new("x", Type::Integer)]);
        assert_eq!(analyze(&with_range, &bare).level, CompatibilityLevel::Compatible);
    }

    #[test]
    fn range_narrowed_is_breaking_widened_is_compatible() {
        let wide = schema(
            (1, 0, 0),
            vec![Field::new("score", Type::Integer).constraint(Constraint::Range { min: Some(0.0), max: Some(100.0) })],
        );
        let narrow = schema(
            (2, 0, 0),
            vec![Field::new("score", Type::Integer).constraint(Constraint::Range { min: Some(0.0), max: Some(10.0) })],
        );
        let report = analyze(&wide, &narrow);
        assert_eq!(report.level, CompatibilityLevel::Breaking);
        assert_eq!(report.findings()[0].previous.as_deref(), Some("integer [0, 100]"));
        assert_eq!(report.findings()[0].new.as_deref(), Some("integer [0, 10]"));

        assert_eq!(analyze(&narrow, &wide).level, CompatibilityLevel::Compatible);
    }

    #[test]
    fn description_changed_is_compatible() {
        let before = schema((1, 0, 0), vec![Field::new("x", Type::Integer).description("old")]);
        let after = schema((2, 0, 0), vec![Field::new("x", Type::Integer).description("new")]);
        assert_eq!(analyze(&before, &after).level, CompatibilityLevel::Compatible);
    }

    #[test]
    fn one_breaking_finding_dominates_the_aggregate() {
        let before = schema(
            (1, 0, 0),
            vec![Field::new("a", Type::Integer).description("old"), Field::new("legacy", Type::Integer)],
        );
        let after = schema((2, 0, 0), vec![Field::new("a", Type::Integer).description("new")]);
        // "a" description change is Compatible; removing "legacy" is Breaking.
        assert_eq!(analyze(&before, &after).level, CompatibilityLevel::Breaking);
    }

    #[test]
    fn severity_rank_orders_breaking_above_unknown_above_conditionally_compatible_above_compatible() {
        assert!(severity_rank(CompatibilityLevel::Breaking) > severity_rank(CompatibilityLevel::Unknown));
        assert!(severity_rank(CompatibilityLevel::Unknown) > severity_rank(CompatibilityLevel::ConditionallyCompatible));
        assert!(severity_rank(CompatibilityLevel::ConditionallyCompatible) > severity_rank(CompatibilityLevel::Compatible));
    }

    #[test]
    fn display_renders_breaking_report_with_reason_previous_new_impact() {
        let wide = schema(
            (1, 0, 0),
            vec![Field::new("score", Type::Integer).constraint(Constraint::Range { min: Some(0.0), max: Some(100.0) })],
        );
        let narrow = schema(
            (2, 0, 0),
            vec![Field::new("score", Type::Integer).constraint(Constraint::Range { min: Some(0.0), max: Some(10.0) })],
        );
        let rendered = analyze(&wide, &narrow).to_string();
        assert!(rendered.starts_with("BREAKING\n"));
        assert!(rendered.contains("Reason:\n  .score.maximum\n"));
        assert!(rendered.contains("Previous:\n  integer [0, 100]\n"));
        assert!(rendered.contains("New:\n  integer [0, 10]\n"));
        assert!(rendered.contains("Impact:\n"));
        assert!(rendered.contains("Required action:\n"));
    }

    #[test]
    fn display_renders_compatible_report_as_terse_lines() {
        let before = schema((1, 0, 0), vec![]);
        let after = schema((2, 0, 0), vec![Field::new("provenance", Type::String).required(false)]);
        let rendered = analyze(&before, &after).to_string();
        assert_eq!(rendered, "COMPATIBLE\n  .provenance — field \"provenance\" was added\n");
    }
}
