use std::collections::{HashMap, HashSet};
use std::fmt;

use obserde_value::Document;

use crate::error::{MigrationError, PlanningError};
use crate::migration::Migration;
use crate::schema_id::SchemaId;

/// One hop in a [`MigrationPlan`]: a migration, and whether it's being
/// walked forward or in reverse.
#[derive(Debug, Clone, Copy)]
pub struct MigrationStep<'g> {
    pub migration: &'g Migration,
    pub reverse: bool,
}

/// An ordered sequence of migration hops from one [`SchemaId`] to
/// another.
#[derive(Debug, Clone)]
pub struct MigrationPlan<'g> {
    steps: Vec<MigrationStep<'g>>,
}

impl<'g> MigrationPlan<'g> {
    pub fn steps(&self) -> &[MigrationStep<'g>] {
        &self.steps
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    fn reverse_count(&self) -> usize {
        self.steps.iter().filter(|s| s.reverse).count()
    }

    /// Threads `doc` through every hop's `apply`/`apply_reverse` in
    /// sequence — every hop gets its own pre/post-validation, not just
    /// the plan's overall endpoints.
    pub fn execute(&self, doc: &Document) -> Result<Document, MigrationError> {
        let mut current = doc.clone();
        for step in &self.steps {
            current = if step.reverse {
                step.migration.apply_reverse(&current)?
            } else {
                step.migration.apply(&current)?
            };
        }
        Ok(current)
    }
}

impl<'g> fmt::Display for MigrationPlan<'g> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.steps.is_empty() {
            return writeln!(f, "(no migration needed)");
        }
        for step in &self.steps {
            let (from, to) = if step.reverse {
                (step.migration.target().contract(), step.migration.source().contract())
            } else {
                (step.migration.source().contract(), step.migration.target().contract())
            };
            let suffix = if step.reverse { " (reverse)" } else { "" };
            writeln!(f, "{from} --{}{suffix}--> {to}", step.migration.id())?;
        }
        Ok(())
    }
}

/// A collection of registered [`Migration`]s, queryable as a directed
/// graph: nodes are [`SchemaId`]s, edges are migrations (plus a reverse
/// edge for every reversible migration).
pub struct MigrationGraph {
    migrations: Vec<Migration>,
    /// Register-time consistency guard: the first `Schema` seen at each
    /// `SchemaId`, and which migration registered it. Makes `SchemaId`'s
    /// revision-dropping assumption safe by rejecting a later migration
    /// whose `Schema` at the same `SchemaId` structurally disagrees,
    /// rather than leaving that an implicit hazard.
    schema_by_id: HashMap<SchemaId, (Vec<obserde_schema::Field>, String)>,
}

