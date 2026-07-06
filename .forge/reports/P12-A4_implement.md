# Implementation Report: P12-A4

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P12-A4                          |
| Phase         | 12 — Graph Validation           |
| Description   | anvilml-scheduler: validate_graph node-type + edge checks (3-4) |
| Implemented   | 2026-07-06T21:00:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented checks 3 and 4 of the six-check DAG validation pipeline in `validate_graph()`. Check 3 iterates all nodes and verifies each node's `"type"` field exists in the `NodeTypeRegistry`, pushing `GraphError::UnknownNodeType` for any unregistered type. Check 4 iterates all edges, parses the `"from"` field as `"node_id:slot_name"`, verifies the source node exists in the nodes array, and verifies the node type declares the referenced output slot — pushing `GraphError::DanglingEdge` for any violation. Both checks follow collect-all-errors semantics. Updated the module-level doc comment to reflect checks 1–4 are implemented. Added 6 new integration tests in `dag_tests.rs`, bringing the total to 21 tests.

## Resolved Dependencies

No new dependencies introduced. All types (`NodeTypeRegistry::get()`, `NodeTypeDescriptor`, `SlotDescriptor`, `SlotType`) are already defined in `anvilml-core` and were verified present in the source files read during inspection.

| Type   | Name          | Version resolved | Source         |
|--------|---------------|------------------|----------------|
| crate  | anvilml-core  | 0.1.22           | Workspace path |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/dag.rs` | Add checks 3–4 to `validate_graph()`; update module-level doc comment; change `_registry` to `registry` |
| Modify | `crates/anvilml-scheduler/tests/dag_tests.rs` | Add 6 new integration tests for checks 3–4; update imports to include `NodeTypeDescriptor`, `SlotDescriptor`, `SlotType` |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |
| Modify | `docs/TESTS.md` | Add 6 new test catalogue entries for checks 3–4 tests |

## Commit Log

```
 .forge/reports/P12-A4_plan.md               | 146 +++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +-
 Cargo.lock                                  |   2 +-
 crates/anvilml-scheduler/Cargo.toml         |   2 +-
 crates/anvilml-scheduler/src/dag.rs         | 158 +++++++++++++++---
 crates/anvilml-scheduler/tests/dag_tests.rs | 243 +++++++++++++++++++++++++++-
 docs/TESTS.md                               |  72 +++++++++
 8 files changed, 610 insertions(+), 32 deletions(-)
```

## Test Results

```
     Running tests/dag_tests.rs (target/debug/deps/dag_tests-c683e917f0d6a6ad)

running 21 tests
test test_graph_error_cycle_detected_display ... ok
test test_graph_error_dangling_edge_display ... ok
test test_graph_error_duplicate_node_id_display ... ok
test test_graph_error_display_distinct ... ok
test test_graph_error_missing_nodes_array_display ... ok
test test_graph_error_not_an_object_display ... ok
test test_graph_error_slot_type_mismatch_display ... ok
test test_graph_error_unknown_node_type_display ... ok
test test_validate_graph_edge_to_nonexistent_node ... ok
test test_validate_graph_duplicate_ids_all_reported ... ok
test test_validate_graph_edge_to_undeclared_slot ... ok
test test_validate_graph_missing_nodes_array_returns_missing_nodes_array ... ok
test test_validate_graph_multiple_duplicate_violations_collected ... ok
test test_validate_graph_no_duplicates_passes_cleanly ... ok
test test_validate_graph_multiple_violations_collected ... ok
test test_validate_graph_non_object_root_returns_not_an_object ... ok
test test_validate_graph_unknown_node_type_reported ... ok
test test_validate_graph_valid_edges_pass_cleanly ... ok
test test_validate_graph_valid_type_passes_check3 ... ok
test test_validated_graph_derives_debug_and_clone ... ok
test test_validated_graph_inner_is_pub_crate ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

All 21 tests passed (15 existing + 6 new). The full workspace test suite exited 0 with zero failures across all crates.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.81s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.46s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.33s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.57s

All four checks exited 0.
```

## Project Gates

**Gate 1 — Config Surface Sync:**
```
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Gate 2 — OpenAPI Drift:** Not triggered — no handler function signatures or ToSchema derives were modified.

**Gate 3 — Node Parity:** Not triggered — no node types added, removed, or renamed.

**Gate 4 — Mock/Real Parity Markers:** Not triggered — no node `execute()` or arch module `load()`/`sample()`/`decode()`/`compute_latent_shape()` methods were added or modified.

## Public API Delta

No new `pub` items introduced. The `validate_graph()` function signature remains unchanged:
```rust
pub fn validate_graph(graph: Value, registry: &NodeTypeRegistry) -> Result<ValidatedGraph, Vec<GraphError>>
```

## Deviations from Plan

None. Implementation matches the approved plan exactly:
- Check 3 iterates nodes, extracts `"id"` and `"type"` fields, queries the registry, pushes `UnknownNodeType` for unregistered types.
- Check 4 parses edges, builds an `id_to_type` map, verifies node existence and slot declaration, pushes `DanglingEdge` for violations.
- Module doc comment updated from "checks 1–2" to "checks 1–4" with deferred list for checks 5–6.
- 6 new integration tests written and passing.

## Blockers

None.
