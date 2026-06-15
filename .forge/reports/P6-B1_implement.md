# Implementation Report: P6-B1

| Field         | Value                                                         |
|---------------|---------------------------------------------------------------|
| Task ID       | P6-B1                                                         |
| Phase         | 006 — Model Registry                                          |
| Description   | anvilml-server: GET /v1/models and GET /v1/models/:id handlers |
| Implemented   | 2026-06-15T22:30:00Z                                          |
| Status        | COMPLETE                                                      |

## Summary

Implemented two HTTP handlers (`list_models` and `get_model`) for the model registry in the `anvilml-server` crate. Added `registry: Arc<ModelStore>` to `AppState`, created `handlers/models.rs` with the two handlers, wired routes into `build_router`, updated `AppState::new_with_hardware` to accept a `ModelStore` parameter, fixed `AppState::new()` to use `open_in_memory()` (so migrations run), and created three integration tests. Bumped `anvilml-server` version from 0.1.7 to 0.1.8.

## Resolved Dependencies

| Type   | Name           | Version resolved | Source        |
|--------|----------------|------------------|---------------|
| crate  | anvilml-registry| 0.1.6           | Cargo.lock    |
| crate  | serde          | 1.0.228          | workspace     |
| crate  | chrono         | 0.4.x            | workspace     |

`anvilml-registry` was already at 0.1.6 in Cargo.lock (path dependency). `serde` was added as a workspace dependency for the `Deserialize` derive on `ModelsFilter`. `chrono` was added to dev-dependencies for `Utc::now()` in tests.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-server/Cargo.toml` | Added `anvilml-registry` to `[dependencies]`, added `serde` and `chrono` to deps/dev-deps; bumped version 0.1.7 → 0.1.8 |
| MODIFY | `crates/anvilml-server/src/state.rs` | Added `registry: Arc<ModelStore>` field to `AppState`; updated `new()` to use `open_in_memory()`; updated `new_with_hardware()` to accept `Arc<ModelStore>` parameter |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Added `pub mod models;` declaration |
| CREATE | `crates/anvilml-server/src/handlers/models.rs` | New handler module with `list_models` and `get_model` functions |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Added route imports, mounted `GET /v1/models` and `GET /v1/models/{id}` in `build_router` |
| MODIFY | `backend/src/main.rs` | Added `ModelStore` import, constructed `ModelStore` before `AppState::new_with_hardware()`, passed it as 4th argument |
| CREATE | `crates/anvilml-server/tests/models_tests.rs` | Integration tests for model endpoints (3 tests) |
| MODIFY | `crates/anvilml-server/tests/system_tests.rs` | Updated `new_with_hardware` call to include `ModelStore` argument |
| MODIFY | `docs/TESTS.md` | Added 3 test entries for new models tests |

## Commit Log

```
 .forge/reports/P6-B1_plan.md                 | 147 ++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   4 +-
 backend/src/main.rs                          |   8 +-
 crates/anvilml-server/Cargo.toml             |   5 +-
 crates/anvilml-server/src/handlers/mod.rs    |   1 +
 crates/anvilml-server/src/handlers/models.rs |  79 ++++++++++++
 crates/anvilml-server/src/lib.rs             |  16 ++-
 crates/anvilml-server/src/state.rs           |  37 ++++--
 crates/anvilml-server/tests/models_tests.rs  | 182 +++++++++++++++++++++++++++
 crates/anvilml-server/tests/system_tests.rs  |   9 +-
 docs/TESTS.md                                |  27 ++++
 13 files changed, 505 insertions(+), 29 deletions(-)
```

## Test Results

```
     Running tests/models_tests.rs (target/debug/deps/models_tests-103525a59ccfc2a2)

running 3 tests
test test_list_models_empty ... ok
test test_get_model_not_found ... ok
test test_list_models_with_kind_filter ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

All other workspace tests pass. One pre-existing test (`test_custom_port_health` in `backend/tests/cli_tests.rs`) requires `lsof`/`ss` which are not available in this environment — this is unrelated to this task's changes.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
CHECK 1 (mock Linux):  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.67s — OK
CHECK 2 (mock Windows): Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.98s — OK
CHECK 3 (real Linux):  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.80s — OK
CHECK 4 (real Windows): Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.96s — OK
```

## Project Gates

Gate 1 (Config Surface Sync):
```
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 2 (OpenAPI Drift):
```
(cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json exited 0 — no drift)
```

## Public API Delta

```
+pub mod models;
+    pub registry: Arc<anvilml_registry::ModelStore>,
```

Two new public items introduced:
- `pub mod models` — new handler module (in `anvilml_server::handlers`)
- `pub registry: Arc<ModelStore>` — new field on `AppState` (in `anvilml_server::state`)

The handler functions `list_models` and `get_model` are `pub(crate)` (crate-private) since they are only used internally by `build_router`.

## Deviations from Plan

- **Path parameter syntax**: The plan specified `GET /v1/models/:id` but axum 0.8.x requires `{id}` syntax. Changed to `GET /v1/models/{id}`.
- **`AppState::new()` uses `open_in_memory()`**: The plan said `AppState::new()` would construct the `ModelStore` from the in-memory pool. However, using raw `SqlitePool::connect()` would leave the database without migrations, causing 500 errors. Changed to use `anvilml_registry::open_in_memory()` which runs migrations, then passes the pool to `ModelStore::new()`.
- **`AppState::new_with_hardware` signature changed**: Added `Arc<ModelStore>` as a 4th parameter. Updated both `backend/src/main.rs` and `crates/anvilml-server/tests/system_tests.rs` to pass the argument.
- **Handler visibility**: The plan listed `list_models` and `get_model` as `pub async fn`. Changed to `pub(crate) async fn` because they only appear in the public signature of `list_models` (via `Query<ModelsFilter>`), and `ModelsFilter` is private. Making the handlers `pub` would expose a private type in the public API.
- **`serde` added as dependency**: The plan didn't mention `serde` as a dependency of `anvilml-server`, but it's needed for `#[derive(Deserialize)]` on `ModelsFilter`. Added `serde = { workspace = true }` to `[dependencies]`.
- **`chrono` added to dev-dependencies**: Needed for `Utc::now()` in the integration test. Added `chrono = { workspace = true }` to `[dev-dependencies]`.

## Blockers

None.
