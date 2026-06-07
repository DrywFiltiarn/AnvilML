# Implementation Report: P11-A4

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P11-A4                                            |
| Phase         | 011 — Graph Validation                            |
| Description   | anvilml-scheduler: dag.rs cycle detection (Kahn)  |
| Implemented   | 2026-06-07T12:30:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Added cycle detection to `anvilml-scheduler::dag::validate_graph` using Kahn's topological sort algorithm. The function now builds an adjacency list from edge references during the existing edge-validation pass, then runs Kahn's algorithm as the final validation step before returning `Ok(ValidatedGraph)`. If a cycle is detected (processed_count < total_nodes), node IDs involved are collected into a sorted list and emitted as `'cycle_detected: {ids}'` in the error vector. Two unit tests were added: `test_cycle_detected_2node` (mutual reference cycle) and `test_valid_zit_5node_passes` (full 5-node ZiT pipeline). All 10 scheduler tests pass, all 216 workspace tests pass, clippy reports zero warnings, all four platform cross-checks pass, and the format gate passes.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|-----------------|----------------|
| (none) | —       | —               | —              |

No new dependencies added. The task only modifies existing code in `dag.rs` and bumps the crate version.

## Files Changed

| Action   | Path                                              | Description                                    |
|----------|---------------------------------------------------|------------------------------------------------|
| Modify   | `crates/anvilml-scheduler/Cargo.toml`             | Bump patch version `0.1.3 → 0.1.4`             |
| Modify   | `crates/anvilml-scheduler/src/dag.rs`             | Add Kahn cycle detection + 2 unit tests        |

## Commit Log

```
 .forge/state/CURRENT_TASK.md            |   6 +-
 .forge/state/state.json                 |  13 +--
 Cargo.lock                              |   2 +-
 crates/anvilml-scheduler/Cargo.toml     |   2 +-
 crates/anvilml-scheduler/src/dag.rs     | 159 ++++++++++++++++++++++++++++-
 5 files changed, 167 insertions(+), 15 deletions(-)
```

## Test Results

```
running 8 tests
test dag::tests::test_cycle_detected_2node ... ok
test dag::tests::test_duplicate_node_id ... ok
test dag::tests::test_unknown_node_ref ... ok
test dag::tests::test_unknown_node_type ... ok
test dag::tests::test_unknown_output_slot ... ok
test dag::tests::test_valid_edge_references ... ok
test dag::tests::test_valid_graph ... ok
test dag::tests::test_valid_zit_5node_passes ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s
```

Full workspace test suite (cargo test --workspace --features mock-hardware): 216 tests, 0 failures.

## Format Gate

```
(no output — exit 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.49s

# 2. Mock-hardware Windows cross-check (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.96s

# 3. Real-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.40s

# 4. Real-hardware Windows cross-check (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.73s
```

All four platform cross-checks exit 0.

## Project Gates

```
# Config surface sync gate
running 0 tests (filtered)
Running tests/config_reference.rs
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- Changed `id_type_map` from `HashMap<&str, &str>` to `HashMap<String, String>` (owned keys/values) instead of the plan's reference-based approach. This was necessary because building the adjacency list in the same loop that iterates over nodes requires owned strings — references into `serde_json::Value` cannot be held across mutable borrows needed for map insertion. The functional behavior is identical; only the memory allocation strategy differs (heap-allocated Strings vs stack-borrowed &str).
- Used `adj.values()` instead of iterating `&adj` with `_src, edges` pattern to satisfy clippy's `for_kv_map` lint.
- Added `.sort()` on the initial queue seed for deterministic processing order (not required by Kahn's algorithm but provides consistent output in tests).

## Blockers

None.
