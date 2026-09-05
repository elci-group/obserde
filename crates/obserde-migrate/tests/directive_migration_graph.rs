//! Obserde's governing architectural directive (§20, Migration Graph)
//! gives a worked diagram almost verbatim:
//!
//! ```text
//!         v1
//!        /  \
//!       v2  v3
//!        \  /
//!         v4
//! ```
//!
//! This test builds that exact diamond and exercises the planner
//! capabilities the directive lists: available paths, shortest path,
//! missing migration, ambiguous migration.

use obserde_core::{Contract, SchemaVersion};
use obserde_migrate::{MigrationGraph, PlanningError, Migration};
use obserde_schema::{Field, Schema, Type};
use obserde_value::Document;

/// Every version in this diamond carries a "value" field so a real
/// `Document` can be threaded through `execute()` end to end.
fn versioned_schema(version: u32) -> Schema {
    let contract = Contract::new("elci.assessment", "diamond", SchemaVersion::new(version, 0, 0), 0).unwrap();
    Schema::new(contract, vec![Field::new("value", Type::Integer)]).unwrap()
}

fn identity(doc: &Document) -> Result<Document, String> {
    Ok(doc.clone())
}

fn build_diamond() -> MigrationGraph {
    let mut graph = MigrationGraph::new();
    graph
        .register(Migration::new("v1_to_v2", SchemaVersion::new(1, 0, 0), versioned_schema(1), versioned_schema(2), identity))
        .unwrap();
    graph
        .register(Migration::new("v1_to_v3", SchemaVersion::new(1, 0, 0), versioned_schema(1), versioned_schema(3), identity))
        .unwrap();
    graph
        .register(Migration::new("v2_to_v4", SchemaVersion::new(1, 0, 0), versioned_schema(2), versioned_schema(4), identity))
        .unwrap();
    graph
        .register(Migration::new("v3_to_v4", SchemaVersion::new(1, 0, 0), versioned_schema(3), versioned_schema(4), identity))
        .unwrap();
    graph
}

fn diamond_schema_id(version: u32) -> obserde_migrate::SchemaId {
    obserde_migrate::SchemaId::from(versioned_schema(version).contract())
}

#[test]
fn available_paths_finds_both_diamond_routes() {
    let graph = build_diamond();
    let paths = graph.available_paths(&diamond_schema_id(1), &diamond_schema_id(4));
    assert_eq!(paths.len(), 2, "expected exactly the two diamond routes: {paths:#?}");
    assert!(paths.iter().all(|p| p.len() == 2));
}

#[test]
fn plan_v1_to_v4_is_ambiguous() {
    let graph = build_diamond();
    let err = graph.plan(&diamond_schema_id(1), &diamond_schema_id(4)).unwrap_err();
    match err {
        PlanningError::AmbiguousMigration { candidates, .. } => assert_eq!(candidates.len(), 2),
        other => panic!("expected AmbiguousMigration, got {other:?}"),
    }
}

#[test]
fn plan_v1_to_v2_is_the_unique_single_hop() {
    let graph = build_diamond();
    let plan = graph.plan(&diamond_schema_id(1), &diamond_schema_id(2)).unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan.steps()[0].migration.id(), "v1_to_v2");
}

#[test]
fn plan_missing_migration_for_an_unregistered_version() {
    let graph = build_diamond();
    let unregistered = diamond_schema_id(99);
    let err = graph.plan(&diamond_schema_id(1), &unregistered).unwrap_err();
    assert!(matches!(err, PlanningError::MissingMigration { .. }));
}

#[test]
fn plan_v1_to_v1_is_the_zero_step_plan() {
    let graph = build_diamond();
    let plan = graph.plan(&diamond_schema_id(1), &diamond_schema_id(1)).unwrap();
    assert!(plan.is_empty());
}

#[test]
fn executing_one_of_the_two_diamond_routes_end_to_end() {
    let graph = build_diamond();
    let paths = graph.available_paths(&diamond_schema_id(1), &diamond_schema_id(4));
    let doc = Document::Map(vec![("value".to_string(), Document::Integer(42))]);
    let result = paths[0].execute(&doc).unwrap();
    assert_eq!(result, doc); // identity transforms throughout, so the value survives unchanged
}
