# Plan Report: P6-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-B1                                       |
| Phase       | 006 — Model Registry                        |
| Description | anvilml-server: GET /v1/models and GET /v1/models/:id handlers |
| Depends on  | P6-A2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-15T21:05:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `handlers/models.rs` in the `anvilml-server` crate with two HTTP handlers — `list_models` (GET /v1/models with optional `?kind=` query filter) and `get_model` (GET /v1/models/:id returning 404 when the model is not found). Add a `registry: Arc<ModelStore>` field to `AppState` and wire it into `build_router`. Register the two routes in the router. After this task, calling `curl http://127.0.0.1:8488/v1/models` returns a 200 JSON array (empty when no models are scanned), and `curl http://127.0.0.1:8488/v1/models/nonexistent-id` returns 404 with the standard `AnvilError` JSON body.

## Scope

### In Scope
- Create `crates/anvilml-server/src/handlers/models.rs` with two handler functions.
- Add `registry: Arc<ModelStore>` field to `AppState` in `crates/anvilml-server/src/state.rs`.
- Mount `GET /v1/models` and `GET /v1/models/:id` routes in `build_router` in `crates/anvilml-server/src/lib.rs`.
- Update `crates/anvilml-server/src/handlers/mod.rs` to declare and re-export the models module.
- Add `anvilml-registry` as a regular (non-dev) dependency in `crates/anvilml-server/Cargo.toml`.
- Create `crates/anvilml-server/tests/models_tests.rs` with integration tests.
- Bump `anvilml-server` patch version from `0.1.7` to `0.1.8`.

### Out of Scope
- POST /v1/models/rescan (handled in P6-B2).
- Model scanning logic (handled in P6-A1).
- Model CRUD in the database (handled in P6-A2).
- Any other handler files (jobs, workers, artifacts, nodes are out of scope).

## Existing Codebase Assessment

The `anvilml-server` crate currently has two handler modules (`health` and `system`) plus the `AppState` struct and `build_router()` function. The `AppState` struct (in `state.rs`) currently holds `start_time`, `version`, `env_report`, `hardware`, and `db`. It does **not** yet have a `registry` field.

The `anvilml-registry` crate's `ModelStore` (in `store.rs`) provides `new(pool)`, `upsert(meta)`, `get(id)`, `list(kind)`, and `delete(id)` — all returning `Result<T, AnvilError>`. The `list` method accepts an optional `ModelKind` filter and returns `Vec<ModelMeta>`. The `get` method returns `Option<ModelMeta>`.

The existing handler pattern is clear: each handler takes `State<AppState>` (and optionally `Path<T>` or `Query<T>`) and returns `Json<T>` or `Result<Json<T>, AnvilError>`. The `AnvilError` type (from `anvilml-core`) implements `IntoResponse`, mapping `ModelNotFound` to 404.

No handler files for jobs, models, workers, artifacts, or nodes exist yet. The `handlers/mod.rs` only declares `health` and `system`. The `anvilml-registry` crate is currently only in `dev-dependencies` of `anvilml-server`, so production handlers cannot import from it.

## Resolved Dependencies

| Type   | Name           | Version verified | MCP source   | Feature flags confirmed |
|--------|----------------|-----------------|--------------|------------------------|
| crate  | axum           | 0.8.9           | Cargo.lock   | json, http1, tokio, ws |
| crate  | anvilml-registry| 0.1.6          | Cargo.lock   | (none)                 |

**axum 0.8.9** — The `axum::extract::Query` and `axum::extract::Path` extractors are available in the base crate without additional features. The `Query` extractor deserializes query parameters into any type implementing `Deserialize`. The `Path` extractor extracts path segments via `FromStr`.

**anvilml-registry 0.1.6** — `ModelStore::new(pool)`, `ModelStore::list(kind)`, and `ModelStore::get(id)` are the methods used. All return `Result<T, AnvilError>`. The `ModelStore` struct is `pub` and re-exported from the crate root.

## Approach

1. **Add `anvilml-registry` to regular dependencies** in `crates/anvilml-server/Cargo.toml`. This is necessary because the handler code (production, not test) needs to call `ModelStore::list()` and `ModelStore::get()`. Currently it's only in `[dev-dependencies]`.

