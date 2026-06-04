# Plan Report: P6-A6

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P6-A6                                               |
| Phase       | 006 — Model Registry                                |
| Description | anvilml-server: GET /v1/models handler (list with kind filter) |
| Depends on  | P6-A5                                                 |
| Project     | anvilml                                               |
| Planned at  | 2026-06-04T07:28:00Z                                  |
| Attempt     | 1                                                     |

## Objective

Create `crates/anvilml-server/src/handlers/models.rs` containing an async handler `list_models` that accepts a `State<Arc<AppState>>`, an optional `Query{kind: Option<ModelKind>}` query parameter, and returns `Json<Vec<ModelMeta>>` by delegating to `registry.list(kind)`. Wire the route as `GET /v1/models` in the axum router. Add an integration test that verifies the handler returns the models scanned from a configured model directory.

## Scope

### In Scope
- Create `crates/anvilml-server/src/handlers/models.rs` with `list_models` handler function.
- Add `pub mod models;` to `crates/anvilml-server/src/handlers/mod.rs`.
- Wire `.route("/v1/models", get(handlers::models::list_models))` in `build_router()` in `lib.rs`.
- Add an integration test in `crates/anvilml-server/tests/api_models.rs` that:
  - Creates a temporary model directory.
  - Drops a fake model file (e.g., `model-fp16.safetensors`).
  - Builds `AppState` with a fresh `ModelRegistry`.
  - Calls `registry.rescan()` to pick up the file.
  - Sends a GET request to `/v1/models` and asserts the response contains the expected model metadata (correct name, kind=Diffusion, dtype_hint=F16).
  - Sends a GET request with `?kind=diffusion` and verifies filtering works.

### Out of Scope
- `GET /v1/models/:id` handler (deferred to P6-A7).
- `POST /v1/models/rescan` handler (deferred to P6-A7).
- Pagination or limit parameters on the list endpoint.
- OpenAPI drift gate update (no new `utoipa` annotations needed — `ModelMeta` is already annotated with `#[derive(ToSchema)]`).
- Changes to `anvilml.toml` (model_dirs entry for `./models/diffusion` already exists from P6-A5).

## Approach

1. **Read existing handler patterns.** Study `handlers/health.rs` and `handlers/system.rs` to confirm the handler signature convention: `(StatusCode, Json<T>)` return type, `State<Arc<AppState>>` extractor.

2. **Create `handlers/models.rs`.** Implement `list_models`:
   ```rust
   use axum::{extract::Query, http::StatusCode, response::Json, extract::State};
   use serde::Deserialize;
   use std::sync::Arc;
   use anvilml_core::ModelKind;

   #[derive(Deserialize)]
   pub struct ModelsListQuery {
       pub kind: Option<ModelKind>,
   }

   pub async fn list_models(
       State(state): State<Arc<crate::state::AppState>>,
       Query(query): Query<ModelsListQuery>,
   ) -> (StatusCode, Json<Vec<anvilml_core::ModelMeta>>) {
       match state.registry.list(query.kind).await {
           Ok(models) => (StatusCode::OK, Json(models)),
           Err(e) => (
               StatusCode::INTERNAL_SERVER_ERROR,
               Json(vec![]), // fallback; in practice errors should surface as 500 with error body
           ),
       }
   }
   ```

3. **Register the module.** Add `pub mod models;` to `handlers/mod.rs`.

4. **Wire the route.** In `lib.rs::build_router()`, add:
   ```rust
   .route("/v1/models", get(handlers::models::list_models))
   ```

5. **Write integration test.** Create `crates/anvilml-server/tests/api_models.rs`:
   - Use `tempfile::tempdir()` or `std::env::temp_dir()` to create a temporary model directory.
   - Write a zero-byte file named `model-fp16.safetensors` (the scanner will scan it; dtype inferred from filename suffix `fp16`).
   - Create a fresh `ModelRegistry` with an in-memory SQLite pool.
   - Call `registry.rescan()` with a `ModelDirConfig` pointing to the temp dir and kind=Diffusion.
   - Build `AppState` with this registry.
   - Build router, send `GET /v1/models`, assert 200 and body contains one model with name=`model-fp16`, kind=`Diffusion`, dtype_hint=`F16`.
   - Send `GET /v1/models?kind=diffusion`, assert same result.
   - Send `GET /v1/models?kind=vae`, assert empty array.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-server/src/handlers/models.rs` | New handler module with `list_models` function |
| Modify | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod models;` |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire `.route("/v1/models", ...)` in `build_router()` |
| Create | `crates/anvilml-server/tests/api_models.rs` | Integration test for GET /v1/models endpoint |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-server/tests/api_models.rs` | `list_models_returns_scanned_models` | GET /v1/models returns 200 with scanned model metadata (name, kind, dtype_hint) |
| `crates/anvilml-server/tests/api_models.rs` | `list_models_kind_filter_diffusion` | GET /v1/models?kind=diffusion returns only diffusion models |
| `crates/anvilml-server/tests/api_models.rs` | `list_models_kind_filter_no_match` | GET /v1/models?kind=vae returns empty array when no vae models exist |

## CI Impact

No CI workflow files are modified. The new integration test is a standard Rust test (`tests/` directory) and will run automatically under `cargo test --workspace --features mock-hardware`. No changes to `.github/workflows/` are needed.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `ModelKind` deserialization from query string may fail if the query param value doesn't match a variant (e.g. case mismatch). Axum's `Query` extractor will return 400 Bad Request by default, which is correct behavior for an invalid kind filter. | No extra handling needed; axum handles this automatically. Document that values must be lowercase enum variants matching the JSON representation of `ModelKind`. |
| The scanner may not infer `dtype_hint=F16` from a zero-byte file (no actual weights to inspect). | The scanner in P6-A1 infers dtype from filename suffix (`fp16` → F16) even without reading file contents, so this should work. If not, the test will fail and we adjust the fixture name. |
| In-memory SQLite pool in tests may conflict with concurrent test execution. | Each test creates its own `ModelRegistry` and `AppState` independently; sqlx's in-memory pool is per-connection and tests run sequentially within a single binary by default. No conflict expected. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --features mock-hardware` passes (unit + integration tests exit 0)
- [ ] `GET /v1/models` returns HTTP 200 with a JSON array of `ModelMeta` objects
- [ ] Scanned models appear with correct `kind`, `name`, and `dtype_hint` fields in the response body
- [ ] `GET /v1/models?kind=diffusion` filters to only diffusion models
- [ ] `GET /v1/models?kind=vae` returns an empty array when no vae models exist
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` passes with zero warnings
- [ ] `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware` passes (platform cross-check)
