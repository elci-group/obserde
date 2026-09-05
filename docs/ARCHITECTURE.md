# Obserde Architecture (Phase 0 + Phase 1 + Phase 2 + Phase 3 + Phase 4 + Phase 5)

## 1. Purpose & Doctrine

Obserde is a schema-first, versioned, validated **data contract** framework
for Rust. It is not a Serde replacement: serialization is a downstream
concern, not the architectural center. The governing doctrine: data is not
merely something to serialize — it is a contract that exists through time.
A schema has an identity, a version, a validity condition, and (in later
phases) a compatibility and migration story. Obserde exists to make those
things explicit and mechanically checkable.

## 2. Terminology

- **Contract** — the identity of a data contract: `namespace.name/version+revision`. Owned by `obserde-core`.
- **SchemaVersion** — an explicit MAJOR.MINOR.PATCH version, immutable once published. Owned by `obserde-core`.
- **Schema** — a versioned, ordered collection of `Field`s, identified by a `Contract`. Owned by `obserde-schema`.
- **Field** — a named, typed member of a `Schema`, with a required flag and zero or more `Constraint`s. Owned by `obserde-schema`.
- **Type** — the primitive/composite type a `Field`'s value must have (`Bool`, `Integer`, `Float`, `String`, `Bytes`, `Timestamp`, `List`, `Map`). Owned by `obserde-schema`.
- **Constraint** — a value-level rule (`Range`, `Length`, `Pattern`, `NonEmpty`, `Custom`) evaluated during validation, not by the schema itself. Owned by `obserde-schema`.
- **Document** — a format-agnostic decoded value tree; the intermediate representation everything else operates over. Owned by `obserde-value`.
- **Canonical Form** — the deterministic normalization of a `Document` under a `Schema`, from which content hashes are derived. Owned by `obserde-canonical`.
- **Validation** — the process of checking a `Document` against a `Schema`, producing a `ValidationResult`. Owned by `obserde-validate`.

## 3. Contract Model

A `Contract` identifies what a piece of data claims to be:
`namespace.name/version+revision`, e.g. `elci.uni.snapshot/1.4.0+2`.
Namespace and name are dotted-lowercase identifiers, validated at
construction. Contract identity is deliberately decoupled from Rust module
paths, crate names, and type names — renaming or moving a Rust type must
never change what contract it satisfies. One contract identity (a fixed
`namespace.name`) can have many published `SchemaVersion`s over time; the
`Contract` type itself always names one specific version+revision, not a
lineage.

## 4. Schema Model

A `Schema` is a versioned, ordered collection of `Field`s, identified by a
`Contract`. It must be inspectable and hashable without executing any
application code — Phase 1 satisfies this by making `Schema` a plain,
`serde`-serializable Rust value with no behavior tied to code execution.
Field declaration order is preserved (never silently reordered) and is
itself part of what a schema's hash identifies.

Obserde does not standardize on an external schema language (JSON Schema,
protobuf, etc.) in Phase 1. It defines its own minimal, Rust-native schema
vocabulary (`Field`/`Type`/`Constraint`), serialized via `serde_json` for
inspection and fixtures. A dedicated external schema language, if ever
needed, is a later-phase decision.

## 5. Type & Constraint Model

`Type` covers primitives (`Bool`, `Integer`, `Float`, `String`, `Bytes`,
`Timestamp`) and composites (`List<T>`, `Map<K, V>`). `Constraint` covers
`Range` (numeric bounds), `Length` (string/collection size bounds),
`Pattern` (a named grammar a string must match — Phase 1 recognizes only
`"identifier"`), `NonEmpty`, and `Custom` (an inspectable escape hatch,
not evaluated in Phase 1). Constraint *evaluation* belongs to
`obserde-validate`, not to the schema types themselves — a `Schema` can be
constructed and inspected without ever validating anything.

`Map`'s key type is currently informational only: `obserde-value::Document`
represents map keys as `String` unconditionally, so a declared non-`String`
key type has nothing independent to check against yet.

## 6. Versioning Model

`SchemaVersion` is an explicit `major.minor.patch` triple. There is no
in-place mutation API on `SchemaVersion` or `Schema` — evolving a schema
means constructing a new `Schema` value with a new `SchemaVersion` inside
its `Contract`, never mutating an existing one. "Published" has no
storage-layer meaning yet in Phase 1 (there is no registry); immutability
is instead demonstrated as a property: two independently-constructed
`Schema` values with identical structure are `PartialEq`-equal and hash
identically (see §15).

