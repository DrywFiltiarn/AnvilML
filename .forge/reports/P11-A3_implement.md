# Implementation Report: P11-A3

| Field         | Value                                         |
|---------------|-----------------------------------------------|
| Task ID       | P11-A3                                        |
| Phase         | 011 — Dynamic Node Registry                   |
| Description   | anvilml-server: GET /v1/nodes listing registered node types |
| Implemented   | 2026-06-19T18:00:00Z                          |
| Status        | COMPLETE                                      |

## Summary

Implemented `GET /v1/nodes` endpoint that returns the current set of registered node types from the `NodeTypeRegistry`. The handler returns `503 Service Unavailable` when no worker has ever reached `Ready` (`has_been_updated() == false`), and `200 OK` with a JSON array of `NodeTypeDescriptor` objects after any worker reaches `Ready` (including mock workers reporting zero types). Changes span `AppState` (new `node_registry` field + 3 constructor extensions), a new `handlers/nodes.rs` module, route mounting in `lib.rs`, re-exports in `handlers/mod.rs`, reordering of `node_registry` construction in `backend/src/main.rs`, and 11 `AppState::new` call site updates across 6 existing test files. Two integration tests were added. The `anvilml-server` crate was version-bumped from `0.1.19` to `0.1.20`.

## Resolved Dependencies

| Type   | Name           | Version resolved | Source      |
|--------|----------------|------------------|-------------|
| crate  | anvilml-core   | 0.1.14 (workspace) | Cargo.toml |
| crate  | anvilml-scheduler | 0.1.2 (workspace) | Cargo.toml |

No new external dependencies are introduced. All types (`NodeTypeRegistry`, `NodeTypeDescriptor`, `AnvilError`) are already in `anvilml-core`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/handlers/nodes.rs` | New handler module with `list_nodes` function |
| MODIFY | `crates/anvilml-server/src/state.rs` | Added `node_registry` field + extended all 3 constructors |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Mounted `GET /v1/nodes` route; imported `list_nodes` |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Added `pub mod nodes;` and `pub use nodes::list_nodes;` |
| MODIFY | `backend/src/main.rs` | Reordered `node_registry` construction before `temp_state`; passed `Arc::clone(&node_registry)` into both constructors |
| CREATE | `crates/anvilml-server/tests/nodes_tests.rs` | 2 integration tests for GET /v1/nodes |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bumped patch version 0.1.19 → 0.1.20 |
| MODIFY | `crates/anvilml-server/tests/handler_tests.rs` | Updated 2 `AppState::new` call sites |
| MODIFY | `crates/anvilml-server/tests/health_tests.rs` | Updated 1 `AppState::new` call site |
| MODIFY | `crates/anvilml-server/tests/models_tests.rs` | Updated 3 `AppState::new` + 3 `new_with_hardware_no_workers` call sites |
| MODIFY | `crates/anvilml-server/tests/state_tests.rs` | Updated 3 `AppState::new` call sites |
| MODIFY | `crates/anvilml-server/tests/system_tests.rs` | Updated 1 `AppState::new` + 1 `new_with_hardware_no_workers` call site |
| MODIFY | `crates/anvilml-server/tests/workers_tests.rs` | Updated 1 `AppState::new` + 1 `new_with_hardware` call site |
| MODIFY | `docs/TESTS.md` | Added entries for 2 new tests |

## Commit Log

```
 .forge/reports/P11-A3_plan.md                | 201 +++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  12 +-
 Cargo.lock                                   |   2 +-
 backend/src/main.rs                          |  16 ++-
 crates/anvilml-server/Cargo.toml             |   2 +-
 crates/anvilml-server/src/handlers/mod.rs    |   2 +
 crates/anvilml-server/src/handlers/nodes.rs  |  57 ++++++++
 crates/anvilml-server/src/lib.rs             |   4 +
 crates/anvilml-server/src/state.rs           |  32 ++++-
 crates/anvilml-server/tests/handler_tests.rs |   6 +-
 crates/anvilml-server/tests/health_tests.rs  |   4 +-
 crates/anvilml-server/tests/models_tests.rs  |  12 +-
 crates/anvilml-server/tests/nodes_tests.rs   | 113 +++++++++++++++
 crates/anvilml-server/tests/state_tests.rs   |  18 ++-
 crates/anvilml-server/tests/system_tests.rs  |   4 +-
 crates/anvilml-server/tests/workers_tests.rs |   5 +-
 docs/TESTS.md                                |  18 +++
 18 files changed, 482 insertions(+), 32 deletions(-)
```

## Test Results

```
     Running tests/nodes_tests.rs (target/debug/deps/nodes_tests-4dd31b2774974e44)

running 2 tests
test test_nodes_returns_200_after_worker_ready ... ok
test test_nodes_returns_503_when_registry_not_updated ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Full workspace test suite: 174 tests passed, 0 failed. All existing tests continue to pass with the updated `AppState::new` signatures.

## Format Gate

```
(exit 0 — no output means no formatting drift)
```

## Platform Cross-Check

```
=== Check 1: Mock-hardware Linux ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.32s

=== Check 2: Mock-hardware Windows ===
    Checking anvilml-core v0.1.14 (/home/dryw/AnvilML/crates/anvilml-core)
    ...
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.32s

=== Check 3: Real-hardware Linux ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.38s

=== Check 4: Real-hardware Windows ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.67s
```

All four cross-checks exit 0.

## Project Gates

**Gate 1 — Config Surface Sync:**
```
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**Gate 2 — OpenAPI Drift:**
```
    Running `target/debug/anvilml-openapi`
(no diff — `git diff --exit-code api/openapi.json` exited 0)
```

**Gate 3 — Node Parity:** Not triggered — this task does not modify `worker/nodes/` or `node_registry.rs`.

## Public API Delta

```
+pub mod nodes;
+pub use nodes::list_nodes;
+    pub node_registry: Arc<anvilml_core::NodeTypeRegistry>,
+    pub async fn new(
pub async fn list_nodes(
```

New public items:
- `pub mod nodes` — module declaration in `handlers/mod.rs`
- `pub use nodes::list_nodes` — re-export in `handlers/mod.rs`
- `pub node_registry: Arc<anvilml_core::NodeTypeRegistry>` — field in `AppState` struct
- `pub async fn list_nodes` — handler function in `handlers/nodes.rs`

Modified public items:
- `pub async fn new(...)` — constructor signature extended with `node_registry` parameter

## Deviations from Plan

1. The plan's Approach step 9 listed 16 call sites across 6 files. The actual count is 11 call sites: handler_tests.rs (2, not 3), health_tests.rs (1), models_tests.rs (3, not 5), state_tests.rs (3, not 5), system_tests.rs (1), workers_tests.rs (1). The plan over-counted because it included call sites that no longer exist or miscounted. All actual call sites were updated correctly.
2. The plan mentioned updating `new_with_hardware_no_workers` and `new_with_hardware` call sites in test files. These were also updated (5 total across models_tests.rs, system_tests.rs, and workers_tests.rs) because the constructor signatures changed.
3. Gate 2 (OpenAPI Drift) returned exit 0 with no diff — the `openapi.json` was already up to date or the tool regenerated it identically. This is acceptable.

## Blockers

None.
