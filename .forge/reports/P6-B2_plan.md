# Plan Report: P6-B2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P6-B2                                             |
| Phase       | 006 — Model Registry                              |
| Description | anvilml-server: POST /v1/models/rescan + startup scan |
| Depends on  | P6-A1, P6-A2                                      |
| Project     | anvilml                                           |
| Planned at  | 2026-06-15T22:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add a `rescan_models` handler (`POST /v1/models/rescan`) that responds with HTTP 202 immediately and spawns a `tokio::spawn` background task to scan all configured model directories (`cfg.model_dirs`) using the existing `ModelScanner`, then upsert each discovered model into the `ModelStore`. Also run an initial scan at server startup in `main.rs` (same scan-and-upsert logic) so models are available before any HTTP request. Each scan logs `tracing::info!(count, dir)` at completion.

After this task completes, a developer can place a `.safetensors` file in `models/diffusion/`, send `curl -X POST http://127.0.0.1:8488/v1/models/rescan` (or rely on the startup scan), and then `curl http://127.0.0.1:8488/v1/models` returns the new model with `kind: "diffusion"` and `dtype: "fp8"` if the filename contains `fp8`.

## Scope

### In Scope
- `crates/anvilml-server/src/handlers/models.rs` — add `rescan_models` handler function
- `crates/anvilml-server/src/lib.rs` — mount `POST /v1/models/rescan` route in `build_router`
- `crates/anvilml-server/src/state.rs` — add `model_dirs: Vec<ModelDirConfig>` field to `AppState`; update both constructors
- `crates/anvilml-registry/src/store.rs` — add `scan_and_upsert(&self, dirs: &[ModelDirConfig])` method that calls `ModelScanner::scan()` then upserts each result
- `backend/src/main.rs` — run initial scan after `AppState` construction, before server bind; log result count
- `crates/anvilml-server/Cargo.toml` — bump patch version `0.1.8 → 0.1.9`
- `crates/anvilml-registry/Cargo.toml` — bump patch version `0.1.8 → 0.1.9`
- `crates/anvilml-server/tests/models_tests.rs` — add integration tests for rescan endpoint

### Out of Scope
- WebSocket broadcast of scan completion events (future task)
- Scan cancellation, timeout, or progress reporting (future task)
- Scan scheduling / periodic rescans (future task)
- Model deletion on scan (scanner only adds/updates, never deletes — future task may add cleanup)

## Existing Codebase Assessment

The codebase already has a fully implemented `ModelScanner` (`crates/anvilml-registry/src/scanner.rs`) with `pub async fn scan(&self, dirs: &[ModelDirConfig]) -> Vec<ModelMeta>` that walks directories, filters `.safetensors` files, derives `ModelKind` from parent directory name, `ModelDtype` from filename, and `ModelMeta::id` from SHA256 of the first 1 MiB. It logs scan completion at INFO with `count=` and `dir=` fields.

The `ModelStore` (`crates/anvilml-registry/src/store.rs`) has `upsert`, `get`, `list`, and `delete` methods backed by SQLite. It does not yet have a combined scan-and-upsert method — the two operations are always called separately in tests.

The `AppState` struct (`crates/anvilml-server/src/state.rs`) holds `registry: Arc<ModelStore>`, `db: SqlitePool`, `hardware`, `version`, `start_time`, and `env_report`. It does **not** hold the `ServerConfig` or `model_dirs` — the config is loaded and consumed in `main.rs` but not stored in `AppState`.

The handler pattern in `handlers/models.rs` uses `pub(crate) async fn` with `State(state): State<AppState>` extractor. The `build_router` function in `lib.rs` mounts routes using `axum::routing::get` (and will need `post` for the new route).

Established patterns: `#[tracing::instrument]` on async functions, structured tracing fields (`key = %value`), `///` doc comments on all `pub(crate)` items, tests in `crates/*/tests/` using `tower::util::ServiceExt::oneshot`, in-memory databases via `open_in_memory()`.

No discrepancy between the design doc and current source affects this task — the scanner and store APIs are exactly as specified.

## Resolved Dependencies

None. This task introduces no new external crates or packages. It reuses existing dependencies: `tokio` (for `tokio::spawn`), `anvilml-core::ModelDirConfig`, `anvilml_registry::ModelScanner`, and `anvilml_registry::ModelStore`. All are already declared in the workspace manifests.

## Approach

