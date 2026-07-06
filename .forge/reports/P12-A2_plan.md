# Plan Report: P12-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P12-A2                                      |
| Phase       | 12 — Graph Validation                       |
| Description | anvilml-scheduler: GraphError enum, all 7 variants |
| Depends on  | P12-A1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-06T15:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Define the `GraphError` enum in `crates/anvilml-scheduler/src/types.rs` with all seven variants specified in the task context (`NotAnObject`, `MissingNodesArray`, `DuplicateNodeId(String)`, `UnknownNodeType{node_id, type_name}`, `DanglingEdge{node_id, slot_name}`, `SlotTypeMismatch{node_id, slot_name, expected, found}`, `CycleDetected(Vec<String>)`), derive `Debug`, `Clone`, and `thiserror::Error` with per-variant `#[error("...")]` Display attributes, add `thiserror` to the crate's `Cargo.toml`, re-export `GraphError` from `lib.rs`, and write at least 6 tests in `dag_tests.rs` — including one test per variant verifying a non-empty, distinct Display string — so that `cargo test -p anvilml-scheduler --test dag_tests` exits 0.

## Scope

### In Scope
- Add `thiserror = "2.0.18"` to `crates/anvilml-scheduler/Cargo.toml`.
- Define `pub enum GraphError` with all 7 variants in `crates/anvilml-scheduler/src/types.rs`, with `#[derive(Debug, Clone, thiserror::Error)]` and `#[error("...")]` Display attributes on each variant.
- Add `pub use types::GraphError;` to `crates/anvilml-scheduler/src/lib.rs`.
- Add at least 7 tests in `crates/anvilml-scheduler/tests/dag_tests.rs`: one per variant verifying that its `Display` output is non-empty and distinct from the other variants.
- Bump `anvilml-scheduler` crate patch version from `0.1.1` to `0.1.2`.

### Out of Scope
None. This task's `defers_to` field is `[]` (empty). No scope is deferred. The task context phrases "per ANVILML_DESIGN.md §12.3" as a reference to the validation checks section — the enum definition itself is fully specified by the task's variant list; no implementation of `validate_graph()` or any check function is part of this task (those belong to P12-A3 through P12-A6).

## Existing Codebase Assessment

The `anvilml-scheduler` crate exists as a buildable stub created in Phase 1 (P1-B5). Phase 12's P12-A1 has already created `types.rs` with the `ValidatedGraph` newtype (construction-gated, `pub(crate)` inner field, `_test_new`/`_test_inner` test helpers). The crate's `lib.rs` declares `pub mod types;` and re-exports `ValidatedGraph`.

The established error-type pattern in this project uses `thiserror::Error` with `#[derive(Debug, Clone, thiserror::Error)]` and `#[error("...")]` attributes — confirmed by `IpcError` in `crates/anvilml-ipc/src/error.rs` (P7-A1) and `AnvilError` in `crates/anvilml-core/src/error.rs` (P2-A1). The workspace uses `thiserror = "2.0.18"` consistently across crates.

