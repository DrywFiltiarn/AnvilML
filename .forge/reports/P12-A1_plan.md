# Plan Report: P12-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P12-A1                                            |
| Phase       | 012 — Graph Validation                            |
| Description | anvilml-scheduler: dag.rs validate_graph collect-all-errors mode |
| Depends on  | P11-A3 (NodeTypeRegistry exists, populated from worker Ready events) |
| Project     | anvilml                                           |
| Planned at  | 2026-06-19T16:45:00Z                              |
| Attempt     | 1                                                 |

## Objective

Implement `validate_graph` in `crates/anvilml-scheduler/src/dag.rs`, a non-fail-fast function that collects all validation errors (missing nodes array, duplicate IDs, unknown node types, bad edge references, slot type mismatches, and cycles) before returning. The function returns `Ok(ValidatedGraph(serde_json::Value))` only when all checks pass. This is the foundational validation logic that Phase 013's `JobScheduler::submit` builds on.

## Scope

### In Scope
- **`crates/anvilml-scheduler/src/dag.rs`** — new file containing:
  - `pub struct ValidatedGraph(pub serde_json::Value)` — newtype wrapping the original graph JSON
  - `pub async fn validate_graph(graph: &serde_json::Value, registry: &NodeTypeRegistry) -> Result<ValidatedGraph, Vec<String>>` — collects all errors, returns `Ok` only when all pass
- **`crates/anvilml-scheduler/src/lib.rs`** — add `pub mod dag` declaration
- **`crates/anvilml-scheduler/tests/dag_tests.rs`** — new file with ≥ 8 tests
- **`crates/anvilml-scheduler/Cargo.toml`** — add `serde_json` dependency; bump patch version `0.1.2 → 0.1.3`

### Out of Scope
- `GraphError` enum — that is P12-A2
- Integration with HTTP handler — that is P12-B1
- Persistence of validated graphs — that is P13
- Real graph execution — that is P14

## Existing Codebase Assessment

The `anvilml-scheduler` crate currently contains only `src/lib.rs` (18 lines) which re-exports `NodeTypeRegistry` from `anvilml-core::node_registry`, and one test file `tests/node_registry_tests.rs` (220 lines) with 6 async tests using `tokio::test`. The crate's `Cargo.toml` declares version `0.1.2` with dependencies on `anvilml-core`, `anvilml-hardware`, `anvilml-registry`, `anvilml-worker`, `tokio`, and `tracing`.

`NodeTypeRegistry` is defined in `anvilml-core::node_registry` as an `Arc<RwLock<HashMap<String, NodeTypeDescriptor>>>` with methods `new()`, `update_from_worker()`, `get()`, `all_types()`, `is_empty()`, and `has_been_updated()`. All methods are `async fn`.

`SlotType` is defined in `anvilml-core::types::node` as an enum with variants: `Model`, `Clip`, `Vae`, `Conditioning`, `Latent`, `Image`, `String`, `Int`, `Float`, `Bool`, `Any`. The `Any` variant is the default and matches any other type for compatibility checking.

`NodeTypeDescriptor` (in the same file) has fields: `type_name`, `display_name`, `category`, `description`, `inputs`, `outputs`. `SlotDescriptor` has `name`, `slot_type`, `optional`.

The established patterns are:
- Tests in `crates/{name}/tests/` as separate test crates (not inline `#[cfg(test)]`)
- `tokio::test` for async tests
- `NodeTypeRegistry` passed by reference (not `Arc`) to functions — callers dereference
- Doc comments on all public items using `///` format
- Structured `tracing::debug!` calls at decision points

No `serde_json` dependency exists in `anvilml-scheduler/Cargo.toml` yet — it must be added. The `indexmap` crate is not yet in the workspace dependencies and will need to be added for deterministic Kahn's algorithm iteration.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source  | Feature flags confirmed |
|--------|-----------|-----------------|-------------|------------------------|
| crate  | serde_json| 1.0.150         | workspace Cargo.toml | derive (from workspace) |
| crate  | indexmap  | 2.7             | crates.io (new dep, not in workspace yet) | none |

Note: `serde_json` is already in `[workspace.dependencies]` at `1.0.150`. `indexmap` is not yet in the workspace — it will be added to `anvilml-scheduler/Cargo.toml` as a direct dependency. The `IndexMap` type from `indexmap` is used for deterministic iteration order in Kahn's algorithm, ensuring cycle detection produces consistent node ordering regardless of hash map iteration order.

