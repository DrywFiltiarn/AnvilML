# Plan Report: P18-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-C1                                      |
| Phase       | 18 — HTTP/WebSocket Server Completion       |
| Description | anvilml-server: AppState gains model_store; GET /v1/models, /v1/models/:id |
| Depends on  | P18-A1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-12T07:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Wire the model registry into the HTTP server for the first time: add `model_store: Arc<ModelStore>` to `AppState`, create a new `handlers/models.rs` module with `list_models()` and `get_model()` handlers, register two GET routes (`/v1/models` and `/v1/models/:id`) in `build_router()`, and write >=4 integration tests. After this task, `GET /v1/models` returns all discovered models as JSON and `GET /v1/models/:id` returns a single model by ID or 404.

## Scope

### In Scope
- `crates/anvilml-server/src/state.rs` — add `model_store: Arc<ModelStore>` field to `AppState` with doc comment.
- `crates/anvilml-server/src/handlers/models.rs` — new file; `list_models()` and `get_model()` handler functions.
- `crates/anvilml-server/src/handlers/mod.rs` — add `pub mod models;`.
- `crates/anvilml-server/src/lib.rs` — register `GET /v1/models` and `GET /v1/models/:id` routes in `build_router()`.
- `crates/anvilml-server/tests/models_tests.rs` — new test file with >=4 tests.

### Out of Scope
- `POST /v1/models/rescan` — deferred to P18-C2, which genuinely states "Extend crates/anvilml-server/src/handlers/models.rs with rescan_models(...)" in its context and is a direct downstream prereq of P18-C1.
- `backend/src/main.rs` changes to populate `model_store` at startup — that is P18-C3's scope (startup auto-scan, which depends on P18-C2's rescan infrastructure). P18-C1 only wires the field into AppState; the construction site in main.rs is handled by a later task.
- `DELETE /v1/models/:id` — not part of the §13.4 route table.

## Existing Codebase Assessment

The codebase has a fully functional `ModelStore` in `anvilml-registry/src/store.rs` with `new()`, `upsert()`, `get()`, `list()`, and `delete()` methods — all `async`, all operating on a `SqlitePool`-backed `models` table. `ModelStore` is already re-exported from `anvilml-registry/src/lib.rs`.

The `anvilml-server` crate follows a thin-delegation pattern: handlers read from `AppState` fields via `.await` (RwLock reads or direct Arc access) and return JSON responses. The existing handler modules (`jobs.rs`, `system.rs`, `artifacts.rs`, `nodes.rs`) all use `axum::extract::{State, Path, Query}` and return `Result<Json<T>, AnvilError>` or `(StatusCode, Json<T>)`.

`AppState` currently has nine fields (config, node_registry, start_time, scheduler, workers, db, artifact_store, broadcaster, hardware, env_report) — the ten-field shape from §13.2 is almost complete; `model_store` is the missing tenth field. No prior phase wired `ModelStore` into `AppState`, which is the core gap this task fills.

The `AnvilError::ModelNotFound(String)` variant already exists and maps to HTTP 404, so no error type work is needed. `ModelKind` and `ModelMeta` are already defined in `anvilml-core` and derive `Serialize`/`Deserialize`/`ToSchema`.

## Resolved Dependencies

No new external dependencies are introduced. All types used (`ModelStore`, `ModelKind`, `ModelMeta`, `AnvilError`) come from crates already declared as path dependencies in `anvilml-server/Cargo.toml`.

| Type   | Name          | Version verified | MCP source | Feature flags confirmed |
|--------|---------------|-----------------|------------|------------------------|
| crate  | anvilml-registry | 0.1.x (path dep) | Cargo.toml | n/a (workspace path) |
| crate  | anvilml-core    | 0.1.x (path dep) | Cargo.toml | n/a (workspace path) |

## Approach

**Step 1 — Add `model_store` field to `AppState`.**

In `crates/anvilml-server/src/state.rs`, add a new field after `env_report`:

