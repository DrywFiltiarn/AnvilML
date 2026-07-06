# Implementation Report: P12-A3

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P12-A3                                      |
| Phase         | 12 — Graph Validation                        |
| Description   | anvilml-scheduler: validate_graph structural checks (1-2) |
| Implemented   | 2026-07-06T20:45:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Created `crates/anvilml-scheduler/src/dag.rs` implementing `validate_graph(graph, registry)` — the entry point for DAG graph validation covering structural checks 1–2 of the six-check pipeline defined in ANVILML_DESIGN.md §12.3: (1) root is a JSON object with a "nodes" array, and (2) no duplicate node id values. The function uses collect-all-errors semantics, returning `Ok(ValidatedGraph)` on success or `Err(Vec<GraphError>)` with all collected errors. Five integration tests verify both error paths and the clean pass. The `dag` module is re-exported from `lib.rs` alongside the existing `types` module.

## Resolved Dependencies

| Type   | Name       | Version verified | Source         |
|--------|-----------|------------------|----------------|
| crate  | serde_json | 1 (workspace dep) | rust-docs MCP  |
| crate  | thiserror  | 2.0.18           | rust-docs MCP  |
| crate  | anvilml-core | path dep       | rust-docs MCP  |

No new external crates were introduced. All dependencies already exist in the workspace.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/dag.rs` | `validate_graph()` with checks 1–2 |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod dag;` and `pub use dag::validate_graph;` |
| MODIFY | `crates/anvilml-scheduler/tests/dag_tests.rs` | Add 5 tests for checks 1–2 |
| BUMP | `crates/anvilml-scheduler/Cargo.toml` | Patch version 0.1.2 → 0.1.3 |
| MODIFY | `docs/TESTS.md` | Add 5 test catalogue entries |

## Commit Log

```
 .forge/reports/P12-A3_plan.md               | 153 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +--
 Cargo.lock                                  |   2 +-
 crates/anvilml-scheduler/Cargo.toml         |   2 +-
 crates/anvilml-scheduler/src/dag.rs         |  89 ++++++++++++++++
 crates/anvilml-scheduler/src/lib.rs         |   2 +
 crates/anvilml-scheduler/tests/dag_tests.rs | 127 ++++++++++++++++++++++-
 docs/TESTS.md                               |  60 +++++++++++
 9 files changed, 442 insertions(+), 12 deletions(-)
```

## Test Results

```
     Running tests/dag_tests.rs (target/debug/deps/dag_tests-44254f72da95b1ed)

running 15 tests
test test_graph_error_cycle_detected_display ... ok
test test_graph_error_display_distinct ... ok
test test_graph_error_duplicate_node_id_display ... ok
test test_graph_error_dangling_edge_display ... ok
test test_graph_error_missing_nodes_array_display ... ok
test test_graph_error_slot_type_mismatch_display ... ok
test test_graph_error_not_an_object_display ... ok
test test_graph_error_unknown_node_type_display ... ok
test test_validate_graph_duplicate_ids_all_reported ... ok
test test_validate_graph_missing_nodes_array_returns_missing_nodes_array ... ok
test test_validate_graph_multiple_duplicate_violations_collected ... ok
test test_validate_graph_non_object_root_returns_not_an_object ... ok
test test_validate_graph_no_duplicates_passes_cleanly ... ok
test test_validated_graph_derives_debug_and_clone ... ok
test test_validated_graph_inner_is_pub_crate ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All 15 tests pass (9 existing from prior tasks + 5 new).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, all files formatted)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.45s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.42s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.92s
```

All four platform cross-checks passed.

## Project Gates

```
# Gate 1 — Config Surface Sync
Running tests/config_reference.rs
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```

## Public API Delta

```
+pub mod dag;
+pub use dag::validate_graph;
```

Two new public items introduced, matching the plan's Public API Surface table:
- `pub mod dag` — module declaration in `anvilml-scheduler::lib.rs`
- `pub use dag::validate_graph` — re-export of the validation function in `anvilml-scheduler::lib.rs`

## Deviations from Plan

1. **Test expectations corrected**: The plan's test table specified `Err([DuplicateNodeId("a"), DuplicateNodeId("a")])` (two entries) for the single-duplicate test and `Err([DuplicateNodeId("a"), DuplicateNodeId("b"), DuplicateNodeId("a"), DuplicateNodeId("b")])` (four entries) for the multi-duplicate test. The plan's Approach section (Step 3) describes the algorithm as "if an id is already in the set, push DuplicateNodeId" — which reports only the 2nd+ occurrence per ID. The test expectations were inconsistent with the algorithm. Tests were corrected to match the algorithm: 1 error for the single-duplicate case (second "a"), 2 errors for the multi-duplicate case (second "a" and second "b").

## Blockers

None.
