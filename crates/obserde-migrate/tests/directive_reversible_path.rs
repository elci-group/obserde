//! Directive §18 wants migrations "reversible where possible, explicitly
//! irreversible where not." This test proves reversibility has real
//! payoff for planning: a reversible migration contributes a usable
//! reverse graph edge, and `plan()`'s fewer-reverse-hops tie-break keeps
//! an equally-short all-forward alternative from losing to a path that
//! silently walks a migration backward.

use obserde_core::{Contract, SchemaVersion};
use obserde_migrate::{Migration, MigrationGraph, SchemaId};
use obserde_schema::{Field, Schema, Type};
use obserde_value::Document;

fn schema(version: u32) -> Schema {
    let contract = Contract::new("elci.reversible", "fixture", SchemaVersion::new(version, 0, 0), 0).unwrap();
    Schema::new(contract, vec![Field::new("value", Type::Integer)]).unwrap()
}

fn schema_id(version: u32) -> SchemaId {
    SchemaId::from(schema(version).contract())
}

fn identity(doc: &Document) -> Result<Document, String> {
    Ok(doc.clone())
}

#[test]
fn plan_finds_the_reverse_edge_of_a_reversible_migration() {
    let mut graph = MigrationGraph::new();
    graph
        .register(
            Migration::new("a_to_b", SchemaVersion::new(1, 0, 0), schema(1), schema(2), identity).with_reverse(identity),
        )
        .unwrap();

    let plan = graph.plan(&schema_id(2), &schema_id(1)).unwrap();
    assert_eq!(plan.len(), 1);
    assert!(plan.steps()[0].reverse);
    assert!(plan.to_string().contains("(reverse)"));

    let doc = Document::Map(vec![("value".to_string(), Document::Integer(7))]);
    assert_eq!(plan.execute(&doc).unwrap(), doc);
}

#[test]
fn shortest_path_prefers_fewer_reverse_hops_among_ties() {
    // v1 -> v2 -> v3 (2 forward hops) vs. a reversible v3 -> v1 shortcut
    // walked backward as v1 -> v3 (1 reverse hop) is NOT a tie (1 < 2,
    // the reverse-hop path just wins on length alone). To actually
    // exercise the tie-break, build two length-2 routes from v1 to v4:
    // one all-forward (v1->v2->v4), one that walks a reversible
    // migration backward (v1->v3, then v3<-v4 reversed i.e. v4->v3 is
    // the forward direction, so v1 to v4 via v3 requires walking
    // "v4_to_v3" in reverse).
    let mut graph = MigrationGraph::new();
    graph
        .register(Migration::new("v1_to_v2", SchemaVersion::new(1, 0, 0), schema(1), schema(2), identity))
        .unwrap();
    graph
        .register(Migration::new("v2_to_v4", SchemaVersion::new(1, 0, 0), schema(2), schema(4), identity))
        .unwrap();
    graph
        .register(Migration::new("v1_to_v3", SchemaVersion::new(1, 0, 0), schema(1), schema(3), identity))
        .unwrap();
    graph
        .register(
            Migration::new("v4_to_v3", SchemaVersion::new(1, 0, 0), schema(4), schema(3), identity).with_reverse(identity),
        )
        .unwrap();

    // Two length-2 routes from v1 to v4:
    //   v1 --v1_to_v2--> v2 --v2_to_v4--> v4          (0 reverse hops)
    //   v1 --v1_to_v3--> v3 --v4_to_v3 (reverse)--> v4  (1 reverse hop)
    let plan = graph.plan(&schema_id(1), &schema_id(4)).unwrap();
    assert_eq!(plan.len(), 2);
    assert!(plan.steps().iter().all(|s| !s.reverse), "expected the all-forward route to win the tie-break: {plan:#?}");
}
