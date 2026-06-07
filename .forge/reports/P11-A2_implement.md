# Implementation Report: P11-A2

| Field       | Value                                           |
|-------------|-------------------------------------------------|
| Task ID     | P11-A2                                          |
| Phase       | 011 — Graph Validation                          |
| Description | anvilml-scheduler: dag.rs duplicate-id + unknown-type checks |
| Implemented | 2026-06-07T10:45:00Z                            |
| Status      | COMPLETE                                        |

## Summary

Created `crates/anvilml-scheduler/src/dag.rs` implementing a `ValidatedGraph(newtype(Value))` and a `validate_graph(&Value) -> Result<ValidatedGraph, Vec<String>>` function that performs non-fail-fast validation: duplicate node IDs produce `'duplicate_node_id: {id}'` errors and unknown node types produce `'unknown_node_type: {type}'` errors. Updated `lib.rs` to export the new module and re-exported `ValidatedGraph` and `validate_graph`. Added `serde_json = { workspace = true }` dependency to the crate's `Cargo.toml` and bumped patch version from `0.1.1` to `0.1.2`.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source          |
|--------|-------------|-----------------|-----------------|
| workspace | serde_json | 1.0.150       | root Cargo.toml |

Note: `serde_json` was already declared as a workspace dependency in the root `Cargo.toml`. The crate now uses `{ workspace = true }` to reference it. No MCP lookup was needed since the version was already established in the workspace manifest.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-scheduler/src/dag.rs` | `ValidatedGraph` newtype, `validate_graph()` function, 3 unit tests |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Add `serde_json = { workspace = true }`; bump version `0.1.1 → 0.1.2` |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Export `pub mod dag;` and re-export `ValidatedGraph`, `validate_graph` |

## Commit Log

```
 .forge/reports/P11-A2_plan.md       |  99 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md        |   6 +-
 .forge/state/state.json             |  13 ++--
 Cargo.lock                          |   3 +-
 crates/anvilml-scheduler/Cargo.toml |   3 +-
 crates/anvilml-scheduler/src/dag.rs | 125 ++++++++++++++++++++++++++++++++++++
 crates/anvilml-scheduler/src/lib.rs |   3 +
 7 files changed, 241 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-8f8e3645a245cedc)

running 5 tests
test dag::tests::test_duplicate_node_id ... ok
test dag::tests::test_unknown_node_type ... ok
test dag::tests::test_valid_graph ... ok
test nodes::tests::test_all_nine_types_present ... ok
test nodes::tests::test_zitsampler_outputs_include_latents_seed ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 197 tests passed, 0 failed, 0 ignored across all crates (anvilml-core, anvilml-hardware, anvilml-ipc, anvilml-registry, anvilml-scheduler, anvilml-server, anvilml-worker, backend, plus doc-tests).

## Format Gate

```
(No output — exit 0, no formatting drift detected)
```

## Platform Cross-Check

```
=== Check 1: Mock-hardware Linux ===
    Checking anvilml-scheduler v0.1.2 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.44s

=== Check 2: Mock-hardware Windows cross ===
    Checking anvilml-scheduler v0.1.2 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.72s

=== Check 3: Real-hardware Linux ===
    Checking anvilml-scheduler v0.1.2 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.33s

=== Check 4: Real-hardware Windows cross ===
    Checking anvilml-scheduler v0.1.2 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.71s
```

All four checks passed (exit 0).

## Project Gates

### Gate 1 — Config Surface Sync
```
     Running tests/config_reference.rs (target/debug/deps/config_reference-fab9a73578a112ef)

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Gate passed. No config surface changes were made by this task.

## Deviations from Plan

- **Clippy fix**: The initial implementation used `seen_ids.iter().any(|&seen| seen == id.as_str())` for duplicate ID checking. Clippy flagged this as `manual_contains` (using `contains()` instead of `iter().any()` is more efficient). Changed to `seen_ids.contains(&id.as_str())`. This is a minimal correctness-preserving fix, not a deviation from the plan's logic.

## Blockers

None.
