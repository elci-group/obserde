use std::collections::BTreeSet;
use std::fmt;

use obserde_schema::{Constraint, Field, Schema, Type};
use obserde_value::Path;

/// The kind of a single structural difference between two `Schema`s.
///
/// Purely descriptive — carries no opinion about whether the change is
/// safe. `compatibility::classify` is where judgment happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum DiffKind {
    FieldAdded,
    FieldRemoved,
    TypeChanged,
    RequiredChanged,
    ConstraintAdded,
    ConstraintRemoved,
    ConstraintChanged,
    DescriptionChanged,
}

/// One structural difference, located within the schema by `path` (e.g.
/// `.score` for a whole-field change, `.score.maximum` for a narrowed
/// `Range` upper bound).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DiffEntry {
    pub path: Path,
    pub kind: DiffKind,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// The full set of structural differences between two `Schema`s, in a
/// stable, deterministic order (declared field order of `after`, then any
/// fields present only in `before`).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SchemaDiff {
    entries: Vec<DiffEntry>,
}

impl SchemaDiff {
    pub fn entries(&self) -> &[DiffEntry] {
        &self.entries
    }

    pub fn is_identical(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Computes the structural diff from `before` to `after`.
///
/// Same-kind constraints on one field are folded into their *effective
/// envelope* before comparison (e.g. two `Range` constraints on one field
/// are intersected first), not compared by first occurrence —
/// `obserde-validate` ANDs every constraint in a field's list
/// unconditionally, so a narrowing hidden behind a second constraint of
/// the same kind would otherwise be invisible here.
pub fn diff(before: &Schema, after: &Schema) -> SchemaDiff {
    let mut entries = Vec::new();
    let root = Path::root();

    for after_field in after.fields() {
        let path = root.field(after_field.name());
        match before.field(after_field.name()) {
            None => entries.push(DiffEntry {
                path,
                kind: DiffKind::FieldAdded,
                before: None,
                after: Some(render_field(after_field)),
            }),
            Some(before_field) => diff_field(before_field, after_field, &path, &mut entries),
        }
    }

    for before_field in before.fields() {
        if after.field(before_field.name()).is_none() {
            let path = root.field(before_field.name());
            entries.push(DiffEntry {
                path,
                kind: DiffKind::FieldRemoved,
                before: Some(render_field(before_field)),
                after: None,
            });
        }
    }

    SchemaDiff { entries }
}

fn render_field(field: &Field) -> String {
    format!(
        "{} ({})",
        field.ty(),
        if field.is_required() { "required" } else { "optional" }
    )
}

fn diff_field(before: &Field, after: &Field, path: &Path, entries: &mut Vec<DiffEntry>) {
    if !types_equal_ignoring_map_keys(before.ty(), after.ty()) {
        entries.push(DiffEntry {
            path: path.clone(),
            kind: DiffKind::TypeChanged,
            before: Some(before.ty().to_string()),
            after: Some(after.ty().to_string()),
        });
    }

    if before.is_required() != after.is_required() {
        entries.push(DiffEntry {
            path: path.clone(),
            kind: DiffKind::RequiredChanged,
            before: Some(before.is_required().to_string()),
            after: Some(after.is_required().to_string()),
        });
    }

    diff_range(before.constraints(), after.constraints(), path, entries);
    diff_length(before.constraints(), after.constraints(), path, entries);
    diff_non_empty(before.constraints(), after.constraints(), path, entries);
    diff_patterns(before.constraints(), after.constraints(), path, entries);
    diff_customs(before.constraints(), after.constraints(), path, entries);

    if before.description_text() != after.description_text() {
        entries.push(DiffEntry {
            path: path.clone(),
            kind: DiffKind::DescriptionChanged,
            before: before.description_text().map(str::to_string),
            after: after.description_text().map(str::to_string),
        });
    }
}

/// `Type`s are equal for compatibility purposes even when a `Map`'s key
/// type differs — `obserde-validate`'s `validate_type` never inspects a
/// `Map`'s key type (`Document::Map` keys are always strings by
/// construction), so a key-type-only change has zero effect on any
/// `validate()` outcome. Recurses through `List`/`Map` value positions so
/// a key-only change nested arbitrarily deep is still ignored throughout.
fn types_equal_ignoring_map_keys(a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::Map(_, a_value), Type::Map(_, b_value)) => types_equal_ignoring_map_keys(a_value, b_value),
        (Type::List(a_elem), Type::List(b_elem)) => types_equal_ignoring_map_keys(a_elem, b_elem),
        _ => a == b,
    }
}

