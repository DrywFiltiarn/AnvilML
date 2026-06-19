# Implementation Report: P12-A1

| Field         | Value                                              |
|---------------|----------------------------------------------------|
| Task ID       | P12-A1                                             |
| Phase         | 012 — Graph Validation                             |
| Description   | anvilml-scheduler: dag.rs validate_graph collect-all-errors mode |
| Implemented   | 2026-06-19T18:15:00Z                               |
| Status        | COMPLETE                                           |

## Summary

Implemented `validate_graph` in `crates/anvilml-scheduler/src/dag.rs`, a non-fail-fast function that collects all six validation errors (missing nodes array, duplicate IDs, unknown node types, bad edge references, slot type mismatches, and cycles) before returning. The function returns `Ok(ValidatedGraph(serde_json::Value))` only when all checks pass. Created 10 integration tests covering each validation check and the non-fail-fast behaviour. Added `serde_json` and `indexmap` dependencies, bumped crate version to 0.1.3.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source        |
|--------|-----------|------------------|---------------|
| crate  | serde_json| 1.0.150          | workspace     |
| crate  | indexmap  | 2.7              | crates.io     |

`serde_json` was already in `[workspace.dependencies]` at 1.0.150. `indexmap` was added as a direct dependency of `anvilml-scheduler` at version 2.7 for deterministic iteration in Kahn's algorithm.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/dag.rs` | New file; `ValidatedGraph` newtype, `validate_graph` function, 6 private check functions, `types_compatible` helper, `extract_edges` helper |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Added `pub mod dag;` declaration |
| CREATE | `crates/anvilml-scheduler/tests/dag_tests.rs` | New test file; 10 tests covering all validation checks |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Added `serde_json = { workspace = true }`, `indexmap = "2.7"`; bumped version 0.1.2 → 0.1.3 |
| MODIFY | `docs/TESTS.md` | Added 10 entries for new dag_tests.rs tests |

## Commit Log

```
 .forge/reports/P12-A1_plan.md               | 205 ++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +-
 Cargo.lock                                  |   4 +-
 crates/anvilml-scheduler/Cargo.toml         |   4 +-
 crates/anvilml-scheduler/src/dag.rs         | 511 ++++++++++++++++++++++++
 crates/anvilml-scheduler/src/lib.rs         |   2 +
 crates/anvilml-scheduler/tests/dag_tests.rs | 576 ++++++++++++++++++++++++++++
 docs/TESTS.md                               |  90 +++++
 9 files changed, 1400 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/dag_tests.rs (target/debug/deps/dag_tests-334b06ad0f170354)

running 10 tests
test test_any_slot_type_compatible ... ok
test test_bad_edge_ref_missing_node ... ok
test test_bad_edge_ref_missing_slot ... ok
test test_cycle_detected ... ok
test test_duplicate_node_ids ... ok
test test_missing_nodes_array ... ok
test test_multiple_errors_collected ... ok
test test_slot_type_mismatch ... ok
test test_unknown_node_type ... ok
test test_valid_graph_returns_ok ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace: 180+ tests, 0 failures.

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no formatting drift.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.54s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.77s

# 3. Real-hardware Linux
cargo check --bin anvilml
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.50s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.67s
```
All 4 checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
→ test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Public API Delta

```
pub mod dag;                          (lib.rs — module declaration)
pub struct ValidatedGraph(pub serde_json::Value);  (dag.rs — newtype)
pub async fn validate_graph(...)     (dag.rs — main function)
```

Three new `pub` items: `ValidatedGraph` struct, `validate_graph` function, and `dag` module declaration. All match the plan's Public API Surface table.

## Deviations from Plan

- **`check_edge_refs` signature changed**: The plan specified `fn check_edge_refs(nodes: &[Value]) -> Vec<String>`, but the implementation requires `async fn check_edge_refs(graph: &Value, nodes: &[&Value], registry: &NodeTypeRegistry) -> Vec<String>`. This was necessary because edge slot validation requires looking up output slot definitions from the registry's `NodeTypeDescriptor`, not from the raw graph JSON (which only contains `{id, type}` per node). The registry lookup is async, so the function had to be async.
- **Test assertion for slot type names**: The plan's `test_slot_type_mismatch` expected error messages containing `"MODEL"` and `"IMAGE"`, but `SlotType`'s `Debug` format uses PascalCase (`Model`, `Image`). Adjusted assertions to match the actual debug output.
- **Version bump timing**: The plan listed version bump as part of step 1 (dependency addition). Followed the ACT process which separates dependency addition (step 1) from version bump (step 5).

## Blockers

None.
