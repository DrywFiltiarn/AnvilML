# Implementation Report: P16-B1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P16-B1                          |
| Phase         | 16 — Live Events                |
| Description   | anvilml-server: AppState gains broadcaster field, wired from main.rs |
| Implemented   | 2026-07-09T15:30:00Z            |
| Status        | COMPLETE                          |

## Summary

Connected the HTTP layer's WebSocket subscribers to the scheduler's event loop by adding a `broadcaster: Arc<EventBroadcaster>` field to `AppState`, wiring a single `EventBroadcaster` instance from `backend/src/main.rs`, and calling `spawn_event_loop()` with the same `Arc` instance. The broadcaster is constructed after the worker pool, shared via `Arc::clone()` into both `AppState` and `spawn_event_loop()`, and its `JoinHandle` is properly aborted and awaited during graceful shutdown. Two new tests verify construction and cloning semantics. All 262 workspace tests pass.

## Resolved Dependencies

| Type   | Name              | Version resolved | Source         |
|--------|-------------------|------------------|----------------|
| crate  | anvilml-ipc       | 0.1.12 (path dep) | Codebase inspection |
| crate  | EventBroadcaster  | Already in workspace (anvilml-ipc) | Codebase inspection |
| crate  | spawn_event_loop  | Already in workspace (anvilml-scheduler) | Codebase inspection |

No new external dependencies introduced. `anvilml-ipc` was added as a path dependency to `backend/Cargo.toml` to access `EventBroadcaster`. `EventBroadcaster` and `spawn_event_loop()` were already exported from existing workspace crates.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-server/src/state.rs | Added `use anvilml_ipc::EventBroadcaster` import and `pub broadcaster: Arc<EventBroadcaster>` field to `AppState` struct with doc comment. |
| Modify | crates/anvilml-server/tests/state_tests.rs | Added `use anvilml_ipc::EventBroadcaster` import; updated `make_full_state()` to construct and include `broadcaster`; updated `test_app_state_constructs()` to include `broadcaster`; added `test_app_state_broadcaster_constructs()` and `test_app_state_broadcaster_clone_shares()` tests. |
| Modify | crates/anvilml-server/tests/health_tests.rs | Added `use anvilml_ipc::EventBroadcaster` import and `broadcaster` field to `make_test_state()`. |
| Modify | crates/anvilml-server/tests/artifacts_tests.rs | Added `use anvilml_ipc::EventBroadcaster` import and `broadcaster` field to `make_test_state()`. |
| Modify | crates/anvilml-server/tests/cors_tests.rs | Added `use anvilml_ipc::EventBroadcaster` import and `broadcaster` field to `make_test_state()`. |
| Modify | crates/anvilml-server/tests/nodes_tests.rs | Added `use anvilml_ipc::EventBroadcaster` import and `broadcaster` field to `make_test_state()`. |
| Modify | crates/anvilml-server/tests/jobs_tests.rs | Added `use anvilml_ipc::EventBroadcaster` import and `broadcaster` field to `make_test_state()`. |
| Modify | backend/src/main.rs | Added `EventBroadcaster` and `spawn_event_loop` imports; constructed `broadcaster` after `workers`; added `broadcaster` to `AppState` struct literal; called `spawn_event_loop()` with `Arc::clone(&scheduler)`, `Arc::clone(workers.demux())`, `Arc::clone(&broadcaster)`, `Arc::clone(&workers)`; added `event_loop_handle` abort and await in graceful shutdown. |
| Modify | backend/Cargo.toml | Added `anvilml-ipc = { path = "../crates/anvilml-ipc" }` dependency. |
| Modify | crates/anvilml-server/Cargo.toml | Bumped patch version `0.1.11` → `0.1.12`. |

## Commit Log

```
 .forge/reports/P16-B1_plan.md                  | 162 +++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                   |   6 +-
 .forge/state/state.json                        |  14 +--
 Cargo.lock                                     |   3 +-
 backend/Cargo.toml                             |   1 +
 backend/src/main.rs                            |  30 +++++
 crates/anvilml-server/Cargo.toml               |   2 +-
 crates/anvilml-server/src/state.rs             |   8 ++
 crates/anvilml-server/tests/artifacts_tests.rs |   2 +
 crates/anvilml-server/tests/cors_tests.rs      |   2 +
 crates/anvilml-server/tests/health_tests.rs    |   2 +
 crates/anvilml-server/tests/jobs_tests.rs      |   2 +
 crates/anvilml-server/tests/nodes_tests.rs     |   2 +
 crates/anvilml-server/tests/state_tests.rs     |  52 ++++++++
 14 files changed, 276 insertions(+), 12 deletions(-)
```

## Test Results

```
     Running tests/state_tests.rs (target/debug/deps/state_tests-616d394fd0e914fd)

running 9 tests
test test_app_state_scheduler_arc_sharing ... ok
test test_app_state_with_new_fields ... ok
test test_app_state_artifact_store_constructs ... ok
test test_app_state_broadcaster_constructs ... ok
test test_app_state_artifact_store_clone_shares ... ok
test test_app_state_clone_shares_node_registry ... ok
test test_app_state_clone_preserves_all_fields ... ok
test test_app_state_broadcaster_clone_shares ... ok
test test_app_state_constructs ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace: 262 tests passed, 0 failed. All existing tests compile and pass with the new `broadcaster` field. The two new tests (`test_app_state_broadcaster_constructs` and `test_app_state_broadcaster_clone_shares`) verify construction and `Arc`-sharing semantics.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

All four platform cross-checks passed:

1. Mock-hardware Linux: `cargo check --workspace --features mock-hardware` — Finished
2. Mock-hardware Windows: `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — Finished
3. Real-hardware Linux: `cargo check --bin anvilml` — Finished
4. Real-hardware Windows: `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — Finished

## Project Gates

Gate 1 — Config Surface Sync:
```
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```

Gate 2 — OpenAPI Drift: Not triggered (no handler signatures, ToSchema derives, or handler-visible AppState fields changed).

Gate 3 — Node Parity: Not triggered (no node types added/removed/renamed).

Gate 4 — Mock/Real Parity Markers: Not triggered (no node execute() or arch module load()/sample()/decode() modified).

## Public API Delta

```
+    pub broadcaster: Arc<EventBroadcaster>,
```

One new `pub` item introduced: the `broadcaster` field on `AppState` in `crates/anvilml-server/src/state.rs`. This matches the plan's Public API Surface exactly — an additive field on an existing struct, no new functions, traits, or types.

## Deviations from Plan

- Added `anvilml-ipc` as a path dependency to `backend/Cargo.toml`. The plan did not explicitly list this file, but it was required because `EventBroadcaster` is defined in `anvilml-ipc` and `main.rs` constructs it directly. Without this dependency, the build fails with `unresolved import anvilml_ipc`.
- Added `spawn_event_loop()` call from `main.rs` (the plan's revised step 5 explicitly required this). The function is imported from `anvilml_scheduler::spawn_event_loop`.
- Updated all five existing integration test files (`health_tests.rs`, `artifacts_tests.rs`, `cors_tests.rs`, `nodes_tests.rs`, `jobs_tests.rs`) to include the `broadcaster` field — these were not listed in the plan's Files Affected but were required to fix compile errors.

## Blockers

None.