fn render_opt_f64(v: Option<f64>) -> String {
    v.map_or_else(|| "none".to_string(), |x| x.to_string())
}

fn render_opt_u64(v: Option<u64>) -> String {
    v.map_or_else(|| "none".to_string(), |x| x.to_string())
}

/// Folds every `Range` constraint in `constraints` into one effective
/// `(min, max)` envelope (`min` = the tightest/largest present minimum,
/// `max` = the tightest/smallest present maximum), or `None` if no
/// `Range` constraint is present at all.
pub(crate) fn effective_range(constraints: &[Constraint]) -> Option<(Option<f64>, Option<f64>)> {
    let mut found = false;
    let mut eff_min = None;
    let mut eff_max = None;
    for constraint in constraints {
        if let Constraint::Range { min, max } = constraint {
            found = true;
            if let Some(m) = min {
                eff_min = Some(eff_min.map_or(*m, |cur: f64| cur.max(*m)));
            }
            if let Some(m) = max {
                eff_max = Some(eff_max.map_or(*m, |cur: f64| cur.min(*m)));
            }
        }
    }
    found.then_some((eff_min, eff_max))
}

pub(crate) fn effective_length(constraints: &[Constraint]) -> Option<(Option<u64>, Option<u64>)> {
    let mut found = false;
    let mut eff_min = None;
    let mut eff_max = None;
    for constraint in constraints {
        if let Constraint::Length { min, max } = constraint {
            found = true;
            if let Some(m) = min {
                eff_min = Some(eff_min.map_or(*m, |cur: u64| cur.max(*m)));
            }
            if let Some(m) = max {
                eff_max = Some(eff_max.map_or(*m, |cur: u64| cur.min(*m)));
            }
        }
    }
    found.then_some((eff_min, eff_max))
}

fn has_non_empty(constraints: &[Constraint]) -> bool {
    constraints.iter().any(|c| matches!(c, Constraint::NonEmpty))
}

fn pattern_grammars(constraints: &[Constraint]) -> BTreeSet<String> {
    constraints
        .iter()
        .filter_map(|c| match c {
            Constraint::Pattern { grammar } => Some(grammar.clone()),
            _ => None,
        })
        .collect()
}

fn custom_constraints(constraints: &[Constraint]) -> BTreeSet<(String, String)> {
    constraints
        .iter()
        .filter_map(|c| match c {
            Constraint::Custom { name, description } => Some((name.clone(), description.clone())),
            _ => None,
        })
        .collect()
}

fn diff_range(before: &[Constraint], after: &[Constraint], path: &Path, entries: &mut Vec<DiffEntry>) {
    match (effective_range(before), effective_range(after)) {
        (None, None) => {}
        (None, Some((min, max))) => entries.push(DiffEntry {
            path: path.clone(),
            kind: DiffKind::ConstraintAdded,
            before: None,
            after: Some(format!("Range{{min: {}, max: {}}}", render_opt_f64(min), render_opt_f64(max))),
        }),
        (Some((min, max)), None) => entries.push(DiffEntry {
            path: path.clone(),
            kind: DiffKind::ConstraintRemoved,
            before: Some(format!("Range{{min: {}, max: {}}}", render_opt_f64(min), render_opt_f64(max))),
            after: None,
        }),
        (Some((before_min, before_max)), Some((after_min, after_max))) => {
            if before_min != after_min {
                entries.push(DiffEntry {
                    path: path.field("minimum"),
                    kind: DiffKind::ConstraintChanged,
                    before: Some(render_opt_f64(before_min)),
                    after: Some(render_opt_f64(after_min)),
                });
            }
            if before_max != after_max {
                entries.push(DiffEntry {
                    path: path.field("maximum"),
                    kind: DiffKind::ConstraintChanged,
                    before: Some(render_opt_f64(before_max)),
                    after: Some(render_opt_f64(after_max)),
                });
            }
        }
    }
}