1. **Add `scan_and_upsert` to `ModelStore`** (`crates/anvilml-registry/src/store.rs`):
   - Add `use anvilml_core::ModelDirConfig;` and `use crate::scanner::ModelScanner;`.
   - Implement `pub async fn scan_and_upsert(&self, dirs: &[ModelDirConfig]) -> Result<usize, AnvilError>`:
     - Call `ModelScanner::scan(dirs).await` to get `Vec<ModelMeta>`.
     - Iterate each `ModelMeta` and call `self.upsert(&meta).await?`.
     - Return the count of models scanned.
     - Annotate with `#[tracing::instrument(skip(self, dirs))]`.
     - Log at DEBUG for each file examined (the scanner already does this); the new method adds no additional per-file logging since the scanner owns that concern.
   - Rationale: Placing scan-and-upsert in `ModelStore` keeps the handler thin (just a background task trigger) and makes the combined operation reusable from `main.rs` for the startup scan.

2. **Add `model_dirs` to `AppState`** (`crates/anvilml-server/src/state.rs`):
   - Add `pub model_dirs: Vec<anvilml_core::ModelDirConfig>` field to `AppState`.
   - Update `new()` constructor: add `model_dirs: Vec::new()` (empty default for tests).
   - Update `new_with_hardware()` constructor: add `model_dirs: Vec<anvilml_core::ModelDirConfig>` parameter.
   - Rationale: The handler needs access to the configured model directories. Passing them through `AppState` follows the established pattern of storing shared state there.

3. **Add `rescan_models` handler** (`crates/anvilml-server/src/handlers/models.rs`):
   - Add imports: `use anvilml_registry::ModelScanner;` and `use tracing::info;`.
   - Implement `pub(crate) async fn rescan_models(State(state): State<AppState>) -> (axum::http::StatusCode, Json<serde_json::Value>)`:
     - Extract `model_dirs` from `state.model_dirs`.
     - Clone `state.registry` for the background task.
     - Spawn a `tokio::spawn` task:
       - Call `state.registry.scan_and_upsert(&model_dirs).await`.
       - On `Ok(count)`: log `tracing::info!(count, dir = %dirs_string, "rescan completed")`.
       - On `Err(e)`: log `tracing::error!(error = %e, "rescan failed")`.
     - Return `(StatusCode::ACCEPTED, Json(serde_json::json!({"status": "scanning"})))`.
     - Rationale: Responding 202 immediately is required by the task spec and ANVILML_DESIGN.md §12.4. The background task ensures the HTTP thread is not blocked during potentially slow directory scans.

4. **Mount the new route** (`crates/anvilml-server/src/lib.rs`):
   - Add `use axum::routing::post;` import.
   - Add `use handlers::models::rescan_models;` import.
   - Add `.route("/v1/models/rescan", post(rescan_models))` to the router chain (after the existing `/v1/models/{id}` route).

5. **Run initial scan at startup** (`backend/src/main.rs`):
   - After `AppState::new_with_hardware(...)` construction and before `build_router(state)`:
     - Call `state.registry.scan_and_upsert(&cfg.model_dirs).await`.
     - Log `tracing::info!(count = %n, dir = %dirs_string, "initial scan completed")` where `dirs_string` joins the configured directory paths.
     - If the scan returns an error, log `tracing::warn!` and continue (a failed initial scan should not prevent the server from starting — models will be picked up on the first manual rescan).
   - Rationale: Running the initial scan at startup means models are available immediately without requiring a manual POST. The scan logic is identical to the rescan handler's background task, so reusing `scan_and_upsert` avoids code duplication.

6. **Update `main.rs` AppState construction**:
   - Pass `cfg.model_dirs.clone()` to `AppState::new_with_hardware()`.

7. **Bump crate versions**:
   - `crates/anvilml-server/Cargo.toml`: `0.1.8 → 0.1.9`
   - `crates/anvilml-registry/Cargo.toml`: `0.1.7 → 0.1.8`

## Public API Surface

| Item | Type | Crate/Module | Signature |
|------|------|-------------|-----------|
| `scan_and_upsert` | `pub async fn` | `anvilml-registry/src/store.rs` | `pub async fn scan_and_upsert(&self, dirs: &[ModelDirConfig]) -> Result<usize, AnvilError>` |
| `rescan_models` | `pub(crate) async fn` | `anvilml-server/src/handlers/models.rs` | `pub(crate) async fn rescan_models(State(state): State<AppState>) -> (StatusCode, Json<Value>)` |
| `AppState::model_dirs` | `pub field` | `anvilml-server/src/state.rs` | `pub model_dirs: Vec<anvilml_core::ModelDirConfig>` |

