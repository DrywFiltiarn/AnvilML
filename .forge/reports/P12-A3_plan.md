# Plan Report: P12-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P12-A3                                        |
| Phase       | 12 — Graph Validation                        |
| Description | anvilml-scheduler: validate_graph structural checks (1-2) |
| Depends on  | P12-A2                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-07-06T18:50:00Z                          |
| Attempt     | 1                                             |

## Objective

Create `crates/anvilml-scheduler/src/dag.rs` implementing `validate_graph(graph, registry)` — the entry point for graph validation that runs structural checks 1–2 (root is an object with a "nodes" array; no duplicate node IDs) in collect-all-errors mode. This is the first check group in the six-check pipeline defined by ANVILML_DESIGN.md §12.3. The function accepts a `serde_json::Value` and a `&NodeTypeRegistry`, returns `Result<ValidatedGraph, Vec<GraphError>>`, and never short-circuits on the first error (except the non-object root case where further checking is structurally meaningless). Five integration tests verify both error paths and the happy path.

## Scope

### In Scope
- Create `crates/anvilml-scheduler/src/dag.rs` with `pub fn validate_graph(...) -> Result<ValidatedGraph, Vec<GraphError>>`.
- Implement check 1: root JSON is an object with a "nodes" array — push `NotAnObject` or `MissingNodesArray` and return early if structural check fails.
- Implement check 2: no duplicate node `id` values — push `DuplicateNodeId(id)` per duplicate found, continue checking.
- Update `crates/anvilml-scheduler/src/lib.rs` to add `pub mod dag;` and `pub use dag::validate_graph;`.
- Add ≥5 tests to `crates/anvilml-scheduler/tests/dag_tests.rs` covering both error paths, the duplicate-ID collector behavior, and the clean pass.

### Out of Scope
- Checks 3–6 (unknown node types, dangling edges, slot-type compatibility, cycle detection) — deferred to P12-A4, P12-A5, P12-A6 respectively.
- `ValidatedGraph` construction on success — that is P12-A6's scope (the final check that gates the `Ok(...)` return).
- Any logging instrumentation — this is a pure function; tracing spans are deferred to the scheduler integration in a later phase.

## Existing Codebase Assessment

**What exists:** `types.rs` already defines both `ValidatedGraph` (P12-A1) and `GraphError` with all seven variants (P12-A2). The `dag.rs` module does not yet exist — it is a blank slate. `lib.rs` currently re-exports only `types::GraphError` and `types::ValidatedGraph` (5 lines). `dag_tests.rs` already contains 9 tests from prior tasks (2 for `ValidatedGraph` derive/visibility, 7 for `GraphError` Display strings). `NodeTypeRegistry` lives in `anvilml-core` and provides `get(&self, type_name: &str) -> Option<NodeTypeDescriptor>` (synchronous, not async — the §12.2 design doc shows an async signature but the actual implementation uses `RwLock` with synchronous methods; this task will call `registry.get()` synchronously).

**Established patterns:** The project uses `thiserror::Error` with `#[error("...")]` attributes for Display. Tests are integration test crates in `crates/{name}/tests/` that import via the crate's public API. Doc comments on every `pub` item follow the `///` style describing what, preconditions, and return values. The `lib.rs` convention is re-exports only, ≤80 lines.

**Gap between design doc and current source:** ANVILML_DESIGN.md §12.2 shows `NodeTypeRegistry::get()` as `async`, but the actual implementation in `anvilml-core/src/node_registry.rs` is synchronous (`pub fn get(&self, type_name: &str) -> Option<NodeTypeDescriptor>`). This task must use the synchronous API. The `serde_json::Value` graph shape is not formally typed — each check will pattern-match on the JSON structure incrementally as the checks are added.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source | Feature flags confirmed |
|--------|-----------|-----------------|------------|------------------------|
| crate  | serde_json | 1 (workspace dep) | rust-docs MCP | n/a |
| crate  | thiserror  | 2.0.18 (in Cargo.toml) | rust-docs MCP | n/a |
| crate  | anvilml-core | path dep | rust-docs MCP | n/a |

No new external crates are introduced. All dependencies already exist in the workspace.

## Approach

