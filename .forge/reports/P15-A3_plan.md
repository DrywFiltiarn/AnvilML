# Plan Report: P15-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P15-A3                                      |
| Phase       | 015 — Artifact Storage                      |
| Description | anvilml-server: GET /v1/artifacts and GET /v1/artifacts/:hash |
| Depends on  | P15-A1, P15-A2                              |
| Project     | anvilml                                     |
| Planned at  | 2026-06-20T16:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Add two HTTP handlers to the AnvilML server — `GET /v1/artifacts` (list artifact metadata, optionally filtered by job ID) and `GET /v1/artifacts/:hash` (serve raw PNG bytes with `Content-Type: image/png`) — wire them into the axum router in `build_router`, add the `ArtifactNotFound` error variant to `AnvilError`, and write integration tests that verify correct response shapes, content types, and 404 behavior.

## Scope

### In Scope
- Create `crates/anvilml-server/src/handlers/artifacts.rs` with two handlers:
  - `list_artifacts(State<AppState>, Query<ListArtifactsQuery>) -> Result<Json<Vec<ArtifactMeta>>, AnvilError>`
  - `serve_artifact(State<AppState>, Path<String>) -> Result<Response, AnvilError>`
- Add `ArtifactNotFound(String)` variant to `AnvilError` in `anvilml-core/src/error.rs` with 404 mapping and error kind `"artifact_not_found"`
- Add `pub mod artifacts` and re-export `list_artifacts` in `handlers/mod.rs`
- Mount both routes in `build_router()` in `lib.rs`
- Write integration tests in `crates/anvilml-server/tests/artifacts_tests.rs`
- Version bump `anvilml-server` from `0.1.23` to `0.1.24`

### Out of Scope
- Any changes to `ArtifactStore` (already implemented in P15-A1)
- Any changes to the scheduler's `ImageReady` event handler (P15-A2)
- WebSocket `JobImageReady` broadcast (P15-A6 in Phase 016)
- OpenAPI spec regeneration (handled by CI openapi-drift gate)
- Windows-specific behavior beyond `#[cfg(windows)]` compile-check pass

## Existing Codebase Assessment

The codebase already has the `ArtifactStore` in `anvilml-ipc/src/artifact_store.rs` with `save()`, `get()`, and `list()` methods fully implemented and tested. `AppState` already holds `artifact_store: Arc<ArtifactStore>`. The handler module structure follows a consistent pattern: each handler file declares its module in `handlers/mod.rs`, handlers use `State<AppState>` for dependency injection, return `Result<T, AnvilError>`, and carry `#[tracing::instrument]` and `#[utoipa::path]` annotations.

The `AnvilError` enum in `anvilml-core/src/error.rs` currently has 14 variants covering database, I/O, serialization, IPC, worker/job/model not-found, graph validation, and config errors. There is no `ArtifactNotFound` variant yet — this task adds it. The pattern for resource-not-found errors is `NotFound(String)` with 404 status code (as seen with `JobNotFound`, `ModelNotFound`, `WorkerNotFound`).

Integration tests use a real TCP listener with `axum::serve` and raw TCP streams (as seen in `handler_tests.rs`) rather than `Router::oneshot`. The `test_state()` helper in `handler_tests.rs` constructs a full `AppState` with in-memory database, artifact store, scheduler, and node registry — this is the pattern to reuse.

## Resolved Dependencies

| Type   | Name          | Version verified | MCP source | Feature flags confirmed |
|--------|---------------|-----------------|------------|------------------------|
| crate  | axum          | 0.8 (workspace) | Cargo.lock | tower-http, serde, uuid |
| crate  | tower-http    | 0.6 (workspace) | Cargo.lock | trace, cors            |
| crate  | sha2          | 0.10            | Cargo.lock | (none needed)          |

No new external crates are introduced. All types used (`axum::http::Response`, `axum::http::header::CONTENT_TYPE`, `axum::http::StatusCode`, `axum::Json`, `axum::extract::{State, Path, Query}`, `anvilml_core::ArtifactMeta`, `anvilml_ipc::ArtifactStore`) are already present in the workspace.