impl MigrationGraph {
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
            schema_by_id: HashMap::new(),
        }
    }

    pub fn migrations(&self) -> &[Migration] {
        &self.migrations
    }

    /// Registers `migration`, first checking that its `source`/`target`
    /// `Schema`s don't structurally conflict with any previously
    /// registered `Schema` sharing the same `SchemaId`.
    pub fn register(&mut self, migration: Migration) -> Result<(), MigrationError> {
        self.check_and_record(migration.source(), migration.id())?;
        self.check_and_record(migration.target(), migration.id())?;
        self.migrations.push(migration);
        Ok(())
    }

    fn check_and_record(&mut self, schema: &obserde_schema::Schema, migration_id: &str) -> Result<(), MigrationError> {
        let id = SchemaId::from(schema.contract());
        match self.schema_by_id.get(&id) {
            Some((existing_fields, existing_migration_id)) => {
                if existing_fields.as_slice() != schema.fields() {
                    return Err(MigrationError::SchemaIdConflict {
                        schema_id: id.to_string(),
                        migration_id: migration_id.to_string(),
                        conflicting_migration_id: existing_migration_id.clone(),
                    });
                }
            }
            None => {
                self.schema_by_id.insert(id, (schema.fields().to_vec(), migration_id.to_string()));
            }
        }
        Ok(())
    }

    fn edges_from<'g>(&'g self, node: &SchemaId) -> Vec<(SchemaId, MigrationStep<'g>)> {
        let mut out = Vec::new();
        for migration in &self.migrations {
            let src = SchemaId::from(migration.source().contract());
            let tgt = SchemaId::from(migration.target().contract());
            if &src == node {
                out.push((tgt.clone(), MigrationStep { migration, reverse: false }));
            }
            if migration.is_reversible() && &tgt == node {
                out.push((src.clone(), MigrationStep { migration, reverse: true }));
            }
        }
        out
    }

    /// Enumerates every simple path (no repeated `SchemaId` within one
    /// path — guarantees termination even with cycles from reverse
    /// edges) from `from` to `to`. Includes the trivial 0-step path when
    /// `from == to`, alongside any genuine cycles back to the same node.
    ///
    /// **Cost caveat**: full simple-path enumeration is worst-case
    /// combinatorial on a densely-reversible graph. Fine for the
    /// realistic scale of a schema-version history (tens to low hundreds
    /// of versions); `plan()` does *not* use this function internally for
    /// that reason — see its own doc comment.
    pub fn available_paths(&self, from: &SchemaId, to: &SchemaId) -> Vec<MigrationPlan<'_>> {
        let mut results = Vec::new();
        if from == to {
            results.push(MigrationPlan { steps: Vec::new() });
        }
        let mut visited = HashSet::new();
        visited.insert(from.clone());
        let mut path = Vec::new();
        self.dfs_all(from, to, &mut visited, &mut path, &mut results);
        results
    }

    fn dfs_all<'g>(
        &'g self,
        current: &SchemaId,
        to: &SchemaId,
        visited: &mut HashSet<SchemaId>,
        path: &mut Vec<MigrationStep<'g>>,
        results: &mut Vec<MigrationPlan<'g>>,
    ) {
        for (next_id, step) in self.edges_from(current) {
            if visited.contains(&next_id) {
                continue;
            }
            path.push(step);
            if next_id == *to {
                results.push(MigrationPlan { steps: path.clone() });
            } else {
                visited.insert(next_id.clone());
                self.dfs_all(&next_id, to, visited, path, results);
                visited.remove(&next_id);
            }
            path.pop();
        }
    }

    fn bfs_shortest_length(&self, from: &SchemaId, to: &SchemaId) -> Option<usize> {
        let mut visited = HashSet::new();
        visited.insert(from.clone());
        let mut frontier = vec![from.clone()];
        let mut depth = 0usize;
        while !frontier.is_empty() {
            let mut next_frontier = Vec::new();
            for node in &frontier {
                for (next_id, _step) in self.edges_from(node) {
                    if next_id == *to {
                        return Some(depth + 1);
                    }
                    if visited.insert(next_id.clone()) {
                        next_frontier.push(next_id);
                    }
                }
            }
            depth += 1;
            frontier = next_frontier;
        }
        None
    }

    fn dfs_bounded<'g>(
        &'g self,
        current: &SchemaId,
        to: &SchemaId,
        remaining: usize,
        visited: &mut HashSet<SchemaId>,
        path: &mut Vec<MigrationStep<'g>>,
        results: &mut Vec<MigrationPlan<'g>>,
    ) {
        for (next_id, step) in self.edges_from(current) {
            if visited.contains(&next_id) {
                continue;
            }
            path.push(step);
            if next_id == *to {
                if remaining == 1 {
                    results.push(MigrationPlan { steps: path.clone() });
                }
                // Reached `to` at the wrong depth (shouldn't happen given
                // a correct `bfs_shortest_length`, but not recursed into
                // either way — no point exploring further from `to`).
            } else if remaining > 1 {
                visited.insert(next_id.clone());
                self.dfs_bounded(&next_id, to, remaining - 1, visited, path, results);
                visited.remove(&next_id);
            }
            path.pop();
        }
    }

    /// Finds the shortest migration path from `from` to `to`.
    ///
    /// `from == to` returns the empty 0-step plan immediately, without
    /// invoking any search — it is always strictly shortest (any real
    /// path needs ≥1 hop; a cycle back to the same node needs ≥2 given
    /// the no-repeated-node rule), so it can never tie with anything.
    ///
    /// Otherwise: BFS finds the shortest length `L` (or `MissingMigration`
    /// if `to` is unreachable), then a depth-bounded DFS enumerates only
    /// the paths tied at exactly that length — bounded by `L`, never the
    /// combinatorial cost `available_paths()` documents. Among paths tied
    /// for shortest, the one with fewer reverse hops is preferred (a
    /// migration's rollback direction shouldn't silently outrank an
    /// equally-short all-forward alternative as "the" recommended path);
    /// if a genuine tie remains after that, `AmbiguousMigration` is
    /// returned with all tied candidates attached.
    // PlanningError::AmbiguousMigration carries its full candidate list
    // deliberately (this codebase's "explain the cause" doctrine) rather
    // than boxing it away — this is a rare error path, not a hot one, so
    // the extra stack size clippy flags here is not a real cost.
    #[allow(clippy::result_large_err)]
    pub fn plan(&self, from: &SchemaId, to: &SchemaId) -> Result<MigrationPlan<'_>, PlanningError<'_>> {
        if from == to {
            return Ok(MigrationPlan { steps: Vec::new() });
        }

        let shortest_len = self
            .bfs_shortest_length(from, to)
            .ok_or_else(|| PlanningError::MissingMigration { from: from.clone(), to: to.clone() })?;

        let mut results = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(from.clone());
        let mut path = Vec::new();
        self.dfs_bounded(from, to, shortest_len, &mut visited, &mut path, &mut results);

        let min_reverse = results
            .iter()
            .map(MigrationPlan::reverse_count)
            .min()
            .expect("bfs_shortest_length found a path, so dfs_bounded must find at least one too");
        let mut best: Vec<_> = results.into_iter().filter(|p| p.reverse_count() == min_reverse).collect();

        if best.len() == 1 {
            Ok(best.pop().expect("checked len == 1"))
        } else {
            Err(PlanningError::AmbiguousMigration {
                from: from.clone(),
                to: to.clone(),
                candidates: best,
            })
        }
    }
}