**Step 1: Create `crates/anvilml-scheduler/src/dag.rs`** with module-level doc comment and the `validate_graph` function.

```rust
/// DAG graph validation for job submission.
///
/// Implements the collect-all-errors validation pipeline defined in
/// ANVILML_DESIGN.md §12.3. This task covers checks 1–2:
/// (1) root is an object with a "nodes" array,
/// (2) no duplicate node id values.
///
/// Checks 3–6 are added by subsequent tasks in Phase 12.
pub fn validate_graph(
    graph: serde_json::Value,
    _registry: &NodeTypeRegistry,
) -> Result<ValidatedGraph, Vec<GraphError>>;
```

The `_registry` parameter is prefixed with underscore to suppress the unused-variables warning — it is needed for checks 3–6 (deferred) but not for checks 1–2. The full signature matches the task context.

**Step 2: Implement check 1 (structural root validation).**

Inside `validate_graph`, first check if `graph` is a JSON object:
- If `!graph.is_object()`, push `GraphError::NotAnObject` to the errors vec and return `Err(errors)` immediately. A non-object root has no "nodes" key to check, so no further checks are meaningful.
- If it is an object, check if it contains a `"nodes"` key whose value is an array:
  - If `"nodes"` is absent or not an array, push `GraphError::MissingNodesArray` and return early.

Rationale: These two errors are mutually exclusive — if the root is not an object, there is no "nodes" key to check. The early return for non-object is the only early return permitted by the collect-all-errors contract (ANVILML_DESIGN.md §12.3: "never short-circuit on the first one, except where a violation makes further checking structurally meaningless").

**Step 3: Implement check 2 (duplicate node IDs).**

Extract the `"nodes"` array from the object. Iterate over the array, collecting each node's `id` field into a `HashSet<String>`. If an `id` is already in the set, push `GraphError::DuplicateNodeId(id.clone())` to the errors vec. Continue iterating — do not stop at the first duplicate.

Rationale: The task requires "push DuplicateNodeId(id) per duplicate found, continue checking" — every duplicate must be reported in a single `Err(Vec)`.

**Step 4: Return result.**

If `errors.is_empty()`, the graph passes checks 1–2. The function returns `Ok(ValidatedGraph(graph))` — note that `ValidatedGraph::new()` (or `_test_new()` for tests) is not available from outside the crate, but this function is `pub` within the crate, so it can call `ValidatedGraph(graph)` directly. The success path returns the validated graph wrapped in `Ok`. If errors were collected, return `Err(errors)`.

**Step 5: Update `crates/anvilml-scheduler/src/lib.rs`** to add:
```rust
pub mod dag;
pub use dag::validate_graph;
```

This follows the same pattern as the existing `pub mod types; pub use types::...;` re-exports.

**Step 6: Add tests to `crates/anvilml-scheduler/tests/dag_tests.rs`.**

Five new tests (see Tests section below). Each test constructs a `NodeTypeRegistry` (empty is sufficient for checks 1–2), calls `validate_graph()`, and asserts on the result.

**Documentation obligations:** The `validate_graph` function gets a `///` doc comment describing what it does, its preconditions (graph shape, registry populated), and return value. The `_registry` parameter's underscore prefix is an inline decision point — no comment needed since it is self-evident (unused until checks 3–6).

## Public API Surface

| Path | Item | Signature |
|------|------|-----------|
| `anvilml-scheduler::dag::validate_graph` | `pub fn` | `fn validate_graph(graph: serde_json::Value, registry: &NodeTypeRegistry) -> Result<ValidatedGraph, Vec<GraphError>>` |
| `anvilml-scheduler::dag` | `pub mod` | (module — no items beyond validate_graph) |