## Approach

1. **Add dependencies to `Cargo.toml`**. Add `serde_json = { workspace = true }` and `indexmap = "2.7"`. Bump version from `0.1.2` to `0.1.3`.

2. **Create `crates/anvilml-scheduler/src/dag.rs`** with the following structure:

   a. **`ValidatedGraph` newtype** (line ~5):
   ```rust
   /// A graph that has passed all validation checks.
   ///
   /// This newtype wraps the original JSON payload and is the only
   /// way to construct a validated graph — the constructor is private
   /// to this module, ensuring no caller can bypass validation.
   #[derive(Debug, Clone)]
   pub struct ValidatedGraph(pub serde_json::Value);
   ```

   b. **`validate_graph` function** (line ~15):
   ```rust
   /// Validate a job graph against the node type registry.
   ///
   /// Collects all errors before returning (non-fail-fast), so callers
   /// receive the complete list of problems in a single response.
   ///
   /// Checks performed (all collected, none fail-fast):
   /// 1. Root JSON is an object with a `"nodes"` array.
   /// 2. No duplicate node `id` values.
   /// 3. Every node `type` exists in `NodeTypeRegistry`.
   /// 4. Every edge reference `{node_id, output_slot}` resolves to an
   ///    existing node and a declared output slot.
   /// 5. Every edge's output slot type is compatible with the receiving
   ///    input slot type (both match, or either is `SlotType::Any`).
   /// 6. The graph is acyclic (Kahn's algorithm).
   ///
   /// # Arguments
   ///
   /// * `graph` — The submitted graph JSON value. Must be a JSON object
   ///   with `"nodes"` and optionally `"edges"` arrays.
   /// * `registry` — The current node type registry, populated from
   ///   worker `Ready` events.
   ///
   /// # Returns
   ///
   /// `Ok(ValidatedGraph)` if all checks pass, containing the original
   /// graph value. `Err(Vec<String>)` with all error messages if any
   /// check fails — the vector contains one human-readable string per
   /// failure, each naming the specific offending node/slot/type.
   #[tracing::instrument(skip(graph, registry), fields(graph_nodes = ?graph.get("nodes").and_then(|n| n.get("len").map(|l| l.as_u64()))))]
   pub async fn validate_graph(
       graph: &serde_json::Value,
       registry: &NodeTypeRegistry,
   ) -> Result<ValidatedGraph, Vec<String>>
   ```

   c. **Internal validation logic** — implement each check as a separate private function to keep the main function readable:

   - `fn check_nodes_array(graph: &Value) -> Option<String>` — verifies `graph["nodes"]` exists and is an array; returns `None` on success, error string on failure.
   - `fn check_duplicate_ids(nodes: &[Value]) -> Vec<String>` — iterates nodes, collects IDs in a `HashSet`, reports duplicates.
   - `fn check_node_types(nodes: &[Value], registry: &NodeTypeRegistry) -> Vec<String>` — for each node, looks up `type` in registry; unknown types produce error strings.
   - `fn check_edge_refs(nodes: &[Value]) -> Vec<String>` — for each edge, resolves `node_id` to a node, then resolves `output_slot` to a declared output slot on that node. Reports missing nodes and missing slots separately.
   - `fn check_slot_compatibility(nodes: &[Value], registry: &NodeTypeRegistry) -> Vec<String>` — for each edge, looks up the source node's output slot type and the target node's input slot type; incompatible types (neither is `Any`) produce an error string naming both types.
   - `fn check_acyclic(nodes: &[Value]) -> Option<String>` — builds adjacency list from edges, runs Kahn's algorithm on the node graph; if not all nodes are processed, returns `Some(error_string)` naming the cycle participants.

   d. **Main function body** — collect errors from each check into a single `Vec<String>`, return early with `Err(errors)` if non-empty, otherwise `Ok(ValidatedGraph(graph.clone()))`.

   e. **Inline comments** — every decision point gets a `//` comment explaining why (per FORGE_AGENT_RULES §12): why Kahn's algorithm over DFS, why `IndexMap` for deterministic iteration, why `HashSet` for duplicate detection, etc.

3. **Update `crates/anvilml-scheduler/src/lib.rs`** — add `pub mod dag;` after the `pub use anvilml_core::NodeTypeRegistry;` line.