The existing `dag_tests.rs` contains 2 tests for `ValidatedGraph` (inner visibility and Debug/Clone derives), written as standalone `#[test]` functions (no `#[tokio::test]` needed since there's no async). The test style uses `serde_json::json!` for constructing test inputs.

No gap between the design doc and current source affects this task: the design doc §12.3 describes the six validation checks (not the error enum), and the task context provides the exact seven variant signatures.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | thiserror | 2.0.18        | rust-docs MCP  | none                   |

`thiserror` 2.0.18 was confirmed via `rust-docs_get_crate_version` — released 2026-01-18, MSRV 1.68, 158.2M downloads. This matches the version already pinned in `anvilml-core/Cargo.toml`. The derive macro API (`#[derive(Error)]`, `#[error("...")]`, struct-field interpolation `{actual} > {max}`) is confirmed working in v2. No feature flags are needed — `thiserror` is a compile-time derive-only dependency.

## Approach

1. **Add `thiserror` dependency to `crates/anvilml-scheduler/Cargo.toml`.** Add `thiserror = "2.0.18"` under the existing `[dependencies]` section, matching the version already used by `anvilml-core`. This is a compile-time-only dependency (the derive macro expands at compile time); no runtime features are needed.

2. **Define `GraphError` enum in `types.rs`.** Append the following enum to `crates/anvilml-scheduler/src/types.rs`, after the existing `ValidatedGraph` definition. The enum derives `Debug`, `Clone`, and `thiserror::Error`. Each variant carries a `#[error("...")]` Display attribute. The struct-style variants use named fields (matching the task context's `{node_id, type_name}` notation):

   ```rust
   /// Errors produced by graph validation.
   ///
   /// Each variant corresponds to one of the six validation checks defined
   /// in ANVILML_DESIGN.md §12.3, plus the structural root check (check 1).
   /// All variants derive `Debug`, `Clone`, and `thiserror::Error`.
   #[derive(Debug, Clone, thiserror::Error)]
   pub enum GraphError {
       /// The root JSON value is not an object (e.g. it is an array, string, or null).
       #[error("root is not an object")]
       NotAnObject,

       /// The root object does not contain a `"nodes"` key.
       #[error("missing \"nodes\" array")]
       MissingNodesArray,

       /// A node `id` value appeared more than once in the nodes array.
       #[error("duplicate node id: {0}")]
       DuplicateNodeId(String),

       /// A node referenced a `type` that is not registered in the node type registry.
       #[error("unknown node type \"{type_name}\" for node {node_id}")]
       UnknownNodeType { node_id: String, type_name: String },

       /// An edge references an output slot that the source node does not declare.
       #[error("dangling edge: node {node_id} missing output slot \"{slot_name}\"")]
       DanglingEdge { node_id: String, slot_name: String },

       /// An edge's output slot type is incompatible with the receiving input slot type.
       #[error("slot type mismatch on node {node_id} slot \"{slot_name}\": expected {expected}, found {found}")]
       SlotTypeMismatch { node_id: String, slot_name: String, expected: String, found: String },

       /// The graph contains a cycle; the Vec lists every node participating in the cycle.
       #[error("cycle detected involving nodes: {0:?}")]
       CycleDetected(Vec<String>),
   }
   ```

   Rationale for Display messages: each message is a short, human-readable string that identifies the error class and includes the most relevant context (node IDs, type names, slot names). The `CycleDetected` variant uses `{0:?}` to include the full vector of node IDs in the display output.

3. **Re-export `GraphError` from `lib.rs`.** Add `pub use types::GraphError;` alongside the existing `pub use types::ValidatedGraph;` line.

4. **Write tests in `dag_tests.rs`.** Add the following tests to `crates/anvilml-scheduler/tests/dag_tests.rs`:
   - `test_graph_error_not_an_object_display` — constructs `GraphError::NotAnObject`, verifies `Display` output is non-empty and equals `"root is not an object"`.
   - `test_graph_error_missing_nodes_array_display` — constructs `GraphError::MissingNodesArray`, verifies Display output is non-empty.
   - `test_graph_error_duplicate_node_id_display` — constructs `GraphError::DuplicateNodeId("node_a".into())`, verifies Display output contains `"node_a"`.
   - `test_graph_error_unknown_node_type_display` — constructs `GraphError::UnknownNodeType { node_id: "n1".into(), type_name: "BadNode".into() }`, verifies Display output contains both identifiers.
   - `test_graph_error_dangling_edge_display` — constructs `GraphError::DanglingEdge { node_id: "n2".into(), slot_name: "output".into() }`, verifies Display output contains both identifiers.
   - `test_graph_error_slot_type_mismatch_display` — constructs `GraphError::SlotTypeMismatch { node_id: "n3".into(), slot_name: "in".into(), expected: "FLOAT".into(), found: "INT".into() }`, verifies Display output contains all four fields.
   - `test_graph_error_cycle_detected_display` — constructs `GraphError::CycleDetected(vec!["A".into(), "B".into(), "C".into()])`, verifies Display output contains `"cycle detected"`.
   - `test_graph_error_display_distinct` — verifies all 7 Display strings are pairwise distinct (no two variants produce the same string), confirming the error messages are useful for disambiguation.

   Rationale for the distinctness test: the task requirement says "each of the 7 variants produces a non-empty, distinct Display string." The distinctness check is the acceptance criterion that confirms the messages are useful for operator diagnosis.

5. **Bump crate version.** Change `version = "0.1.1"` to `version = "0.1.2"` in `crates/anvilml-scheduler/Cargo.toml`.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| `pub enum GraphError` | `anvilml_scheduler::GraphError` | 7-variant error enum with `Debug`, `Clone`, `thiserror::Error` derives |
| `pub use types::GraphError` | `anvilml_scheduler::GraphError` | Re-export from crate root |

No new functions, structs, or traits. Only the enum and its re-export.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Add `thiserror = "2.0.18"` dependency; bump version 0.1.1 → 0.1.2 |
| Modify | `crates/anvilml-scheduler/src/types.rs` | Append `GraphError` enum with 7 variants, derives, and Display attributes |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Add `pub use types::GraphError;` re-export |
| Modify | `crates/anvilml-scheduler/tests/dag_tests.rs` | Add 8 tests for GraphError Display strings |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_graph_error_not_an_object_display` | `GraphError::NotAnObject` Display is non-empty and equals exact string | None | `GraphError::NotAnObject` | Display = `"root is not an object"` | `cargo test -p anvilml-scheduler --test dag_tests test_graph_error_not_an_object_display` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_graph_error_missing_nodes_array_display` | `GraphError::MissingNodesArray` Display is non-empty | None | `GraphError::MissingNodesArray` | Display is non-empty string | Same command exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_graph_error_duplicate_node_id_display` | `DuplicateNodeId(String)` Display includes the node ID string | None | `DuplicateNodeId("node_a".into())` | Display contains `"node_a"` | Same command exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_graph_error_unknown_node_type_display` | `UnknownNodeType` Display includes node_id and type_name | None | `{ node_id: "n1", type_name: "BadNode" }` | Display contains both `"n1"` and `"BadNode"` | Same command exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_graph_error_dangling_edge_display` | `DanglingEdge` Display includes node_id and slot_name | None | `{ node_id: "n2", slot_name: "output" }` | Display contains both `"n2"` and `"output"` | Same command exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_graph_error_slot_type_mismatch_display` | `SlotTypeMismatch` Display includes all four fields | None | `{ node_id: "n3", slot_name: "in", expected: "FLOAT", found: "INT" }` | Display contains all four values | Same command exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_graph_error_cycle_detected_display` | `CycleDetected` Display includes cycle node list | None | `CycleDetected(["A","B","C"])` | Display contains `"cycle detected"` and node names | Same command exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | `test_graph_error_display_distinct` | All 7 Display strings are pairwise distinct | None | All 7 variants | No two `.to_string()` outputs are equal | Same command exits 0 |

## CI Impact

No CI changes required. The new tests are in the existing `dag_tests.rs` file under `crates/anvilml-scheduler/tests/`, which is already picked up by `cargo test --workspace --features mock-hardware`. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `GraphError` enum is a pure data type with no platform-specific behaviour, no `#[cfg(unix)]` / `#[cfg(windows)]` guards required, and no path-separator or line-ending handling. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `thiserror` 2.0 struct-field interpolation in `#[error("...")]` may not work with named fields (e.g. `{node_id}` syntax on struct-style variants). | Low | High | The existing `IpcError::PayloadTooLarge { actual, max }` variant in `crates/anvilml-ipc/src/error.rs` already uses named struct fields with `#[error("payload too large: {actual} > {max}")]`, confirming the syntax works. If it does not compile, fall back to positional `{0}` / `{1}` or a manual `impl Display`. |
| Adding `thiserror` to `anvilml-scheduler` may introduce a transitive dependency conflict with the workspace's existing `thiserror` pin, since both are path dependencies in the same workspace. | Low | Medium | Using the exact same version string (`"2.0.18"`) as `anvilml-core` avoids this — cargo will deduplicate the dependency. The workspace already uses `anvilml-core` as a dependency of `anvilml-scheduler`'s transitive deps. |
| The 8 new tests in `dag_tests.rs` may push the file past a line-count review threshold. | Low | Low | The existing file is 43 lines; adding 8 tests (~120 lines) brings it to ~165 lines, well under the 500-line test file review threshold from ARCHITECTURE.md §11. |

## Acceptance Criteria

- [ ] `grep "thiserror" crates/anvilml-scheduler/Cargo.toml` matches (dependency is declared)
- [ ] `grep "pub use types::GraphError" crates/anvilml-scheduler/src/lib.rs` matches (re-export is present)
- [ ] `grep "pub enum GraphError" crates/anvilml-scheduler/src/types.rs` matches (enum is defined)
- [ ] `cargo test -p anvilml-scheduler --test dag_tests` exits 0 (all tests pass, including existing 2 + new 8 = 10 total)
- [ ] `grep -c "^#\[test\]" crates/anvilml-scheduler/tests/dag_tests.rs` returns a number >= 6 (at least 6 test functions in the file)
- [ ] `grep '^version' crates/anvilml-scheduler/Cargo.toml` returns `version = "0.1.2"` (patch version bumped)