## 7. Value Model

`Document` is the format-agnostic decoded value tree: `Null`, `Bool`,
`Integer`, `Float`, `String`, `Bytes`, `List`, `Map`. It is explicitly
neither a wire format (JSON/TOML bytes — `obserde-json`'s concern, §11)
nor a Rust domain struct (the application's own type) — see the
directive's "wire types vs. domain types" separation. `Document::Map` stores
`Vec<(String, Document)>`, not a `HashMap`/`BTreeMap`, so original key
order and duplicate keys remain structurally observable; canonical key
ordering is `obserde-canonical`'s explicit job, not baked into storage.

## 8. Validation Model

`obserde-validate::validate(schema, doc)` checks two things:

- **Structural**: every required field is present; every present field's
  value matches its declared `Type`, recursively through `List`/`Map`.
- **Constraint**: each field's `Constraint`s are evaluated against its
  value (`Range`, `Length`, `Pattern("identifier")`, `NonEmpty`).

**Semantic validation** — whether a value is *meaningful* against an
ontology — lives in `obserde-padagonia` (§14) as a fully independent
sibling pass, not an extension of this crate: `obserde-validate` has no
dependency on `obserde-padagonia`, and vice versa. See §14 for why the two
are kept structurally separate rather than composed into one pipeline.

Findings are `ValidationIssue { path, code, severity, message, expected,
actual }` inside a `ValidationResult`, not bare strings — `path` locates
the finding within the `Document` (e.g. `.scores.alice[2]`), `code` is a
stable machine-readable string, and `severity` distinguishes `Error` from
`Warning` (only `Error` makes a result invalid).

## 9. Canonicalisation Model

`obserde-canonical::canonicalize(schema, doc)` produces a deterministic
normal form:

- Top-level map keys ordered by declared field order, then any remaining
  keys lexicographically; nested maps (with no schema-declared order) are
  ordered lexicographically throughout.
- `-0.0` collapses to `0.0`; `NaN` passes through unchanged.
- Explicit `Null` entries are preserved, never dropped or added.
- List order is preserved as-is.

String values, including ones holding a timestamp, pass through
unchanged in Phase 1 — real Unicode (NFC) normalization and
timestamp-to-UTC reformatting are deferred until a real
Unicode-normalization/date-time dependency is introduced, rather than
hand-rolling that arithmetic now. `document_hash()` hashes a `Document`
assumed to already be canonical; it does not itself normalize ordering.

## 10. Error Model

Every crate defines one `thiserror`-derived `<Area>Error` enum (e.g.
`SchemaError`, `ValidateError`, `CanonicalisationError`), matching the
convention used across the ELCI Group's other Rust tools. Each implements
`obserde_core::ErrorCode`, returning a short, stable, dotted string (e.g.
`"schema.field.duplicate"`, `"validate.constraint.range"`) rather than a
numeric registry code. This replaces the numeric scheme sketched in
Obserde's original architectural directive (e.g. "OBV1021") — no such
registry exists anywhere in the ELCI family, and a stable dotted string is
lower-overhead while remaining just as machine-filterable. Once shipped, a
variant's code is part of that crate's public contract and must not
change.

`ValidateError` (a Rust error type — conditions that prevent validation
from running at all, like an unrecognized `Pattern` grammar) is
deliberately named distinctly from `ValidationIssue` (a struct — one
finding inside a successful `ValidationResult`), to avoid the ambiguity in
the original directive's shared name `ValidationError` for both concepts.

## 11. Encoding Model

`obserde-json::encode`/`decode` are the first, and so far only, encoding
backend (directive §23). They are schema-agnostic by construction — the
crate's `[dependencies]` never include `obserde-schema`, so it is
structurally impossible for either function to take a `&Schema`. Schema-
aware orchestration (validate, canonicalize, encode; decode, then
re-canonicalize and compare) is the caller's job, demonstrated end-to-end
in `crates/obserde-json/tests/pipeline.rs` — directive §44's Phase 2 goal
("Demonstrate: schema → validation → canonicalisation → encoding →
decoding → hashing").

`encode` is total over `Null`/`Bool`/`Integer`/`Float`/`String`/`List`/
`Map`; `Bytes` becomes a base64 string (`base64`'s standard, padded
alphabet, pinned explicitly). It rejects non-finite floats
(`EncodeError::NonFiniteFloat`) rather than encoding them, because
`serde_json`'s default behavior for `NaN`/infinite `f64` is to silently
emit JSON `null` — an undetectable, data-corrupting encode that would
contradict Obserde's typed, path-bearing error philosophy.

`decode`/`decode_with_limits` parse via `serde_json::from_str::<Value>`
(covered by `serde_json`'s own hard-coded 128-deep recursion guard) then
walk the result into a `Document`, enforcing a configurable
`DecodeLimits`. **Stated plainly, not implied to be stronger than it
is**: `max_input_bytes` is checked before parsing and is the real,
allocation-bounding resource-exhaustion defense (directive §25); plain
JSON has no entity-expansion mechanism, so the wire-bytes-to-parsed-tree
amplification factor is small and constant, meaning `max_input_bytes`
alone already gives a fixed worst-case memory bound. `max_string_len` and
`max_collection_len` are checked only *after* `serde_json` has fully
materialized the parsed tree — they are post-parse shape/policy
rejections, not independent pre-allocation guards. `max_depth` *is*
enforced cheaply pre-blowup, riding on `serde_json`'s own 128-cap, and
cannot usefully be set above 128 (raising it further would require
enabling `serde_json`'s `unbounded_depth` feature, which must never be
enabled — it exists specifically to opt out of that DoS guard).

JSON has one number grammar; `Document` has two variants (`Integer`,
`Float`). `decode` distinguishes them via `serde_json::Number::is_i64()` /
`is_u64()`: an integer-looking literal that overflows `i64` but still
fits `u64` is rejected explicitly (`DecodeError::IntegerOutOfRange`)
rather than silently downgraded to an approximate `Float` — the decode-side
mirror of `encode`'s `NonFiniteFloat` rejection. There is no decode-side
check for non-finite values themselves: JSON text has no lexical spelling
for NaN/Infinity, and `serde_json`'s own parser already rejects a numeric
literal whose exponent would overflow to infinity, surfacing as
`DecodeError::Syntax` before `obserde-json`'s own logic runs.

**`decode(encode(x)) == x` does not hold universally** (directive §34's
"where semantically appropriate" is the explicit escape hatch this uses,
not a license to skip testing — see the scoped `proptest` in
`crates/obserde-json/tests/roundtrip.rs`). Three documented exclusions:

1. **`Document::Bytes`** always decodes back as `Document::String` (the
   base64 text) — a schema-agnostic decoder cannot know a bare JSON
   string should become `Bytes` instead; only a schema-aware caller can
   (see `tests/pipeline.rs`'s `bytes_field_does_not_round_trip_through_schema_agnostic_decode`,
   which turns this from prose into an executable regression test).
2. **Duplicate map keys** collapse to last-value-wins already at *encode*
   time, not just decode: `serde_json::Map` (backing `Value::Object`) is a
   genuine map and cannot represent duplicate keys, even with the
   `preserve_order` feature enabled (that feature preserves insertion
   *order*, not duplicate *entries*). A `Document::Map` with duplicate
   keys never round-trips through JSON text in either direction.
3. **Integer literals outside `i64::MIN..=i64::MAX`** are rejected
   explicitly within the recoverable `(i64::MAX, u64::MAX]` band (above);
   literals beyond `u64::MAX` are already lossily approximated to `f64`
   inside `serde_json` itself before `obserde-json`'s code runs at all (no
   `arbitrary_precision` feature enabled) — an unrecoverable decode-
   *correctness* limitation affecting any sufficiently large input, not
   just a round-trip concern for values that originated as
   `Document::Integer(i64)` (always in range by construction).

The workspace enables `serde_json`'s `preserve_order` feature (`Value`
preserves original JSON key order — a no-op for the typed
`Schema`/`Document` (de)serialization Phase 0/1 already used, verified by
running the pre-existing 64 tests unchanged after enabling it) and
`float_roundtrip` feature — the latter not optional: `serde_json`'s
default float parser is a fast approximation that is not always bit-exact,
found empirically via `roundtrip.rs`'s property test failing on a
generated value without it.

## 12. Compatibility Semantics

`obserde-compat` implements schema diff and compatibility analysis in two
stages, mirroring the structural/semantic split `obserde-validate` already
uses for documents:

- **`diff(before, after) -> SchemaDiff`** (structural, no judgment) lists
  what changed: fields added/removed, a field's `Type` changed
  (ignoring a `Map`'s key-type component — see below), `required` toggled,
  a constraint added/removed/changed, or a description changed. Same-kind
  constraints on one field are folded into their *effective envelope*
  before comparison (e.g. two `Range` constraints are intersected first),
  not compared by first occurrence: `obserde-validate` ANDs every
  constraint in a field's list unconditionally, so a narrowing hidden
  behind a field's second same-kind constraint would otherwise be
  invisible to the diff. A `Map`'s key-type parameter is deliberately
  ignored everywhere in the type tree (recursively, including nested
  inside `List`/`Map`) — `validate_type`'s `Map` arm never inspects it
  (§5), so a key-type-only change has zero effect on any `validate()`
  outcome and isn't a real structural difference for this purpose.
- **`analyze(before, after) -> CompatibilityReport`** (judgment on top of
  the diff) classifies every `DiffEntry` into a `CompatibilityLevel` and
  aggregates them.

`CompatibilityLevel` has five states, matching the directive's required
model: `Identical`, `Compatible`, `ConditionallyCompatible`, `Unknown`,
`Breaking`. All classification is **structural/hypothetical, not
data-driven** — `analyze` never sees real historical documents, only the
two `Schema` definitions (the same approach real-world Avro/Protobuf
compatibility checkers use). "Breaking" means a hypothetical document that
satisfied `before` could structurally fail `after`, not that any specific
real document has been observed to do so.

**`Unknown` and `ConditionallyCompatible` are real variants that
`analyze` never currently produces.** This is deliberate, not an
oversight:

- `Unknown` would mean "we cannot determine the effect of this change."
  Every diff kind this crate detects maps to a deterministic effect in
  `obserde-validate` today. In particular, `Constraint::Pattern` is
  **not** treated as unknowable just because most grammar strings are
  symbolic: `validator.rs` evaluates `"identifier"` with real semantics
  (classified the same as `Range`/`Length`) and treats every other
  grammar name as a **hard error** (`ValidateError::InvalidPatternGrammar`)
  — a fully deterministic, worse-than-Breaking effect, not an unknowable
  one. Both cases are classified `Breaking`, with distinct reason text for
  the hard-error case.
- `ConditionallyCompatible` would mean "breaking, but a registered
  migration bridges it." `obserde-migrate` (§13) now exists, but `analyze`
  does not consult it — nothing here is automatically checked against a
  real `MigrationGraph` yet.

Both stay in the enum for directive conformance and forward-compatibility
(a future evaluated non-`"identifier"` grammar, Phase 5's semantic
constructs, or wiring `analyze` up to a `MigrationGraph`).

Classification summary: adding a required field, removing any field,
changing a field's type (beyond the ignored `Map`-key case), tightening
`required` from optional to mandatory, adding a `Range`/`Length`/
`NonEmpty`/`Pattern` constraint, and narrowing a `Range`/`Length` envelope
are all `Breaking`. Adding an optional field, relaxing `required`, adding
or changing a `Custom` constraint, removing any constraint, widening a
`Range`/`Length` envelope, and changing a description are all
`Compatible` — `Custom` constraints are classified `Compatible` (not
`Unknown`) specifically because they have zero enforcement effect in
`obserde-validate` today (confirmed no-op), so no `validate()` outcome can
possibly change regardless of what a `Custom` constraint is edited to say.

`CompatibilityFinding`s carry directive §42's Reason/Previous/New/Impact/
Required-action shape. `required_action` for `Breaking`/`Unknown` findings
is honest, generic text pointing at `obserde-migrate` existing-but-
unconsulted rather than a fabricated migration ID — the directive's own
§42 example cites a concrete `M-0047` that has no referent in this
codebase.

## 13. Migration Model

`obserde-migrate` implements directive §18-§20: migration definition,
execution with pre/post-validation, and graph-based planning. It is the
first crate in this workspace to depend on `obserde-validate` as a real
(non-dev) dependency — `Migration::apply`/`apply_reverse` need real
`validate()` calls to enforce "no silent migrations."

**`SchemaId`** is the migration graph's node identity: a `Contract` with
its `revision` deliberately dropped, since migrations transition between
*structurally different* schema versions and `revision` is treated as a
non-structural build/implementation stamp — an assumption nothing else in
this codebase enforces, made safe here (not left an implicit hazard) by
`MigrationGraph::register`'s consistency check: registering a `Schema`
whose `fields()` disagree with a previously-registered `Schema` sharing
the same `SchemaId` is a hard `SchemaIdConflict` error, turning what would
otherwise be a confusing `execute()`-time failure two hops later into an
immediate, precise authoring error.

**`Migration`** holds a source `Schema`, a target `Schema`, its own
identity and version (independent of the schema versions it bridges,
directive §18), a `ValidationPolicy` (`Strict` or `PostOnly`), a forward
transform, and an `Option`al reverse transform — modeling reversibility as
a *capability*, not a bare flag: `None` means genuinely irreversible,
`Some(f)` means `f` is the real reverse transform. Both directions share
one execution flow (pre-validate → transform → post-validate, directive
§19), with one deliberate asymmetry: **`apply_reverse` always validates
`Strict`, regardless of the migration's declared policy** — the
rollback/undo path is rarer and lower-trust than the forward hot path, so
skipping its pre-validation isn't offered. Post-validation is never
skippable in either direction under any policy — that's the concrete "no
silent migrations" enforcement point, proven by a dedicated test (a
`PostOnly` migration whose transform produces a target-schema-violating
document must still hard-fail). "Fail atomically" (§19) falls out of
Rust's ownership model for free: a failed `Result` never mutates anything
the caller can observe.

**`MigrationGraph`** treats each registered `Migration` as a directed edge
`SchemaId::from(source) -> SchemaId::from(target)`, plus a reverse edge
whenever the migration `is_reversible()` — giving reversibility real
payoff for planning, not just metadata. `available_paths()` enumerates
every simple path (no repeated `SchemaId`, guaranteeing termination even
with cycles from reverse edges) between two `SchemaId`s, with a documented
worst-case-combinatorial cost caveat on a densely-reversible graph.
`plan()` does not reuse `available_paths()`: it runs BFS to find the
shortest length, then a depth-bounded DFS to enumerate only the
paths tied at that exact length — bounded by the shortest length itself,
never `available_paths()`'s theoretical blowup. `from == to` returns the
empty 0-step plan immediately, without invoking any search (always
strictly shortest, so it can never spuriously tie). Among paths tied for
shortest, `plan()` prefers fewer reverse hops as a secondary tie-break,
so a migration's rollback direction doesn't silently outrank an
equally-short all-forward alternative as "the" recommended path; a
genuine remaining tie is `PlanningError::AmbiguousMigration`, carrying all
tied candidates (this codebase's "explain the cause" doctrine). No path
at all is `PlanningError::MissingMigration`.

**Directive §20's "trusted path" and "incompatible path" planner
capabilities are explicitly not implemented.** There is no trust/signing/
provenance concept anywhere else in this codebase to hang "trusted" off
of, and every registered `Migration` is a binary yes/no graph edge, with
no partial-compatibility grading to distinguish "incompatible" from
simply "missing." This mirrors how §12 documents `CompatibilityLevel::
Unknown`/`ConditionallyCompatible` as reserved-but-unproduced rather than
silently omitted.

## 14. Semantic Model

`obserde-padagonia` implements directive §44 Phase 5: semantic
identifiers, ontology references, semantic validation, "maintain clean
separation between structural and semantic concerns." It is a sibling of
`obserde-compat` in the dependency graph (`obserde-core`, `obserde-schema`,
`obserde-value` — nothing else): despite both `obserde-validate` and
`obserde-padagonia` having "validation" in their purpose, there is zero
dependency edge between them. Structural and semantic validation are
independent sibling passes a caller can run in either order or skip
either one, not stages of one pipeline — enforced at the dependency-graph
level, the same way `obserde-json` enforces "schema-agnostic" by simply
not depending on `obserde-schema`.

**Why this crate has no dependency on the real Padagonia project.** The
actual Padagonia project (a separate git repository at a sibling path) was
inspected directly before designing this crate: it is a full
"ontology-native, immutable, provenance-rich graph store" with an HTTP
server (`axum`, `tokio`, `tower-http`), a metrics exporter, and vector
search (`fast-hnsw`) — and it depends on `bound-core` via an unpublished,
unpinned, cross-repo relative path (`{ path = "../bound/crates/bound-core" }`)
that only resolves if a sibling `bound` repository happens to be checked
out on the same machine. Its `stable_external_id()` function produces
opaque hash strings (`"{kind}_{32 hex digits}"`), not the human-readable
dotted identifiers the directive's own illustrative example uses
(`UNI.Assessment.Score`) — that example is architectural illustration, not
a literal format spec. A second Padagonia ontology model addresses its own
graph nodes by plain, model-specific `&str` — there is no single canonical
Padagonia identifier type to bind against, even within Padagonia itself.
Taking a real dependency (in `[dependencies]` *or* `[dev-dependencies]`)
would buy zero type-safety in exchange for a 10x+ dependency-footprint
increase and a fragile cross-repo coupling. This is the literal reading of
directive §6's "Padagonia integration SHOULD nevertheless be optional at
the lowest framework layer so Obserde remains independently usable," not
a shortfall.

**`SemanticId`** is a validated stable identifier (dot-separated segments,
each starting with an ASCII letter of either case, followed by letters/
digits/underscores) — deliberately more permissive than `Contract`'s
strict lowercase-only grammar so it accepts both the directive's
PascalCase illustrative style and Padagonia's real opaque hash format.
**`FieldSemantics`**/**`SemanticAnnotations`** associate a `Schema` field
name with a concept `SemanticId` plus outgoing `(relation, target)` pairs
(directive §6's `measures →`, `represents →` edges) — entirely external to
`Schema`/`Field`, never merged into Phase 0's types, so "clean separation"
is a literal type-level fact, not just a convention. Directive §6's
`range → 0..100` edge is deliberately *not* modeled as a relation: it's
already fully covered by `Constraint::Range` from Phase 0, and the
directive's own diagram illustrates Padagonia and Obserde describing one
concept from complementary angles, not asking this crate to re-encode a
value constraint Obserde already enforces structurally.

**`SemanticResolver`** is the pluggable boundary to an ontology authority
(`exists`, `relation_permitted`, both fallible — a real implementation
involves I/O that can genuinely fail). **`StaticResolver`** is the only
implementation this phase ships: a genuinely usable in-memory resolver,
not a test-only mock. **`validate_semantic(schema, annotations, resolver,
doc)`** has a two-part failure model, deliberately *not* symmetric with
`obserde-validate::validate`'s `ValidateError`:

1. **Hard `Err(SemanticError::UnknownAnnotatedField)`**, checked up front
   before touching the resolver or document at all — `SemanticAnnotations`
   naming a field absent from `Schema` is a deterministic authoring
   mistake (same schema+annotations fail the same way every call),
   exactly analogous to `ValidateError::InvalidPatternGrammar`.
2. **Soft `SemanticIssue`s** for everything else — an unknown concept, an
   unknown relation target, an unpermitted relation, or the resolver
   itself failing. A resolver `Err` is plausibly transient live I/O,
   unlike case 1's deterministic mistake, so it does not abort the call —
   matching `validate_field`'s own precedent (runs every constraint
   unconditionally even after a type mismatch, collecting every finding
   rather than stopping at the first). Concretely: `exists(target)`
   failing does not skip the following `relation_permitted(...)` check
   for the same relation — both run unconditionally, so a target-unknown
   issue and a not-permitted issue can legitimately co-occur and both
   surface.

An unannotated field, or an annotated field absent from the document, is
silently skipped — that's structural `validate()`'s job to flag, not
semantic's; `validate_semantic` assumes nothing about whether structural
validation has already run.

**What's deliberately not implemented.** Nothing here cross-checks a
field's own `Constraint::Range` against what a resolver might
independently know about the same concept's valid range (Padagonia says
`0..100`, the `Field` says `0..50` — undetected). The concrete shape a
future increment would take: a `SemanticResolver::declared_range(id) ->
Option<(f64, f64)>` method cross-checked in `validate_semantic` — not
built now, since it only covers `Range` (not directive §13's general
"meaningful value" idea), adds trait surface for one narrow case, and
risks quietly reintroducing the structural/semantic coupling this crate's
boundary is designed to avoid. Directive §13's broader "value is
meaningful within its declared semantic type" is likewise not
implemented — no generic way to ask this exists without a much deeper
ontology integration than an existence/permission check can express.
Documented honestly as deferred, the same way §12 documents
`CompatibilityLevel::Unknown`/`ConditionallyCompatible` and §13 documents
"trusted path"/"incompatible path" as reserved-but-unproduced rather than
faked.

## 15. Formal Invariants

Four invariants from Phase 1 remain tested in this codebase, plus one new
one each from Phases 2 and 3:

1. **A valid document satisfies its schema** — `obserde-validate`'s
   positive-path tests, including the shared `fixtures/contract-example/`
   fixture.
2. **A canonical document is idempotent** —
   `canonicalize(schema, canonicalize(schema, x)) == canonicalize(schema, x)`,
   tested both as a concrete example and via `proptest` over arbitrary
   bounded `Document` trees (`crates/obserde-canonical/tests/idempotency.rs`).
3. **A published schema is immutable** — there is no mutation API on
   `Schema`/`SchemaVersion`; two independently-constructed `Schema` values
   with identical structure are `PartialEq`-equal and produce the same
   `schema_hash()`.
4. **A canonical hash identifies a canonical representation** — the golden
   fixture test asserts `document_hash(canonicalize(schema, valid)) ==`
   the checked-in expected hash, and a unit test confirms structurally
   different documents hash differently.
5. **`decode(encode(x)) == x`, scoped** — `crates/obserde-json/tests/roundtrip.rs`'s
   `proptest`, over a `Document` strategy deliberately excluding `Bytes`
   and duplicate-keyed maps (§11's three documented exclusions), plus the
   concrete happy-path demonstration in `tests/pipeline.rs`.
6. **Diff is reflexive** — `diff(S, S)` is always empty and
   `analyze(S, S).level == Identical`, for any `Schema` `S`
   (`crates/obserde-compat/src/diff.rs`'s `diff_is_reflexive` test).
7. **A successful migration produces a document valid under its target
   schema** — `Migration::apply`/`apply_reverse` always post-validate,
   under every `ValidationPolicy`, in both directions; proven by a
   dedicated test where a deliberately-buggy transform must hard-fail
   rather than silently succeed (`crates/obserde-migrate/src/migration.rs`'s
   `post_validation_failed_is_the_no_silent_migrations_proof` test).

## 16. Crate Map

Real:

- `obserde-core` — `Contract`, `SchemaVersion`, the `ErrorCode` convention.
- `obserde-schema` — `Schema`, `Field`, `Type`, `Constraint`.
- `obserde-value` — `Document`, `Path`.
- `obserde-validate` — structural + constraint `validate()`, `ValidationResult`.
- `obserde-canonical` — `canonicalize()`, `schema_hash()`, `document_hash()`.
- `obserde-json` — `encode()`/`decode()`, `DecodeLimits` — the JSON encoding backend (Phase 2).
- `obserde-compat` — `diff()`/`SchemaDiff`, `analyze()`/`CompatibilityReport` — schema diff and compatibility analysis (Phase 3).
- `obserde-migrate` — `Migration`, `MigrationGraph`, `plan()`/`available_paths()` — migration definition, execution, and graph-based planning (Phase 4). First crate to depend on `obserde-validate` as a non-dev dependency.
- `obserde-padagonia` — `SemanticId`, `SemanticAnnotations`, `SemanticResolver`/`StaticResolver`, `validate_semantic()` — semantic identifiers, ontology references, and semantic validation (Phase 5). Depends on the same three foundation crates as `obserde-compat`; zero dependency on `obserde-validate` despite both crates having "validation" in their purpose — independent sibling passes, not a pipeline.

Deferred:

- `obserde-cli` — first-class CLI, not yet scoped to a phase in this codebase; no directory exists for it yet.

## 17. Non-Goals for This Phase

No TOML/YAML encoding backend (JSON only so far), no CLI, no schema
registry, no deprecation states, no streaming/incremental decoding (§11's
`DecodeLimits` walk operates on a fully-parsed `serde_json::Value`, not a
token stream — see §11's honest accounting of what that does and doesn't
buy you); directive §20's "trusted path"/"incompatible path"
migration-planner grading (§13); wiring `obserde-compat` to consult a
`MigrationGraph` (§12, §13); on-disk or persisted migration definitions
(`obserde-migrate`'s `Migration`s are constructed in Rust and held in an
in-memory `MigrationGraph` only, the same posture Phase 1 took toward
`Schema` before an external format existed); a real Padagonia-backed
`SemanticResolver` adapter, and cross-checking structural `Constraint`s
against a resolver's semantic facts about the same concept (§14). All of
the above are real directive requirements for later phases or increments,
not omissions from this one.