```rust
/// SQLite-backed model metadata store.
///
/// Provides CRUD operations on the `models` table: listing models,
/// fetching by ID, inserting/upserting via the scanner, and deleting.
/// Shared with `ModelStore` from `anvilml-registry` (Phase 6).
pub model_store: Arc<anvilml_registry::ModelStore>,
```

Add the import at the top of the file:
```rust
use anvilml_registry::ModelStore;
```

**Step 2 — Create `handlers/models.rs`.**

New file at `crates/anvilml-server/src/handlers/models.rs`. Implement two handler functions:

`list_models()` — accepts `State<AppState>` and optional `Query<ListModelsParams>`:
- Define `ListModelsParams { kind: Option<ModelKind> }` with `#[derive(Debug, Deserialize)]` — the `kind` field is optional; when `None`, all models are returned.
- Call `state.model_store.list(params.kind).await?` — delegates to `ModelStore::list()`.
- Return `Json(result)` — the `Vec<ModelMeta>` serialises directly via its `Serialize` derive.
- Add `#[tracing::instrument(skip(state), fields(kind))]` for observability.

`get_model()` — accepts `State<AppState>` and `Path<String>`:
- Call `state.model_store.get(&model_id).await?` — delegates to `ModelStore::get()`.
- If `get()` returns `Ok(None)`, return `Err(AnvilError::ModelNotFound(model_id))` — `AnvilError::IntoResponse` maps this to HTTP 404.
- If `get()` returns `Ok(Some(meta))`, return `Json(meta)`.
- Add `#[tracing::instrument(skip(state), fields(model_id))]` for observability.

Both functions follow the thin-delegation pattern: no business logic, just read-lock access, delegate, return.

**Step 3 — Register `models` module in `handlers/mod.rs`.**

Add `pub mod models;` to the existing module declarations.

**Step 4 — Register routes in `build_router()`.**

In `crates/anvilml-server/src/lib.rs`, add two route registrations before the closing `.with_state(app_state)`:

```rust
// GET /v1/models — list all models, optionally filtered by kind
.route(
    "/v1/models",
    axum::routing::get(handlers::models::list_models),
)
// GET /v1/models/{id} — look up a single model by its ID
.route(
    "/v1/models/{id}",
    axum::routing::get(handlers::models::get_model),
)
```

The `/v1/models` literal route must be registered before `/v1/models/{id}` so axum matches the literal path first (same pattern already used for `/v1/jobs` vs `/v1/jobs/{id}`).

**Step 5 — Write integration tests.**

New file at `crates/anvilml-server/tests/models_tests.rs`. Follow the exact same test pattern as `jobs_tests.rs`:
- Use `make_test_pool()` to create an in-memory SQLite pool with migrations applied.
- Create a `ModelStore` from the pool.
- Insert test model rows directly via `model_store.upsert()`.
- Build `AppState` with the `model_store` field populated.
- Use `build_router(state)` and `router.oneshot()` for in-process HTTP requests.

Four tests:
1. `test_list_models_no_filter` — insert 2 models of different kinds, call GET /v1/models, assert 200 and array length 2.
2. `test_list_models_kind_filter` — insert 3 models (2 diffusion, 1 vae), call GET /v1/models?kind=diffusion, assert 200 and array length 2.
3. `test_get_model_existing_returns_200` — insert 1 model, call GET /v1/models/{id}, assert 200 and body contains the correct id, name, and kind.
4. `test_get_model_unknown_returns_404` — call GET /v1/models/{random-uuid}, assert 404.

## Public API Surface

New public items (all in `anvilml-server` crate):

| Item | Path | Signature |
|------|------|-----------|
| struct | `handlers::models::ListModelsParams` | `pub(crate) struct ListModelsParams { pub kind: Option<ModelKind> }` |
| fn | `handlers::models::list_models` | `pub(crate) async fn list_models(State(state): State<AppState>, Query(params): Query<ListModelsParams>) -> Result<Json<Vec<ModelMeta>>, AnvilError>` |
| fn | `handlers::models::get_model` | `pub(crate) async fn get_model(State(state): State<AppState>, Path(model_id): Path<String>) -> Result<Json<ModelMeta>, AnvilError>` |

Modified public item:

| Item | Path | Change |
|------|------|--------|
| field | `AppState::model_store` | Added: `pub model_store: Arc<ModelStore>` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-server/src/state.rs | Add `model_store: Arc<ModelStore>` field to AppState |
| CREATE | crates/anvilml-server/src/handlers/models.rs | New handler module with list_models() and get_model() |
| Modify | crates/anvilml-server/src/handlers/mod.rs | Add `pub mod models;` |
| Modify | crates/anvilml-server/src/lib.rs | Register GET /v1/models and GET /v1/models/:id routes in build_router() |
| CREATE | crates/anvilml-server/tests/models_tests.rs | Integration tests for the two handlers |
| Modify | crates/anvilml-server/Cargo.toml | Bump patch version 0.1.19 → 0.1.20 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| models_tests.rs | test_list_models_no_filter | GET /v1/models returns all models when no kind filter is provided | AppState with model_store containing 2 models of different kinds | No query params | 200 OK, JSON array of length 2 | `cargo test -p anvilml-server --test models_tests test_list_models_no_filter` |
| models_tests.rs | test_list_models_kind_filter | GET /v1/models?kind=diffusion returns only diffusion models | AppState with 3 models (2 diffusion, 1 vae) | `?kind=diffusion` | 200 OK, JSON array of length 2 | `cargo test -p anvilml-server --test models_tests test_list_models_kind_filter` |
| models_tests.rs | test_get_model_existing_returns_200 | GET /v1/models/:id returns the correct model for an existing ID | AppState with 1 model inserted | GET /v1/models/{id} | 200 OK, JSON body with matching id, name, kind | `cargo test -p anvilml-server --test models_tests test_get_model_existing_returns_200` |
| models_tests.rs | test_get_model_unknown_returns_404 | GET /v1/models/:id returns 404 for a non-existent ID | AppState with model_store | GET /v1/models/{random-uuid} | 404 Not Found | `cargo test -p anvilml-server --test models_tests test_get_model_unknown_returns_404` |

## CI Impact

No new CI jobs. The existing `rust-linux` and `rust-windows` CI jobs run `cargo test --workspace --features mock-hardware`, which will pick up the new `models_tests.rs` test crate automatically. The `openapi-drift` CI job (P18-F1) will need to be updated with the new route annotations in a future task, but this task does not modify the OpenAPI pipeline.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. All code uses platform-neutral types (`String` for model IDs, `PathBuf` from ModelStore which is already in the codebase). No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ModelStore::list()` and `ModelStore::get()` signatures may not match what the task context expects — the existing implementation uses `Option<ModelKind>` for the kind filter, but the stored value is plain text (not the enum). The `ModelStore::list()` method already handles this internally by serialising the enum to snake_case text before the SQL query. | Low | Medium | Read `anvilml-registry/src/store.rs` before writing (done in inspection). The `list()` method signature is `async fn list(&self, kind: Option<ModelKind>) -> Result<Vec<ModelMeta>, AnvilError>` — confirmed against source. |
| `ModelStore::get()` returns `Result<Option<ModelMeta>, AnvilError>`, but the handler needs to convert `None` to `AnvilError::ModelNotFound`. The conversion is straightforward (`ok_or_else`) but must be done correctly to avoid a silent 200 with empty body. | Low | Medium | Follow the exact pattern used in `jobs.rs::get_job()` which does the same conversion (`job.ok_or_else(|| AnvilError::JobNotFound(...)).map(Json)`). |
| Test pool must have the `models` table available. The existing migration `001_initial.sql` (already applied in tests) creates the `models` table, so `make_test_pool()` already supports it. | Low | Low | Reuse the same migration path (`"../../database/migrations"`) as existing test files. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test models_tests` exits 0 with >=4 tests
- [ ] `cargo check -p anvilml-server --features mock-hardware` exits 0
- [ ] `grep "^## " .forge/reports/P18-C1_plan.md` shows all 12 required section headings
- [ ] `head -1 .forge/reports/P18-C1_plan.md` prints `# Plan Report: P18-C1`
- [ ] `wc -l .forge/reports/P18-C1_plan.md` prints a value > 40