fn diff_length(before: &[Constraint], after: &[Constraint], path: &Path, entries: &mut Vec<DiffEntry>) {
    match (effective_length(before), effective_length(after)) {
        (None, None) => {}
        (None, Some((min, max))) => entries.push(DiffEntry {
            path: path.clone(),
            kind: DiffKind::ConstraintAdded,
            before: None,
            after: Some(format!("Length{{min: {}, max: {}}}", render_opt_u64(min), render_opt_u64(max))),
        }),
        (Some((min, max)), None) => entries.push(DiffEntry {
            path: path.clone(),
            kind: DiffKind::ConstraintRemoved,
            before: Some(format!("Length{{min: {}, max: {}}}", render_opt_u64(min), render_opt_u64(max))),
            after: None,
        }),
        (Some((before_min, before_max)), Some((after_min, after_max))) => {
            if before_min != after_min {
                entries.push(DiffEntry {
                    path: path.field("minimum"),
                    kind: DiffKind::ConstraintChanged,
                    before: Some(render_opt_u64(before_min)),
                    after: Some(render_opt_u64(after_min)),
                });
            }
            if before_max != after_max {
                entries.push(DiffEntry {
                    path: path.field("maximum"),
                    kind: DiffKind::ConstraintChanged,
                    before: Some(render_opt_u64(before_max)),
                    after: Some(render_opt_u64(after_max)),
                });
            }
        }
    }
}

fn diff_non_empty(before: &[Constraint], after: &[Constraint], path: &Path, entries: &mut Vec<DiffEntry>) {
    let before_has = has_non_empty(before);
    let after_has = has_non_empty(after);
    if !before_has && after_has {
        entries.push(DiffEntry {
            path: path.clone(),
            kind: DiffKind::ConstraintAdded,
            before: None,
            after: Some("NonEmpty".to_string()),
        });
    } else if before_has && !after_has {
        entries.push(DiffEntry {
            path: path.clone(),
            kind: DiffKind::ConstraintRemoved,
            before: Some("NonEmpty".to_string()),
            after: None,
        });
    }
}

fn diff_patterns(before: &[Constraint], after: &[Constraint], path: &Path, entries: &mut Vec<DiffEntry>) {
    let before_set = pattern_grammars(before);
    let after_set = pattern_grammars(after);
    for grammar in after_set.difference(&before_set) {
        entries.push(DiffEntry {
            path: path.clone(),
            kind: DiffKind::ConstraintAdded,
            before: None,
            after: Some(format!("Pattern({grammar:?})")),
        });
    }
    for grammar in before_set.difference(&after_set) {
        entries.push(DiffEntry {
            path: path.clone(),
            kind: DiffKind::ConstraintRemoved,
            before: Some(format!("Pattern({grammar:?})")),
            after: None,
        });
    }
}

fn diff_customs(before: &[Constraint], after: &[Constraint], path: &Path, entries: &mut Vec<DiffEntry>) {
    let before_set = custom_constraints(before);
    let after_set = custom_constraints(after);
    for (name, description) in after_set.difference(&before_set) {
        entries.push(DiffEntry {
            path: path.clone(),
            kind: DiffKind::ConstraintAdded,
            before: None,
            after: Some(format!("Custom({name:?}, {description:?})")),
        });
    }
    for (name, description) in before_set.difference(&after_set) {
        entries.push(DiffEntry {
            path: path.clone(),
            kind: DiffKind::ConstraintRemoved,
            before: Some(format!("Custom({name:?}, {description:?})")),
            after: None,
        });
    }
}