4. **Create `crates/anvilml-scheduler/tests/dag_tests.rs`** with ≥ 8 tests:

   - `test_missing_nodes_array` — submit a graph without `"nodes"` → `Err` with one message about missing nodes array.
   - `test_duplicate_node_ids` — two nodes with same `"id"` → `Err` with duplicate ID error.
   - `test_unknown_node_type` — node with `type: "NonExistent"` → `Err` with unknown type error.
   - `test_bad_edge_ref_missing_node` — edge references a `node_id` that does not exist → `Err` with missing node error.
   - `test_bad_edge_ref_missing_slot` — edge references an `output_slot` not declared by the source node → `Err` with missing slot error.
   - `test_slot_type_mismatch` — edge connects `MODEL` output to `IMAGE` input → `Err` with type mismatch error.
   - `test_cycle_detected` — three nodes forming A→B→C→A cycle → `Err` with cycle error naming all three nodes.
   - `test_valid_graph_returns_ok` — complete valid graph with LoadModel, Sampler, VaeDecode, SaveImage → `Ok(ValidatedGraph)`.
   - `test_multiple_errors_collected` — graph with both duplicate IDs and unknown type in same submission → `Err` with ≥ 2 error strings (verifies non-fail-fast).
   - `test_any_slot_type_compatible` — edge connects `Any` output to `MODEL` input → passes (verifies `Any` compatibility rule).

   Each test uses `NodeTypeRegistry` populated with appropriate node types via `update_from_worker`, then calls `validate_graph` and asserts on the result.

5. **Add `///` doc comments** to `ValidatedGraph` and `validate_graph` per FORGE_AGENT_RULES §12.1.

## Public API Surface

| Item | Crate/Module | Signature |
|------|-------------|-----------|
| `ValidatedGraph` | `anvilml_scheduler::dag` | `pub struct ValidatedGraph(pub serde_json::Value)` |
| `validate_graph` | `anvilml_scheduler::dag` | `pub async fn validate_graph(graph: &serde_json::Value, registry: &NodeTypeRegistry) -> Result<ValidatedGraph, Vec<String>>` |

