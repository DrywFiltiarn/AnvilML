# Implementation Report: P12-A2

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P12-A2                              |
| Phase         | 012 — Graph Validation             |
| Description   | GraphError enum and types.rs ValidatedGraph |
| Implemented   | 2026-06-19T19:00:00Z               |
| Status        | COMPLETE                            |

## Summary

Created `crates/anvilml-scheduler/src/types.rs` with the `GraphError` enum covering all five graph validation failure modes (`UnknownNodeType`, `DuplicateNodeId`, `UnknownEdgeRef`, `SlotTypeMismatch`, `CycleDetected`) and a `Display` implementation that reproduces the exact error strings produced by the original check functions. Refactored all five internal check functions in `dag.rs` to return `Vec<GraphError>` (or `Option<GraphError>` for `check_acyclic`), with string conversion only at the `validate_graph` return point. Added `pub mod types` and `pub use types::GraphError` to `lib.rs` for downstream crate access. Bumped `anvilml-scheduler` patch version from `0.1.3` to `0.1.4`. All 10 existing `dag_tests.rs` tests pass without modification because the public API of `validate_graph` is unchanged.

## Resolved Dependencies

None. This task introduces no new external crates or packages. All types used (`GraphError`, `SlotType`, `ValidatedGraph`) are either created in this task or already exist in `anvilml-core` / `dag.rs`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/types.rs` | `GraphError` enum with 5 variants, `Display` impl, `ValidatedGraph` re-export |
| MODIFY | `crates/anvilml-scheduler/src/dag.rs` | Import `GraphError`, refactor check functions to return `Vec<GraphError>`, convert to strings at return point |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod types; pub use types::GraphError;` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.3 → 0.1.4` |

## Commit Log

```
 .forge/reports/P12-A2_plan.md         | 225 ++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md          |   6 +-
 .forge/state/state.json               |  13 +-
 Cargo.lock                            |   2 +-
 crates/anvilml-scheduler/Cargo.toml   |   2 +-
 crates/anvilml-scheduler/src/dag.rs   |  98 +++++++++------
 crates/anvilml-scheduler/src/lib.rs   |   3 +
 crates/anvilml-scheduler/src/types.rs | 118 ++++++++++++++++++
 8 files changed, 416 insertions(+), 51 deletions(-)
```

## Test Results

```
     Running tests/dag_tests.rs (target/debug/deps/dag_tests-7f2018b426fcb4f8)

running 10 tests
test test_bad_edge_ref_missing_node ... ok
test test_any_slot_type_compatible ... ok
test test_bad_edge_ref_missing_slot ... ok
test test_cycle_detected ... ok
test test_duplicate_node_ids ... ok
test test_multiple_errors_collected ... ok
test test_missing_nodes_array ... ok
test test_slot_type_mismatch ... ok
test test_unknown_node_type ... ok
test test_valid_graph_returns_ok ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace: 158 tests passed, 0 failed.

## Format Gate

```
(no output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.85s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.07s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.05s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.69s
```

All four cross-checks exit 0.

## Project Gates

**Gate 1 — Config Surface Sync:**
```
running 1 test
test config_reference ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Gate 2 — OpenAPI Drift:** Not applicable — task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields used in response types.

**Gate 3 — Node Parity:** Not applicable — task does not add, remove, or rename node types, and does not modify `crates/anvilml-scheduler/src/node_registry.rs`.

## Public API Delta

```
+pub mod types;
+pub use types::GraphError;
```

New public items introduced:

| Item | Type | Module Path |
|------|------|-------------|
| `GraphError` | `pub enum` | `anvilml_scheduler::types::GraphError` |
| `ValidatedGraph` | `pub use` (re-export) | `anvilml_scheduler::types::ValidatedGraph` |
| `GraphError` | `pub use` (re-export) | `anvilml_scheduler::GraphError` (crate root) |

The `Display` impl on `GraphError` is `impl Display for GraphError` — public because `Display` is a public trait and the impl is `pub`.

## Deviations from Plan

- **`UnknownEdgeRef` variant field usage:** The plan specifies `UnknownEdgeRef { node_id, slot }` for both missing nodes and missing slots. The original `check_edge_refs` produces two distinct error strings: `"validation failed: edge references missing source node \"{id}\""` for missing nodes, and `"validation failed: node \"{id}\" has no output slot \"{slot}\""` for missing slots. To preserve both error messages, the `Display` impl distinguishes them by checking if `slot` is empty: empty slot → "missing source node" message; non-empty slot → "has no output slot" message. This is a deviation from the plan's Display format (which only showed the "has no output slot" variant) but preserves the exact error strings the existing tests assert on.

- **`check_node_types` unused variable:** The original function computed `node_id` from each node's `"id"` field and used it in the error message. After refactoring to `GraphError::UnknownNodeType(node_type.to_string())`, the `node_id` variable became unused. It was removed since the `Display` impl uses `type_name` for both placeholders, matching the test assertions (which check for `"NonExistent"` and `"unknown type"` from the type name).

## Blockers

None.