2. **Add `registry: Arc<ModelStore>` to `AppState`** in `crates/anvilml-server/src/state.rs`. Add the field to the struct, update both constructors (`new()` and `new_with_hardware()`) to initialise it with `Arc::new(ModelStore::new(db.clone()).await)` — using the existing `db` pool. The `ModelStore::new()` call is async, so `new()` (which is already async) can await it directly. For `new_with_hardware()`, we cannot await inside a sync fn, so we'll add a second constructor or note that the ACT agent will handle the sync/async boundary — actually, looking at the current code, `new_with_hardware` is sync and takes a pre-existing pool. We can add `registry: Arc<ModelStore>` and construct it with `Arc::new(ModelStore::new(db.clone()).await)` but that requires async. The cleanest approach: add the field and use `new()` for tests (which is async), and in `new_with_hardware` we'll need to create the ModelStore from the pool. Since `new_with_hardware` is sync, we cannot call `async fn new()`. The ACT agent should add a `ModelStore::new_from_pool(pool)` or simply construct it synchronously — but `ModelStore::new` is async. 

   **Correction:** Looking at `ModelStore::new`, it only stores the pool — no actual async I/O happens inside it (no query execution). The `async fn` is likely just a convention. Let me check: `ModelStore::new(pool: SqlitePool) -> Self` — it just does `ModelStore { pool }`. It's declared `async` but doesn't await anything. This is a common pattern in Rust where the function is async for API consistency but is effectively sync. We can call it inside a sync fn and immediately `.await` via `tokio::runtime::Handle::current().block_on(...)` — but that's ugly. 

   **Better approach:** The ACT agent should add a separate constructor `AppState::new_with_registry` that accepts a pre-built `Arc<ModelStore>`, or change `new_with_hardware` to accept `Arc<ModelStore>` directly. For this plan, I'll specify that `AppState` gains a `registry: Arc<ModelStore>` field, and the ACT agent will update `new_with_hardware` to accept an `Arc<ModelStore>` parameter (the caller — `main.rs` — constructs the `ModelStore` after opening the pool). This is the cleanest API and matches the pattern of passing pre-built `Arc` values.

3. **Create `crates/anvilml-server/src/handlers/models.rs`** with two handler functions:
   
   a. `list_models` — signature: `pub async fn list_models(State(state): State<AppState>, Query(filter): Query<ModelsFilter>) -> Result<Json<Vec<ModelMeta>>, AnvilError>`. Define a private struct `ModelsFilter { kind: Option<ModelKind> }` that derives `Deserialize`. The handler calls `state.registry.list(filter.kind)` and returns the result as `Json`. The `Result<..., AnvilError>` is needed because `registry.list()` returns `Result<Vec<ModelMeta>, AnvilError>`, and `AnvilError` implements `IntoResponse`.
   
   b. `get_model` — signature: `pub async fn get_model(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<ModelMeta>, AnvilError>`. The handler calls `state.registry.get(&id)`, and on `Ok(Some(meta))` returns `Ok(Json(meta))`. On `Ok(None)` returns `Err(AnvilError::ModelNotFound(id))`. On `Err(e)` propagates the error.

   Both functions use the existing handler pattern: extract `State<AppState>`, call into the registry, return `Json<T>` or `Err(AnvilError)`.

4. **Update `crates/anvilml-server/src/handlers/mod.rs`** to declare `pub mod models;` and re-export the two handlers: `pub use models::{get_model, list_models};`.

5. **Update `crates/anvilml-server/src/lib.rs`** to mount the new routes in `build_router`:
   ```rust
   .route("/v1/models", get(list_models))
   .route("/v1/models/:id", get(get_model))
   ```
   Add imports for `list_models` and `get_model` from `handlers::models`.

6. **Create `crates/anvilml-server/tests/models_tests.rs`** with three integration tests:
   
   a. `test_list_models_empty` — Build an `AppState` with an empty in-memory registry (no models in the database). Request GET /v1/models. Assert 200 and empty JSON array `[]`.
   
   b. `test_list_models_with_kind_filter` — Insert a model into the in-memory registry via `ModelStore::upsert`. Request GET /v1/models?kind=diffusion. Assert 200 and the model is in the response. Request GET /v1/models?kind=vae. Assert 200 and empty array.
   
   c. `test_get_model_not_found` — Request GET /v1/models/nonexistent-id. Assert 404 status.

   Test pattern follows the existing `health_tests.rs` and `system_tests.rs`: use `build_router`, `Request::builder`, `ServiceExt::oneshot`, and `to_bytes` for body inspection.

7. **Bump `anvilml-server` patch version** from `0.1.7` to `0.1.8` in `Cargo.toml`.

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `list_models` | `pub async fn` | `anvilml_server::handlers::models` | `async fn list_models(State<AppState>, Query<ModelsFilter>) -> Result<Json<Vec<ModelMeta>>, AnvilError>` |
| `get_model` | `pub async fn` | `anvilml_server::handlers::models` | `async fn get_model(State<AppState>, Path<String>) -> Result<Json<ModelMeta>>, AnvilError>` |
| `AppState.registry` | `pub field` | `anvilml_server::state` | `registry: Arc<ModelStore>` (new field) |