No new `pub use` re-exports needed — `ValidatedGraph` is imported directly from the `dag` module.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/dag.rs` | New file; `ValidatedGraph` newtype and `validate_graph` function with 6 validation checks |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod dag;` declaration |
| CREATE | `crates/anvilml-scheduler/tests/dag_tests.rs` | New test file; ≥ 10 tests covering all validation checks |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Add `serde_json` dependency (workspace), add `indexmap = "2.7"`; bump version `0.1.2 → 0.1.3` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/dag_tests.rs` | `test_missing_nodes_array` | Graph without `"nodes"` key returns error | None | `{ "edges": [] }` | `Err` with message about missing nodes array | `cargo test -p anvilml-scheduler --features mock-hardware -- test_missing_nodes_array` exits 0 |
| `tests/dag_tests.rs` | `test_duplicate_node_ids` | Two nodes with same `"id"` returns error | None | Graph with 2 nodes, both `"id": "n1"` | `Err` with duplicate ID message naming `"n1"` | `cargo test -p anvilml-scheduler --features mock-hardware -- test_duplicate_node_ids` exits 0 |
| `tests/dag_tests.rs` | `test_unknown_node_type` | Node with unregistered type returns error | Registry has `LoadModel` only | Node with `type: "NonExistent"` | `Err` with unknown type message naming `"NonExistent"` | `cargo test -p anvilml-scheduler --features mock-hardware -- test_unknown_node_type` exits 0 |
| `tests/dag_tests.rs` | `test_bad_edge_ref_missing_node` | Edge references non-existent node returns error | Registry has `LoadModel` | Edge with `"node_id": "ghost"` | `Err` with missing node message | `cargo test -p anvilml-scheduler --features mock-hardware -- test_bad_edge_ref_missing_node` exits 0 |
| `tests/dag_tests.rs` | `test_bad_edge_ref_missing_slot` | Edge references undeclared output slot returns error | Registry has `LoadModel` (outputs `model` only) | Edge with `output_slot: "nonexistent"` | `Err` with missing slot message | `cargo test -p anvilml-scheduler --features mock-hardware -- test_bad_edge_ref_missing_slot` exits 0 |
| `tests/dag_tests.rs` | `test_slot_type_mismatch` | Incompatible slot types (MODEL→IMAGE) returns error | Registry has `LoadModel` (outputs `Model`) and `SaveImage` (inputs `Image`) | Edge from `LoadModel.model` to `SaveImage.image` | `Err` with mismatch message naming both types | `cargo test -p anvilml-scheduler --features mock-hardware -- test_slot_type_mismatch` exits 0 |
| `tests/dag_tests.rs` | `test_cycle_detected` | Three-node cycle A→B→C→A returns error | Registry has all three types | Graph with cycle edges | `Err` with cycle message naming cycle participants | `cargo test -p anvilml-scheduler --features mock-hardware -- test_cycle_detected` exits 0 |
| `tests/dag_tests.rs` | `test_valid_graph_returns_ok` | Complete valid DAG returns `Ok` | Registry has all required node types | Full valid graph (LoadModel → Sampler → VaeDecode → SaveImage) | `Ok(ValidatedGraph)` | `cargo test -p anvilml-scheduler --features mock-hardware -- test_valid_graph_returns_ok` exits 0 |
| `tests/dag_tests.rs` | `test_multiple_errors_collected` | Non-fail-fast: multiple errors returned together | Registry has `LoadModel` only | Graph with duplicate IDs + unknown type | `Err` with ≥ 2 error strings | `cargo test -p anvilml-scheduler --features mock-hardware -- test_multiple_errors_collected` exits 0 |
| `tests/dag_tests.rs` | `test_any_slot_type_compatible` | `Any` slot type accepts any connection | Registry has node with `Any` output | Edge from `Any` output to `Model` input | `Ok(ValidatedGraph)` (no type error) | `cargo test -p anvilml-scheduler --features mock-hardware -- test_any_slot_type_compatible` exits 0 |

## CI Impact

No CI changes required. The `cargo test --workspace --features mock-hardware` job (rust-linux, rust-windows) will automatically pick up the new test file in `crates/anvilml-scheduler/tests/` because it runs the full workspace test suite. No new file types, gates, or test modules are introduced that would change CI behavior.

## Platform Considerations

None identified. The `validate_graph` function operates purely on `serde_json::Value` data structures and `HashMap`/`IndexMap` — no platform-specific syscalls, path handling, or line-ending concerns. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `serde_json::Value` edge format is not documented precisely — the task says `{node_id, output_slot}` but the actual JSON shape in the graph may use a different key structure (e.g., `source`/`target` vs `node_id`/`output_slot`). | Medium | High | Read the graph JSON example in `ANVILML_DESIGN.md §19.2` (Appendix B) which shows the exact edge format: `"edge": { "node_id": "model", "output_slot": "model" }`. Use these exact key names. Write the `test_valid_graph_returns_ok` test using the Appendix B graph structure to confirm parsing works. |
| `IndexMap` is not yet in the workspace dependencies — adding it as a direct dependency of `anvilml-scheduler` is permitted by the dependency graph rules (it's a new leaf dependency), but the version must be confirmed. | Low | Medium | Use `indexmap = "2.7"` directly in `anvilml-scheduler/Cargo.toml`. The `IndexMap` API (`keys()`, `values()`, `get()`, `insert()`) is stable across 2.x versions. If MCP lookup shows a newer version, use that. |
| `NodeTypeRegistry::get()` and `update_from_worker()` are `async fn` — the plan calls `validate_graph` which is also `async`, so all registry calls inside it will be `.await`ed. This is correct but the test setup needs to `#[tokio::test]` the registry updates. | Low | Low | All tests already use `#[tokio::test]`. The registry is set up within each test function body before calling `validate_graph`. |
| Kahn's algorithm implementation may produce non-deterministic cycle member lists if the underlying data structure iterates in hash order. | Low | Medium | Use `IndexMap` for the adjacency list and in-degree tracking, which preserves insertion order. This makes the cycle member list deterministic and reproducible across runs. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- dag` exits 0 with ≥ 8 tests
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (no regressions in other crates)
- [ ] `cargo check --workspace --features mock-hardware` exits 0 (new dependency compiles)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no lint warnings)
- [ ] `head -1 crates/anvilml-scheduler/src/dag.rs` confirms file starts with `//!` crate-level doc comment
- [ ] `grep "^pub " crates/anvilml-scheduler/src/dag.rs | head -5` shows exactly 2 pub items: `ValidatedGraph` struct and `validate_graph` function
- [ ] `grep "^## " .forge/reports/P12-A1_plan.md` returns 12 section headings