## Approach

1. **Add `ArtifactNotFound` to `AnvilError`** in `crates/anvilml-core/src/error.rs`:
   - Add `ArtifactNotFound(String)` variant with `#[error("artifact not found: {0}")]`
   - Add mapping in `status_code()`: `ArtifactNotFound(_) => StatusCode::NOT_FOUND`
   - Add mapping in `error_kind()`: `AnvilError::ArtifactNotFound(_) => "artifact_not_found"`
   - Add doc comment explaining the variant (404, artifact resource not found)

2. **Create `crates/anvilml-server/src/handlers/artifacts.rs`** with two handlers:

   a. `list_artifacts` handler:
   - Extract `State<AppState>` and `Query<ListArtifactsQuery>` (a struct with `job_id: Option<Uuid>`)
   - Call `state.artifact_store.list(params.job_id).await?`
   - Return `Json(artifacts)`
   - Add `#[tracing::instrument(skip(state))]`
   - Add `#[utoipa::path]` annotation for OpenAPI

   b. `serve_artifact` handler:
   - Extract `State<AppState>` and `Path(hash)` (the SHA-256 hex digest string)
   - Call `state.artifact_store.get(&hash).await?`
   - If `None`, return `Err(AnvilError::ArtifactNotFound(hash))`
   - If `Some(path)`, read file bytes with `tokio::fs::read(&path).await?`
   - Build `Response` body with `Body::from(bytes)` and set `Content-Type: image/png` header
   - Add `#[tracing::instrument(skip(state), fields(hash = %hash))]`
   - Add `#[utoipa::path]` annotation for OpenAPI

3. **Update `crates/anvilml-server/src/handlers/mod.rs`**:
   - Add `pub mod artifacts;`
   - Add `pub use artifacts::{list_artifacts, serve_artifact};`

4. **Update `crates/anvilml-server/src/lib.rs`**:
   - Add imports: `use handlers::artifacts::{list_artifacts, serve_artifact};`
   - Mount routes in `build_router()`:
     - `.route("/v1/artifacts", get(list_artifacts))`
     - `.route("/v1/artifacts/{hash}", get(serve_artifact))`
   - Place these routes after existing routes (before `.with_state(state)`)

5. **Write integration tests** in `crates/anvilml-server/tests/artifacts_tests.rs`:
   - Test 1: `test_list_artifacts_empty` — build router with empty artifact store, verify `GET /v1/artifacts` returns 200 with `[]`
   - Test 2: `test_list_artifacts_filtered` — save an artifact via `ArtifactStore`, call `GET /v1/artifacts?job_id=<id>`, verify it returns the artifact
   - Test 3: `test_serve_artifact_returns_png` — save an artifact, `GET /v1/artifacts/:hash`, verify 200 with `Content-Type: image/png` and non-empty body
   - Test 4: `test_serve_artifact_not_found` — `GET /v1/artifacts/nonexistent_hash`, verify 404

   Use the `test_state()` helper from `handler_tests.rs` (copy the pattern: build router, bind TcpListener, spawn server, send raw HTTP request, parse response).

6. **Version bump** `anvilml-server` in `Cargo.toml` from `0.1.23` to `0.1.24`.

## Public API Surface