No new `pub` items in `lib.rs` files. No new trait implementations.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-registry/src/store.rs` | Add `scan_and_upsert` method |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.7 → 0.1.8 |
| MODIFY | `crates/anvilml-server/src/state.rs` | Add `model_dirs` field to `AppState`; update constructors |
| MODIFY | `crates/anvilml-server/src/handlers/models.rs` | Add `rescan_models` handler |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Mount `POST /v1/models/rescan` route |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.8 → 0.1.9 |
| MODIFY | `backend/src/main.rs` | Run initial scan at startup before server bind |
| MODIFY | `crates/anvilml-server/tests/models_tests.rs` | Add integration tests for rescan endpoint |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/models_tests.rs` | `test_rescan_returns_202` | POST /v1/models/rescan returns HTTP 202 with `{"status": "scanning"}` body | In-memory AppState with empty model_dirs | POST to `/v1/models/rescan` | 202 status, JSON body with `status=scanning` | `cargo test -p anvilml-server --test models_tests -- rescan` exits 0 |
| `crates/anvilml-server/tests/models_tests.rs` | `test_rescan_populates_registry` | After POST /v1/models/rescan with model files on disk, GET /v1/models returns the scanned models | Model files placed in temp model_dirs; AppState configured with those dirs | POST to `/v1/models/rescan`, then GET `/v1/models` | Models appear in list with correct kind/dtype | `cargo test -p anvilml-server --test models_tests -- rescan_populates` exits 0 |
| `crates/anvilml-server/tests/models_tests.rs` | `test_rescan_infer_kind_and_dtype` | Scanned models have correct `kind` (from directory name) and `dtype` (from filename) | Temp dirs named `diffusion` and `vae` containing files with `fp8` and no dtype marker in filenames | POST to `/v1/models/rescan`, then GET `/v1/models` | `kind=diffusion, dtype=fp8` for diffusion file; `kind=vae, dtype=unknown` for vae file | `cargo test -p anvilml-server --test models_tests -- infer_kind` exits 0 |

## CI Impact

No CI changes required. The new tests are in existing test files (`crates/anvilml-server/tests/models_tests.rs`) which are picked up by the existing `cargo test --workspace --features mock-hardware` CI job. The route addition does not affect the OpenAPI generator (it operates on `#[utoipa::path]` annotations, and this task does not add any). No new crate dependencies means no changes to the CI dependency resolution.

## Platform Considerations

None identified. The implementation uses only cross-platform Rust std library, tokio, and axum primitives. Filesystem operations go through `tokio::fs` which abstracts platform differences. The `ModelScanner` already handles cross-platform paths via `PathBuf`. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ModelScanner::scan()` logs `count` and `dir` at INFO, but the rescan handler would also log at INFO after the background task completes — resulting in duplicate INFO log lines per scan | Low | Medium | The scanner's INFO log is the mandatory log point per ENVIRONMENT.md §9 (Model scan → Scan completed). The handler's background task should NOT log another INFO for the same scan — it only logs errors. The startup scan in `main.rs` adds one INFO at INFO level. This matches the requirement of `tracing::info!(count,dir)` per scan exactly once. |
| The `scan_and_upsert` method calls `self.upsert(&meta).await?` for each model — a single DB error aborts the entire scan, potentially leaving partially-uploaded results | Medium | High | The scanner already handles per-file errors gracefully (skips unreadable files). The upsert loop should catch per-model errors individually (log at WARN, continue) rather than propagating a single failure. This ensures a bad model record doesn't prevent all other models from being scanned. |
| Background task in `rescan_models` drops its `JoinHandle` — if the task panics, there is no way to observe the failure | Low | Low | Tokio panics in a spawned task propagate to the task's `JoinHandle`. The handler discards the handle intentionally (fire-and-forget for 202 response). The `tracing::error!` log on failure ensures the operator sees the issue. This is the correct pattern for fire-and-forget tasks per ANVILML_DESIGN.md §4.7. |
| `AppState` now holds `Vec<ModelDirConfig>` — the `Clone` derive on `AppState` means each handler gets a clone of the vec, but `ModelDirConfig` contains `PathBuf` which is cheap to clone | Low | Low | `ModelDirConfig` contains `PathBuf` (cheap clone) and `bool`/`Option<u32>` (copy). The vec clone is O(n) where n = number of configured dirs (typically < 10). No performance concern. |

## Acceptance Criteria

- [ ] `cargo build --workspace --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-server --test models_tests -- rescan` exits 0 (all three rescan tests pass)
- [ ] `cargo test -p anvilml-registry -- store` exits 0 (existing store tests still pass)
- [ ] `curl -s -X POST http://127.0.0.1:8488/v1/models/rescan` returns HTTP 202 with `{"status":"scanning"}` body (verified by integration test, not manual curl in CI)
- [ ] After placing a `.safetensors` file in a model directory and triggering a rescan, `curl -s http://127.0.0.1:8488/v1/models` returns the model with `kind: "diffusion"` and `dtype: "fp8"` (verified by integration test)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