impl fmt::Display for SchemaDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let added: Vec<&DiffEntry> = self
            .entries
            .iter()
            .filter(|e| matches!(e.kind, DiffKind::FieldAdded | DiffKind::ConstraintAdded))
            .collect();
        let removed: Vec<&DiffEntry> = self
            .entries
            .iter()
            .filter(|e| matches!(e.kind, DiffKind::FieldRemoved | DiffKind::ConstraintRemoved))
            .collect();
        let changed: Vec<&DiffEntry> = self
            .entries
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    DiffKind::TypeChanged
                        | DiffKind::RequiredChanged
                        | DiffKind::ConstraintChanged
                        | DiffKind::DescriptionChanged
                )
            })
            .collect();

        let mut wrote_section = false;
        if !added.is_empty() {
            writeln!(f, "ADDED")?;
            for entry in &added {
                writeln!(f, "  {}", entry.path)?;
            }
            wrote_section = true;
        }
        if !removed.is_empty() {
            if wrote_section {
                writeln!(f)?;
            }
            writeln!(f, "REMOVED")?;
            for entry in &removed {
                writeln!(f, "  {}", entry.path)?;
            }
            wrote_section = true;
        }
        if !changed.is_empty() {
            if wrote_section {
                writeln!(f)?;
            }
            writeln!(f, "CHANGED")?;
            for entry in &changed {
                writeln!(f, "  {}", entry.path)?;
                if let (Some(before), Some(after)) = (&entry.before, &entry.after) {
                    writeln!(f, "  {before} → {after}")?;
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

    fn contract(version: (u32, u32, u32)) -> Contract {
        Contract::new("elci.test", "fixture", SchemaVersion::new(version.0, version.1, version.2), 0).unwrap()
    }

    fn schema(version: (u32, u32, u32), fields: Vec<Field>) -> Schema {
        Schema::new(contract(version), fields).unwrap()
    }

    #[test]
    fn diff_is_reflexive() {
        let s = schema((1, 0, 0), vec![Field::new("score", Type::Integer)]);
        assert!(diff(&s, &s).is_identical());
    }

    #[test]
    fn field_added() {
        let before = schema((1, 0, 0), vec![]);
        let after = schema((2, 0, 0), vec![Field::new("score", Type::Integer)]);
        let d = diff(&before, &after);
        assert_eq!(d.entries().len(), 1);
        assert_eq!(d.entries()[0].kind, DiffKind::FieldAdded);
        assert_eq!(d.entries()[0].path.to_string(), ".score");
    }

    #[test]
    fn field_removed() {
        let before = schema((1, 0, 0), vec![Field::new("legacy", Type::Integer)]);
        let after = schema((2, 0, 0), vec![]);
        let d = diff(&before, &after);
        assert_eq!(d.entries().len(), 1);
        assert_eq!(d.entries()[0].kind, DiffKind::FieldRemoved);
        assert_eq!(d.entries()[0].path.to_string(), ".legacy");
    }

    #[test]
    fn type_changed() {
        let before = schema((1, 0, 0), vec![Field::new("score", Type::Integer)]);
        let after = schema((2, 0, 0), vec![Field::new("score", Type::Float)]);
        let d = diff(&before, &after);
        assert_eq!(d.entries().len(), 1);
        assert_eq!(d.entries()[0].kind, DiffKind::TypeChanged);
    }

    #[test]
    fn map_key_only_type_change_is_not_a_diff() {
        let before = schema((1, 0, 0), vec![Field::new("scores", Type::map(Type::String, Type::Integer))]);
        let after = schema((2, 0, 0), vec![Field::new("scores", Type::map(Type::Integer, Type::Integer))]);
        assert!(diff(&before, &after).is_identical());
    }

    #[test]
    fn nested_map_key_only_type_change_is_not_a_diff() {
        let before = schema(
            (1, 0, 0),
            vec![Field::new("scores", Type::list(Type::map(Type::String, Type::Integer)))],
        );
        let after = schema(
            (2, 0, 0),
            vec![Field::new("scores", Type::list(Type::map(Type::Integer, Type::Integer)))],
        );
        assert!(diff(&before, &after).is_identical());
    }

    #[test]
    fn required_tightened_and_relaxed() {
        let before = schema((1, 0, 0), vec![Field::new("x", Type::Integer).required(false)]);
        let after = schema((2, 0, 0), vec![Field::new("x", Type::Integer).required(true)]);
        let d = diff(&before, &after);
        assert_eq!(d.entries()[0].kind, DiffKind::RequiredChanged);
        assert_eq!(d.entries()[0].before.as_deref(), Some("false"));
        assert_eq!(d.entries()[0].after.as_deref(), Some("true"));

        assert!(diff(&after, &before).entries()[0].kind == DiffKind::RequiredChanged);
    }

    #[test]
    fn range_added_removed_and_bound_changed() {
        let no_range = schema((1, 0, 0), vec![Field::new("score", Type::Integer)]);
        let with_range = schema(
            (2, 0, 0),
            vec![Field::new("score", Type::Integer).constraint(Constraint::Range { min: Some(0.0), max: Some(100.0) })],
        );
        assert_eq!(diff(&no_range, &with_range).entries()[0].kind, DiffKind::ConstraintAdded);
        assert_eq!(diff(&with_range, &no_range).entries()[0].kind, DiffKind::ConstraintRemoved);

        let narrowed = schema(
            (3, 0, 0),
            vec![Field::new("score", Type::Integer).constraint(Constraint::Range { min: Some(0.0), max: Some(10.0) })],
        );
        let d = diff(&with_range, &narrowed);
        assert_eq!(d.entries().len(), 1);
        assert_eq!(d.entries()[0].kind, DiffKind::ConstraintChanged);
        assert_eq!(d.entries()[0].path.to_string(), ".score.maximum");
        assert_eq!(d.entries()[0].before.as_deref(), Some("100"));
        assert_eq!(d.entries()[0].after.as_deref(), Some("10"));
    }

    #[test]
    fn two_range_constraints_are_folded_into_one_envelope() {
        // Two Range constraints on one field are ANDed by obserde-validate
        // (an intersection), so the diff must compare the *folded*
        // envelope, not just the first constraint in the Vec.
        let before = schema(
            (1, 0, 0),
            vec![Field::new("score", Type::Integer)
                .constraint(Constraint::Range { min: Some(0.0), max: Some(100.0) })
                .constraint(Constraint::Range { min: Some(10.0), max: Some(200.0) })],
        );
        // Effective envelope of `before` is [10, 100] (max of mins, min of maxes).
        let after = schema(
            (2, 0, 0),
            vec![Field::new("score", Type::Integer).constraint(Constraint::Range { min: Some(10.0), max: Some(100.0) })],
        );
        assert!(diff(&before, &after).is_identical());

        let narrower = schema(
            (3, 0, 0),
            vec![Field::new("score", Type::Integer).constraint(Constraint::Range { min: Some(10.0), max: Some(50.0) })],
        );
        let d = diff(&before, &narrower);
        assert_eq!(d.entries().len(), 1);
        assert_eq!(d.entries()[0].path.to_string(), ".score.maximum");
        assert_eq!(d.entries()[0].before.as_deref(), Some("100"));
        assert_eq!(d.entries()[0].after.as_deref(), Some("50"));
    }

    #[test]
    fn length_added_removed_and_bound_changed() {
        let before = schema(
            (1, 0, 0),
            vec![Field::new("summary", Type::String).constraint(Constraint::Length { min: None, max: Some(200) })],
        );
        let after = schema(
            (2, 0, 0),
            vec![Field::new("summary", Type::String).constraint(Constraint::Length { min: None, max: Some(50) })],
        );
        let d = diff(&before, &after);
        assert_eq!(d.entries()[0].path.to_string(), ".summary.maximum");
        assert_eq!(d.entries()[0].before.as_deref(), Some("200"));
        assert_eq!(d.entries()[0].after.as_deref(), Some("50"));
    }

    #[test]
    fn non_empty_added_and_removed() {
        let without = schema((1, 0, 0), vec![Field::new("tags", Type::String)]);
        let with = schema((2, 0, 0), vec![Field::new("tags", Type::String).constraint(Constraint::NonEmpty)]);
        assert_eq!(diff(&without, &with).entries()[0].kind, DiffKind::ConstraintAdded);
        assert_eq!(diff(&with, &without).entries()[0].kind, DiffKind::ConstraintRemoved);
    }

    #[test]
    fn pattern_added_and_removed() {
        let without = schema((1, 0, 0), vec![Field::new("id", Type::String)]);
        let with = schema(
            (2, 0, 0),
            vec![Field::new("id", Type::String).constraint(Constraint::Pattern { grammar: "identifier".to_string() })],
        );
        assert_eq!(diff(&without, &with).entries()[0].kind, DiffKind::ConstraintAdded);
        assert_eq!(diff(&with, &without).entries()[0].kind, DiffKind::ConstraintRemoved);
    }

    #[test]
    fn pattern_grammar_swap_is_added_plus_removed() {
        let before = schema(
            (1, 0, 0),
            vec![Field::new("id", Type::String).constraint(Constraint::Pattern { grammar: "identifier".to_string() })],
        );
        let after = schema(
            (2, 0, 0),
            vec![Field::new("id", Type::String).constraint(Constraint::Pattern { grammar: "email".to_string() })],
        );
        let d = diff(&before, &after);
        assert_eq!(d.entries().len(), 2);
        assert!(d.entries().iter().any(|e| e.kind == DiffKind::ConstraintAdded));
        assert!(d.entries().iter().any(|e| e.kind == DiffKind::ConstraintRemoved));
    }

    #[test]
    fn custom_added_removed_and_changed() {
        let without = schema((1, 0, 0), vec![Field::new("x", Type::Integer)]);
        let with = schema(
            (2, 0, 0),
            vec![Field::new("x", Type::Integer).constraint(Constraint::Custom {
                name: "checksum".to_string(),
                description: "must match".to_string(),
            })],
        );
        assert_eq!(diff(&without, &with).entries()[0].kind, DiffKind::ConstraintAdded);
        assert_eq!(diff(&with, &without).entries()[0].kind, DiffKind::ConstraintRemoved);
    }

    #[test]
    fn description_changed() {
        let before = schema((1, 0, 0), vec![Field::new("x", Type::Integer).description("old")]);
        let after = schema((2, 0, 0), vec![Field::new("x", Type::Integer).description("new")]);
        let d = diff(&before, &after);
        assert_eq!(d.entries().len(), 1);
        assert_eq!(d.entries()[0].kind, DiffKind::DescriptionChanged);
    }

    #[test]
    fn display_groups_by_added_removed_changed() {
        let before = schema(
            (1, 0, 0),
            vec![
                Field::new("legacy_score", Type::Integer),
                Field::new("score", Type::Integer).constraint(Constraint::Range { min: Some(0.0), max: Some(100.0) }),
            ],
        );
        let after = schema(
            (2, 0, 0),
            vec![
                Field::new("provenance", Type::String).required(false),
                Field::new("score", Type::Integer).constraint(Constraint::Range { min: Some(0.0), max: Some(10.0) }),
            ],
        );
        let rendered = diff(&before, &after).to_string();
        assert_eq!(
            rendered,
            "ADDED\n  .provenance\n\nREMOVED\n  .legacy_score\n\nCHANGED\n  .score.maximum\n  100 → 10\n"
        );
    }
}