| Item | Location | Signature |
|------|----------|-----------|
| `AnvilError::ArtifactNotFound(String)` | `anvilml-core/src/error.rs` | New enum variant, 404 status |
| `list_artifacts` | `anvilml-server/src/handlers/artifacts.rs` | `pub async fn list_artifacts(State<AppState>, Query<ListArtifactsQuery>) -> Result<Json<Vec<ArtifactMeta>>, AnvilError>` |
| `serve_artifact` | `anvilml-server/src/handlers/artifacts.rs` | `pub async fn serve_artifact(State<AppState>, Path<String>) -> Result<Response, AnvilError>` |
| `ListArtifactsQuery` | `anvilml-server/src/handlers/artifacts.rs` | `pub struct ListArtifactsQuery { pub job_id: Option<Uuid> }` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-core/src/error.rs` | Add `ArtifactNotFound` variant, status code mapping, error kind |
| CREATE | `crates/anvilml-server/src/handlers/artifacts.rs` | `list_artifacts` and `serve_artifact` handlers with utoipa annotations |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod artifacts` and re-exports |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Mount `/v1/artifacts` and `/v1/artifacts/{hash}` routes |
| CREATE | `crates/anvilml-server/tests/artifacts_tests.rs` | 4 integration tests for artifact endpoints |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.23 → 0.1.24 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_list_artifacts_empty` | Empty store returns `[]` with 200 | Fresh `AppState` with no artifacts saved | `GET /v1/artifacts` | 200, body `[]`, content-type `application/json` | `cargo test -p anvilml-server --features mock-hardware --test artifacts_tests -- test_list_artifacts_empty --exact` exits 0 |
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_list_artifacts_filtered` | List with job_id filter returns only matching artifacts | One artifact saved via `ArtifactStore` | `GET /v1/artifacts?job_id=<saved_job_id>` | 200, body contains exactly one artifact with matching job_id | `cargo test -p anvilml-server --features mock-hardware --test artifacts_tests -- test_list_artifacts_filtered --exact` exits 0 |
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_serve_artifact_returns_png` | Serve returns 200 with `image/png` content-type and non-empty body | One artifact saved via `ArtifactStore` | `GET /v1/artifacts/<hash>` | 200, `Content-Type: image/png`, body length > 0 | `cargo test -p anvilml-server --features mock-hardware --test artifacts_tests -- test_serve_artifact_returns_png --exact` exits 0 |
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_serve_artifact_not_found` | Unknown hash returns 404 | No artifacts saved | `GET /v1/artifacts/nonexistent_hash` | 404, error kind `artifact_not_found` | `cargo test -p anvilml-server --features mock-hardware --test artifacts_tests -- test_serve_artifact_not_found --exact` exits 0 |

## CI Impact

No CI changes required. The new test file is picked up by `cargo test --workspace --features mock-hardware` automatically (Rust test discovery). The new routes are covered by the openapi-drift CI gate which regenerates `api/openapi.json` and checks for drift — the `#[utoipa::path]` annotations on the new handlers will be reflected in the generated spec.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The `tokio::fs::read` used in `serve_artifact` is platform-neutral. The `PathBuf` returned by `ArtifactStore::get` works identically on Linux and Windows. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `axum::http::Response` API shape differs from what's expected — `Response::builder()` with `body(Body::from(bytes))` may have different method names in the workspace axum version. | Low | High | Verify the exact axum Response/Body API at plan time by reading the `axum` dependency in `Cargo.lock`. If `Body::from` is unavailable, use `axum::body::Body::from` or `http::Response::builder().body(...)` as fallback. |
| `ArtifactNotFound` error variant requires updating `AnvilError::error_kind()` and `status_code()` — if a match arm is missing, clippy will catch it with a `non_exhaustive_omitted_patterns` lint. | Low | Medium | The Rust compiler enforces exhaustive matching on enums. Adding the variant to the enum automatically makes all match arms non-exhaustive — clippy `-D warnings` will fail until all arms are updated. This is a self-correcting risk. |
| Integration tests using raw TCP streams may be slow or flaky if the server task doesn't start fast enough. | Medium | Low | Use `tokio::time::timeout` with a reasonable duration (5s) for connect and read operations, matching the pattern already used in `handler_tests.rs`. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --features mock-hardware -- AnvilError` exits 0 (error variant compiles and maps correctly)
- [ ] `cargo test -p anvilml-server --features mock-hardware --test artifacts_tests` exits 0 (all 4 integration tests pass)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no warnings from new code)
- [ ] `cargo fmt --all -- --check` exits 0 (code is formatted)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (no regressions)