impl Default for MigrationGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use obserde_core::{Contract, SchemaVersion};
    use obserde_schema::{Field, Schema, Type};

    fn schema(version: SchemaVersion, fields: Vec<Field>) -> Schema {
        let contract = Contract::new("elci.test", "graph", version, 0).unwrap();
        Schema::new(contract, fields).unwrap()
    }

    fn id(major: u32) -> SchemaId {
        SchemaId::from(schema(SchemaVersion::new(major, 0, 0), vec![]).contract())
    }

    fn noop(doc: &Document) -> Result<Document, String> {
        Ok(doc.clone())
    }

    fn migration(name: &str, from: u32, to: u32) -> Migration {
        Migration::new(
            name,
            SchemaVersion::new(1, 0, 0),
            schema(SchemaVersion::new(from, 0, 0), vec![]),
            schema(SchemaVersion::new(to, 0, 0), vec![]),
            noop,
        )
    }

    #[test]
    fn register_succeeds_for_consistent_schemas() {
        let mut graph = MigrationGraph::new();
        assert!(graph.register(migration("m1", 1, 2)).is_ok());
        assert_eq!(graph.migrations().len(), 1);
    }

    #[test]
    fn register_detects_schema_id_conflict() {
        let mut graph = MigrationGraph::new();
        graph.register(migration("m1", 1, 2)).unwrap();

        let conflicting_v2 = schema(SchemaVersion::new(2, 0, 0), vec![Field::new("extra", Type::Bool)]);
        let m2 = Migration::new("m2", SchemaVersion::new(1, 0, 0), conflicting_v2, schema(SchemaVersion::new(3, 0, 0), vec![]), noop);
        let err = graph.register(m2).unwrap_err();
        assert!(matches!(err, MigrationError::SchemaIdConflict { .. }));
    }

    #[test]
    fn plan_from_equals_to_is_the_empty_plan() {
        let graph = MigrationGraph::new();
        let plan = graph.plan(&id(1), &id(1)).unwrap();
        assert!(plan.is_empty());
    }

    #[test]
    fn plan_missing_migration() {
        let mut graph = MigrationGraph::new();
        graph.register(migration("m1", 1, 2)).unwrap();
        let err = graph.plan(&id(1), &id(99)).unwrap_err();
        assert!(matches!(err, PlanningError::MissingMigration { .. }));
    }

    #[test]
    fn plan_single_hop() {
        let mut graph = MigrationGraph::new();
        graph.register(migration("m1", 1, 2)).unwrap();
        let plan = graph.plan(&id(1), &id(2)).unwrap();
        assert_eq!(plan.len(), 1);
        assert!(!plan.steps()[0].reverse);
    }
}