No new types, traits, or structs are introduced. The function uses existing types: `ValidatedGraph` (types.rs), `GraphError` (types.rs), `NodeTypeRegistry` (anvilml-core).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/dag.rs` | `validate_graph()` with checks 1–2 |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod dag;` and `pub use dag::validate_graph;` |
| MODIFY | `crates/anvilml-scheduler/tests/dag_tests.rs` | Add ≥5 tests for checks 1–2 |
| BUMP | `crates/anvilml-scheduler/Cargo.toml` | Patch version 0.1.2 → 0.1.3 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_non_object_root_returns_not_an_object` | Check 1: a non-object root (JSON array) returns `Err` containing exactly one `NotAnObject` error | Empty registry | `serde_json::json!([]))` | `Err([NotAnObject])` | `cargo test -p anvilml-scheduler --test dag_tests -- test_validate_graph_non_object_root_returns_not_an_object` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_missing_nodes_array_returns_missing_nodes_array` | Check 1: an object without a "nodes" key returns `Err` containing exactly one `MissingNodesArray` error | Empty registry | `serde_json::json!({"edges": []})` | `Err([MissingNodesArray])` | `cargo test -p anvilml-scheduler --test dag_tests -- test_validate_graph_missing_nodes_array_returns_missing_nodes_array` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_duplicate_ids_all_reported` | Check 2: duplicate node IDs are all collected in a single `Err`, not just the first | Empty registry | `{"nodes": [{"id":"a"},{"id":"b"},{"id":"a"}]}` | `Err([DuplicateNodeId("a"), DuplicateNodeId("a")])` — two entries for the repeated id | `cargo test -p anvilml-scheduler --test dag_tests -- test_validate_graph_duplicate_ids_all_reported` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_no_duplicates_passes_cleanly` | Checks 1–2 pass with zero errors; returns `Ok(ValidatedGraph(...))` | Empty registry | `{"nodes": [{"id":"a"},{"id":"b"}]}` | `Ok(ValidatedGraph)` | `cargo test -p anvilml-scheduler --test dag_tests -- test_validate_graph_no_duplicates_passes_cleanly` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_validate_graph_multiple_duplicate_violations_collected` | Check 2: multiple different duplicate IDs are all reported in one `Err` | Empty registry | `{"nodes": [{"id":"a"},{"id":"b"},{"id":"a"},{"id":"c"},{"id":"b"}]}` | `Err([DuplicateNodeId("a"), DuplicateNodeId("b"), DuplicateNodeId("a"), DuplicateNodeId("b")])` — each duplicate occurrence reported | `cargo test -p anvilml-scheduler --test dag_tests -- test_validate_graph_multiple_duplicate_violations_collected` exits 0 |

## CI Impact

No CI changes required. The new `dag.rs` source file is compiled as part of the `anvilml-scheduler` crate, and `dag_tests.rs` is picked up by `cargo test --workspace --features mock-hardware` (the full workspace test suite). No new Cargo targets, features, or CI job configurations are introduced.

## Platform Considerations

None identified. This task is platform-neutral: it operates on `serde_json::Value` data structures with no filesystem, network, or platform-specific I/O. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `serde_json::Value` field access on `"nodes"` array elements may panic if a node entry is not an object (e.g. `node["id"]` on a JSON string). | Low | Medium | Use `.get("nodes")` + `.as_array()` with `?` or pattern matching to safely handle malformed entries; skip malformed nodes rather than panicking. |
| The `_registry` parameter triggers a clippy `unused_variables` warning, and adding `#[allow(...)]` may be considered poor style. | Low | Low | Prefix with underscore (`_registry`) — this is the established Rust convention for intentionally unused parameters and is clippy-clean without any `#[allow]` attribute. |
| Existing tests in `dag_tests.rs` (from P12-A1/A2) may break if the test crate cannot resolve the new `validate_graph` import or if `lib.rs` changes break visibility. | Low | High | Update `lib.rs` with both `pub mod dag;` and `pub use dag::validate_graph;` in one edit; run `cargo test -p anvilml-scheduler --test dag_tests` immediately after to confirm all 14 tests (9 existing + 5 new) pass. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test dag_tests` exits 0 (all existing + new tests pass)
- [ ] `wc -l crates/anvilml-scheduler/src/dag.rs` reports a file with implementation (non-empty, <200 lines)
- [ ] `grep "^pub fn validate_graph" crates/anvilml-scheduler/src/dag.rs` returns exactly one match
- [ ] `grep "^pub mod dag;" crates/anvilml-scheduler/src/lib.rs` returns one match
- [ ] `grep "^pub use dag::validate_graph;" crates/anvilml-scheduler/src/lib.rs` returns one match
