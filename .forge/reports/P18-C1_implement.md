# Implementation Report: P18-C1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P18-C1                          |
| Phase         | 18 — HTTP/WebSocket Server Completion |
| Description   | anvilml-server: AppState gains model_store; GET /v1/models, /v1/models/:id |
| Implemented   | 2026-07-12T09:45:00Z            |
| Status        | COMPLETE                        |

## Summary

Wired the model registry into the HTTP server: added `model_store: Arc<ModelStore>` to `AppState`, created a new `handlers/models.rs` module with `list_models()` and `get_model()` thin-delegation handlers, registered two GET routes (`/v1/models` and `/v1/models/{id}`) in `build_router()`, and wrote 4 integration tests. All 376 workspace tests pass. The model store field was also added to the backend binary's `AppState` construction and all 8 existing test files that construct `AppState` directly were updated.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|------------|------------------|----------------|
| crate  | anvilml-registry | 0.1.x (path dep) | Cargo.toml    |
| crate  | anvilml-core    | 0.1.x (path dep) | Cargo.toml    |

No new external dependencies introduced. All types (`ModelStore`, `ModelKind`, `ModelMeta`, `AnvilError`) come from crates already declared as path dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-server/src/state.rs | Add `model_store: Arc<ModelStore>` field to AppState with doc comment and import |
| CREATE | crates/anvilml-server/src/handlers/models.rs | New handler module with `list_models()` and `get_model()` |
| Modify | crates/anvilml-server/src/handlers/mod.rs | Add `pub mod models;` |
| Modify | crates/anvilml-server/src/lib.rs | Register `GET /v1/models` and `GET /v1/models/{id}` routes in `build_router()` |
| CREATE | crates/anvilml-server/tests/models_tests.rs | 4 integration tests for the model handlers |
| Modify | crates/anvilml-server/Cargo.toml | Bump patch version 0.1.19 → 0.1.20 |
| Modify | backend/src/main.rs | Add `model_store` field to AppState construction with ModelStore import |
| Modify | crates/anvilml-server/tests/health_tests.rs | Add `model_store` field to `make_test_state()` |
| Modify | crates/anvilml-server/tests/nodes_tests.rs | Add `model_store` field to `make_test_state()` |
| Modify | crates/anvilml-server/tests/cors_tests.rs | Add `model_store` field to `make_test_state()` |
| Modify | crates/anvilml-server/tests/system_tests.rs | Add `model_store` field to `make_test_state()` |
| Modify | crates/anvilml-server/tests/handler_tests.rs | Add `model_store` field to `make_test_state()` |
| Modify | crates/anvilml-server/tests/artifacts_tests.rs | Add `model_store` field to `make_test_state()` |
| Modify | crates/anvilml-server/tests/state_tests.rs | Add `model_store` field to both AppState constructions |
| Modify | crates/anvilml-server/tests/jobs_tests.rs | Add `model_store` field to `make_test_state()` |
| Modify | docs/TESTS.md | Add 4 entries for new model handler tests |

## Commit Log

 .forge/reports/P18-C1_plan.md                  | 184 +++++++++++++++
 .forge/state/CURRENT_TASK.md                   |   6 +-
 .forge/state/state.json                        |  13 +-
 Cargo.lock                                     |   2 +-
 backend/src/main.rs                            |   7 +-
 crates/anvilml-server/Cargo.toml               |   2 +-
 crates/anvilml-server/src/handlers/mod.rs      |   1 +
 crates/anvilml-server/src/handlers/models.rs   |  64 ++++++
 crates/anvilml-server/src/lib.rs               |  10 +
 crates/anvilml-server/src/state.rs             |   8 +
 crates/anvilml-server/tests/artifacts_tests.rs |   3 +-
 crates/anvilml-server/tests/cors_tests.rs      |   5 +-
 crates/anvilml-server/tests/handler_tests.rs   |   5 +-
 crates/anvilml-server/tests/health_tests.rs    |   5 +-
 crates/anvilml-server/tests/jobs_tests.rs      |   5 +-
 crates/anvilml-server/tests/models_tests.rs    | 307 +++++++++++++++++++++++++
 crates/anvilml-server/tests/nodes_tests.rs     |   5 +-
 crates/anvilml-server/tests/state_tests.rs     |   6 +-
 crates/anvilml-server/tests/system_tests.rs    |   5 +-
 docs/TESTS.md                                  |  48 ++++
 20 files changed, 664 insertions(+), 27 deletions(-)

## Test Results

     Running tests/models_tests.rs (target/debug/deps/models_tests-8c89f83ac3081831)

running 4 tests
test test_get_model_unknown_returns_404 ... ok
test test_get_model_existing_returns_200 ... ok
test test_list_models_no_filter ... ok
test test_list_models_kind_filter ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Full workspace test suite: 376 passed, 0 failed across all crates.

## Format Gate

`cargo fmt --all -- --check` exited 0 (clean after fix-up pass).

## Platform Cross-Check

All 4 checks passed:
1. `cargo check --workspace --features mock-hardware` — ok
2. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — ok
3. `cargo check --bin anvilml` — ok
4. `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — ok

## Project Gates

Gate 1 — Config Surface Sync: `cargo test -p anvilml --features mock-hardware -- config_reference` exited 0 (1 passed).

## Public API Delta

```
+pub mod models;
+    pub model_store: Arc<ModelStore>,
```

New public items:
- `pub mod models` — module path `anvilml_server::handlers::models`
- `pub model_store: Arc<ModelStore>` — field on `AppState` at `anvilml_server::state::AppState`

The handler functions `list_models` and `get_model` are not `pub` (they use `pub(crate)` via axum's handler trait), matching the plan's Public API Surface table which listed them as `pub(crate)`.

## Deviations from Plan

- `backend/src/main.rs` was modified to add `model_store: Arc::new(ModelStore::new(pool.clone()))` to the `AppState` construction. The plan said "the construction site in main.rs is handled by a later task" but adding the field to `AppState` made this a compile requirement, not a scope choice.
- All 8 existing test files in `crates/anvilml-server/tests/` that construct `AppState` directly were updated with the `model_store` field. This was a compile requirement caused by adding a new required field to `AppState`.
- The `jobs_tests.rs` `make_test_state()` was changed from `db` to `db: db.clone()` to avoid a borrow-after-move error with the new `model_store` field.

## Blockers

None.