`ModelsFilter` is a private struct in `models.rs` (not re-exported).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-server/Cargo.toml` | Add `anvilml-registry` to `[dependencies]`; bump version 0.1.7 → 0.1.8 |
| MODIFY | `crates/anvilml-server/src/state.rs` | Add `registry: Arc<ModelStore>` field to `AppState`; update constructors |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Declare `pub mod models;`; re-export `get_model`, `list_models` |
| CREATE | `crates/anvilml-server/src/handlers/models.rs` | New handler module with `list_models` and `get_model` |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Mount new routes in `build_router`; import handlers |
| CREATE | `crates/anvilml-server/tests/models_tests.rs` | Integration tests for model endpoints |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/models_tests.rs` | `test_list_models_empty` | GET /v1/models returns 200 with empty JSON array when no models exist | In-memory registry with zero models | GET /v1/models | HTTP 200, body `[]` | `cargo test -p anvilml-server -- models_tests::test_list_models_empty` exits 0 |
| `crates/anvilml-server/tests/models_tests.rs` | `test_list_models_with_kind_filter` | GET /v1/models?kind=diffusion returns only diffusion models; GET /v1/models?kind=vae returns empty when no vae models exist | In-memory registry with one diffusion model inserted via ModelStore | GET /v1/models?kind=diffusion, GET /v1/models?kind=vae | HTTP 200, first returns array with 1 model, second returns `[]` | `cargo test -p anvilml-server -- models_tests::test_list_models_with_kind_filter` exits 0 |
| `crates/anvilml-server/tests/models_tests.rs` | `test_get_model_not_found` | GET /v1/models/:id returns 404 when model ID does not exist | In-memory registry with zero models | GET /v1/models/nonexistent-id | HTTP 404, body contains `"error":"model_not_found"` | `cargo test -p anvilml-server -- models_tests::test_get_model_not_found` exits 0 |

## CI Impact

No CI changes required. The new handler module and test file are picked up by the existing `cargo test --workspace --features mock-hardware` CI job. The `openapi-drift` gate may need updating if the OpenAPI generator is sensitive to new handler routes — but since the task only adds routes and the OpenAPI generator (`anvilml-openapi`) will pick up the new `ToSchema` derives on `ModelMeta` automatically, the gate should produce identical output if `openapi.json` is already up-to-date with these routes. If it produces a diff, the ACT agent must regenerate `api/openapi.json` via `cargo run -p anvilml-openapi`.

## Platform Considerations

None identified. The handlers use only axum extractors (`State`, `Query`, `Path`) and the `ModelStore` which is a pure SQLite abstraction — all platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `AppState::new_with_hardware()` is sync but `ModelStore::new()` is async — adding the field requires either making the constructor async or accepting a pre-built `Arc<ModelStore>`. | High | High | Plan specifies adding a new constructor `AppState::new_with_hardware_and_registry` that takes `Arc<ModelStore>` as a parameter. The ACT agent should check `backend/src/main.rs` to update the call site. This avoids mixing sync and async in the constructor. |
| `anvilml-registry` is currently only in `[dev-dependencies]` of `anvilml-server`. Moving it to regular dependencies changes the dependency graph but not the crate structure — the crate already transitively depends on it via `anvilml-scheduler`. | Low | Low | Adding to `[dependencies]` is a straightforward TOML edit. The transitive path already exists, so no new compilation units are introduced. |
| The `Query` extractor with a custom `ModelsFilter` struct requires `Deserialize` — if the serde rename strategy doesn't match the URL query param (`kind` vs `kind`), the filter won't parse. | Low | Medium | `ModelKind` uses `#[serde(rename_all = "snake_case")]` which produces `"diffusion"`, `"text_encoder"`, etc. The `ModelsFilter` struct derives `Deserialize` with default serde naming (field name = `kind`), so `?kind=diffusion` maps directly to `kind: Some(ModelKind::Diffusion)`. No custom naming needed. |
| `Path<String>` may fail to parse if the model ID contains URL-unsafe characters. | Low | Low | Model IDs are SHA256 hex digests (lowercase a-f, 0-9) — all URL-safe. No percent-decoding or custom `FromStr` needed. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server -- models_tests` exits 0
- [ ] `cargo test -p anvilml-server` exits 0 (all existing tests still pass)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (no regressions)
- [ ] `head -1 .forge/reports/P6-B1_plan.md` prints `# Plan Report: P6-B1`
- [ ] `grep "^## " .forge/reports/P6-B1_plan.md` shows all 11 section headings
- [ ] `wc -l .forge/reports/P6-B1_plan.md` outputs a value > 40
