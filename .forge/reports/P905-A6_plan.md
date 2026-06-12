# Plan Report: P905-A6

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P905-A6                                           |
| Phase       | 905 — FP8 dtype & model metadata override         |
| Description | anvilml-server: PATCH /v1/models/:id metadata override endpoint |
| Depends on  | P905-A5                                           |
| Project     | anvilml                                           |
| Planned at  | 2026-06-12T13:00:00Z                             |
| Attempt     | 1                                                 |

## Objective

Add a `PATCH /v1/models/:id` endpoint to the AnvilML server that allows clients to partially
override model metadata (`dtype_hint` and/or `kind`). The endpoint delegates to the
already-implemented `ModelRegistry::patch_meta` method (from P905-A5), returns 404 when the
model does not exist, and returns 200 with the updated `ModelMeta` on success.

## Scope

### In Scope
- Add `patch_model` async handler function in `crates/anvilml-server/src/handlers/models.rs`
- Add `#[utoipa::path]` annotation for the PATCH endpoint (OpenAPI generation)
- Wire the route in `crates/anvilml-server/src/lib.rs` (`PATCH` on `/v1/models/{id}`)
- Add three unit tests in `crates/anvilml-server/src/lib.rs`:
  - `patch_model_updates_dtype_hint` — sends `{"dtype_hint":"f8_e4m3"}`, verifies 200 with updated dtype and recomputed vram_estimate_mib
  - `patch_model_returns_404` — sends PATCH for non-existent model ID, verifies 404
  - `patch_model_partial_preserves_other_fields` — sends partial patch (e.g. kind only), verifies dtype_hint and other fields are unchanged
- Bump `anvilml-server` patch version from `0.1.18` to `0.1.19`

### Out of Scope
- No changes to `anvilml-core` or `anvilml-registry` (types already exist from P905-A5)
- No changes to OpenAPI generation (handled by Gate 2 trigger)
- No changes to WebSocket event broadcasting for metadata updates
- No changes to model rescan logic

## Approach

1. **Add handler** in `crates/anvilml-server/src/handlers/models.rs`:
   - Import `ModelMetaPatch` from `anvilml_core` (already re-exported via `anvilml-core::types::model`)
   - Import `axum::extract::Json` (already imported)
   - Implement `pub async fn patch_model(State(state): State<Arc<App>>, Path(id): Path<String>, body: Json<ModelMetaPatch>)` following the existing pattern:
     - Call `state.registry.patch_meta(&id, body.into_inner()).await`
     - On `Ok(None)` → 404 with `{"error":"not_found","message":"model not found"}`
     - On `Ok(Some(meta))` → 200 with `Json(serde_json::to_value(&meta).unwrap())`
     - On `Err(e)` → 500 with error JSON, log at ERROR level
   - Add `#[utoipa::path]` annotation:
     - `patch`, `path = "/v1/models/{id}"`, summary = "Patch model metadata"
     - Params: `("id" = String, Path, description = "Model ID")`
     - Request body: `body = ModelMetaPatch` (via utoipa `ToSchema`)
     - Responses: 200 with `ModelMeta`, 404, 422, 500

2. **Wire route** in `crates/anvilml-server/src/lib.rs`:
   - Import `patch` from `axum::routing` (add to existing `use axum::routing::{get, post}` → `use axum::routing::{get, patch, post}`)
   - Add `.route("/v1/models/{id}", patch(handlers::models::patch_model).get(handlers::models::get_model))`
   - Note: axum allows chaining multiple methods on the same route path; the existing `.route("/v1/models/{id}", get(handlers::models::get_model))` line will be extended to include `patch`

3. **Add tests** in `crates/anvilml-server/src/lib.rs` (within existing `#[cfg(test)] mod tests`):
   - `patch_model_updates_dtype_hint`: Use `make_test_app` or the `get_model_returns_404_when_missing` pattern with a real registry. Insert a model via `registry.upsert()`, send PATCH with `{"dtype_hint":"f8_e4m3"}`, verify 200 and that `dtype_hint` changed and `vram_estimate_mib` recomputed.
   - `patch_model_returns_404`: Use empty registry, send PATCH to non-existent ID, verify 404.
   - `patch_model_partial_preserves_other_fields`: Insert a model with known `dtype_hint`, send PATCH with only `{"kind":"vae"}`, verify `dtype_hint` is unchanged and `kind` is updated.

4. **Bump version** in `crates/anvilml-server/Cargo.toml`:
   - Change `version = "0.1.18"` → `version = "0.1.19"`

5. **Verify**: Run `cargo test -p anvilml-server --features mock-hardware` and confirm exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Add `patch_model` handler with `#[utoipa::path]` annotation |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire PATCH route; add 3 unit tests |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version `0.1.18 → 0.1.19` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/src/lib.rs` | `patch_model_updates_dtype_hint` | PATCH with `dtype_hint` changes dtype and recomputes vram_estimate_mib, returns 200 |
| `crates/anvilml-server/src/lib.rs` | `patch_model_returns_404` | PATCH for non-existent model ID returns 404 with error JSON |
| `crates/anvilml-server/src/lib.rs` | `patch_model_partial_preserves_other_fields` | Partial patch (kind only) leaves dtype_hint and other fields unchanged |

## CI Impact

This task modifies only the `anvilml-server` crate. The `#[utoipa::path]` annotation addition
triggers the OpenAPI drift gate (Gate 2), requiring `cargo run -p anvilml-openapi && git diff
--exit-code backend/openapi.json` to be run after implementation. No CI workflow files are
modified. The `mock-hardware` feature is already forwarded in `anvilml-server/Cargo.toml`.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ModelMetaPatch` import path mismatch | Low | Build failure | `anvilml_core::ModelMetaPatch` is re-exported in `anvilml-core/src/lib.rs`; verify import compiles |
| Axum route conflict — same path with different methods | Low | Build failure | axum supports `.route(path, get(...).patch(...))` chaining; pattern matches existing `/v1/jobs/{id}` route |
| utoipa `ToSchema` not derived on `ModelMetaPatch` | None | N/A | Already derived in P905-A5 (`#[derive(Debug, Clone, Deserialize, ToSchema)]`) |
| Test DB isolation — tests share in-memory pool | Low | Test flakiness | Each test creates its own `TempDir` + fresh SQLite file; no shared state |
| OpenAPI drift after handler addition | Certain | Stale openapi.json | Gate 2 run post-implementation; regenerate `backend/openapi.json` |

## Acceptance Criteria

- [ ] `patch_model` handler exists in `crates/anvilml-server/src/handlers/models.rs` with correct `#[utoipa::path]` annotation
- [ ] PATCH route wired in `crates/anvilml-server/src/lib.rs` alongside existing GET route on `/v1/models/{id}`
- [ ] Three unit tests present: `patch_model_updates_dtype_hint`, `patch_model_returns_404`, `patch_model_partial_preserves_other_fields`
- [ ] `anvilml-server` patch version bumped to `0.1.19` in `Cargo.toml`
- [ ] `cargo test -p anvilml-server --features mock-hardware` exits 0
- [ ] `cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json` passes (OpenAPI drift gate)
