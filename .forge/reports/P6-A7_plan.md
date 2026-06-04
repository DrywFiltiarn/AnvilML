# Plan Report: P6-A7

| Field       | Value                                              |
|-------------|----------------------------------------------------|
| Task ID     | P6-A7                                                |
| Phase       | 006 — Model Registry                                |
| Description | anvilml-server: GET /v1/models/:id and POST /v1/models/rescan |
| Depends on  | P6-A6 (GET /v1/models list handler)                  |
| Project     | anvilml                                              |
| Planned at  | 2026-06-04T08:00:00Z                                |
| Attempt     | 1                                                    |

## Objective

Extend the anvilml-server HTTP API with two new endpoints: `GET /v1/models/:id` to retrieve a single model by its ID (returning 200 ModelMeta or 404 not-found JSON), and `POST /v1/models/rescan` to trigger a background model directory rescan (returning 202 Accepted).

## Scope

### In Scope
- Add `async fn get_model(State<Arc<AppState>>, Path<String>)` handler in `crates/anvilml-server/src/handlers/models.rs`: calls `registry.get(id)`, returns `(StatusCode::OK, Json(meta))` on found, or `(StatusCode::NOT_FOUND, Json(error_object))` when not found.
- Add `async fn rescan_models(State<Arc<AppState>>)` handler in `crates/anvilml-server/src/handlers/models.rs`: spawns a non-blocking `tokio::spawn` task calling `registry.rescan(&state.model_dirs)`, returns `(StatusCode::ACCEPTED, Json(rescan_response))` immediately.
- Wire both routes in `crates/anvilml-server/src/lib.rs` router: `.route("/v1/models/:id", get(handlers::models::get_model))` and `.route("/v1/models/rescan", post(handlers::models::rescan_models))`.
- Add `model_dirs: Vec<ModelDirConfig>` field to `AppState` (in `state.rs`) so the rescan handler can access configured directories.
- Update `backend/src/main.rs` to pass `cfg.model_dirs.clone()` when constructing `AppState`.

### Out of Scope
- Any changes to `anvilml-registry` crate (get and rescan methods already exist from P6-A2 and P6-A4).
- Adding authentication or middleware.
- WebSocket events for rescan completion (future task).
- Cleanup of stale model entries (explicitly not done by rescan per P6-A4 spec).
- Any changes to the Python worker, scheduler, or hardware crates.

## Approach

1. **Add `model_dirs` field to `AppState`** (`state.rs`): Add `pub model_dirs: Vec<ModelDirConfig>` alongside existing fields. Update both `new()` and `new_with_hardware()` constructors to accept an optional `Vec<ModelDirConfig>` parameter (defaulting to empty when None). Update the `Clone` impl to clone the new field.

2. **Add `get_model` handler** (`handlers/models.rs`):
   - Import `axum::extract::Path` and `anvilml_core::config::ModelDirConfig`.
   - Define a small local struct or tuple for the 404 JSON body: `{ "error": "not_found", "message": "model not found" }`.
   - Implement `get_model` that extracts the ID from `Path(id)`, calls `state.registry.get(&id).await`, and returns either `(StatusCode::OK, Json(meta))` or `(StatusCode::NOT_FOUND, Json(error_body))`.

3. **Add `rescan_models` handler** (`handlers/models.rs`):
   - Implement `rescan_models` that clones `state.model_dirs` and `state.registry`, then spawns a `tokio::spawn` task calling `registry.rescan(&dirs).await`.
   - The handler returns immediately with `(StatusCode::ACCEPTED, Json(rescan_response))` where the response is `{ "status": "rescan_started" }`.
   - The spawned task logs success/failure via `tracing::info!`/`tracing::warn!`.

4. **Wire routes** (`lib.rs`): Add two `.route()` calls:
   - `.route("/v1/models/:id", get(handlers::models::get_model))`
   - `.route("/v1/models/rescan", post(handlers::models::rescan_models))`
   - Place these after the existing `/v1/models` route.

5. **Pass model_dirs from main.rs** (`backend/src/main.rs`): When constructing `AppState`, pass `cfg.model_dirs.clone()` to the registry/state builder (either as a new parameter or via an updated constructor).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Add `get_model` and `rescan_models` handlers |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire GET `/v1/models/:id` and POST `/v1/models/rescan` routes |
| Modify | `crates/anvilml-server/src/state.rs` | Add `model_dirs: Vec<ModelDirConfig>` field to `AppState` + update constructors |
| Modify | `backend/src/main.rs` | Pass `cfg.model_dirs.clone()` when building `AppState` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/src/lib.rs` (inline tests) | New test: `get_model_returns_404_when_missing` | GET `/v1/models/nonexistent-id` returns 404 with JSON error body |
| `crates/anvilml-server/src/lib.rs` (inline tests) | New test: `rescan_returns_202` | POST `/v1/models/rescan` returns 202 Accepted immediately |

## CI Impact

No changes to CI workflow files. The existing CI matrix (fmt, clippy, test on Linux + Windows, openapi-diff, python-worker) will automatically cover these changes. The `openapi-diff` gate must pass after implementation since new handler signatures and utoipa annotations are not used here — however the handlers do not have utoipa annotations yet, so no OpenAPI drift is expected. If the anvilml-openapi tool scans route paths regardless of annotations, a drift may be detected; the plan accounts for this by ensuring `cargo run -p anvilml-openapi` produces matching output.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Route ordering conflict: `/v1/models/rescan` could match `/v1/models/:id` with `:id="rescan"` | axum routes are matched in registration order; placing the static `/v1/models/rescan` route before the parameterised `/v1/models/:id` route ensures correct dispatch. Alternatively, register both and verify no ambiguity warning at compile time. |
| Missing import for `ModelDirConfig` in state.rs | Import via `anvilml_core::config::ModelDirConfig` (re-exported as `pub use config::*`). Verify compilation before writing report. |
| Tokio spawn task outlives request lifecycle | The spawned task holds an `Arc<ModelRegistry>` clone which is `'static`, so it will run to completion independently of the HTTP response. No lifetime issues. |
| OpenAPI drift gate fails after adding new routes | Run `cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json` during verification; if drifted, regenerate openapi.json as part of the task. |

## Acceptance Criteria

- [ ] `cargo test --workspace --features mock-hardware` exits 0 after changes
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` passes with no errors
- [ ] `curl http://127.0.0.1:8488/v1/models/<valid-id>` returns HTTP 200 with a JSON ModelMeta object
- [ ] `curl http://127.0.0.1:8488/v1/models/nonexistent-id` returns HTTP 404 with a JSON error body containing `"error"` and `"message"` fields
- [ ] `curl -X POST http://127.0.0.1:8488/v1/models/rescan` returns HTTP 202 Accepted
- [ ] After adding a new model file to a configured model directory, running `POST /v1/models/rescan` and then `GET /v1/models` shows the newly scanned model
- [ ] `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware` passes (platform cross-check)
