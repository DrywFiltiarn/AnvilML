# Plan Report: P15-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P15-B2                                      |
| Phase       | 15 — Artifact Storage Wiring                |
| Description | anvilml-server: GET /v1/artifacts/:hash serve PNG bytes |
| Depends on  | P15-B1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-08T19:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement the `GET /v1/artifacts/{hash}` HTTP handler that serves raw PNG bytes for a
content-addressed artifact by its SHA-256 hash. The handler delegates to
`ArtifactStore::get()`, returning `200 OK` with `Content-Type: image/png` on success,
or `404 Not Found` via `AnvilError::ArtifactNotFound(hash)` when the artifact does not
exist. This completes the second half of the artifact HTTP surface started by P15-B1
(`GET /v1/artifacts` list).

## Scope

### In Scope
- Add `get_artifact(State<AppState>, Path(String)) -> Result<Response, AnvilError>` to
  `crates/anvilml-server/src/handlers/artifacts.rs`.
- Wire `GET /v1/artifacts/{hash}` route in `crates/anvilml-server/src/lib.rs`
  `build_router()`, registered after `/v1/artifacts` so the literal path takes priority
  over the parameterised path (axum matches longest literal first, but explicit ordering
  makes intent clear).
- Add at least 4 new integration tests in `crates/anvilml-server/tests/artifacts_tests.rs`:
  existing hash returns 200 with correct Content-Type and bytes, unknown hash returns 404,
  byte-for-byte match with saved content.
- Bump `anvilml-server` crate version from `0.1.9` to `0.1.10`.

### Out of Scope
None. `defers_to (from JSON): []`. This task implements its full scope.

## Existing Codebase Assessment

(a) **What already exists:** The `ArtifactStore` (Phase 6) provides `get(&self, hash: &str)
-> Result<Option<Vec<u8>>, AnvilError>` which reads the PNG file from the content-addressed
directory. `AppState` already holds `artifact_store: Arc<ArtifactStore>` (added by P15-A1).
`AnvilError::ArtifactNotFound(String)` is already defined in `error.rs` and maps to HTTP 404.
The existing `artifacts.rs` module has `list_artifacts()` and a `make_test_state()` helper
in the test file, plus 4 integration tests covering the list endpoint.

(b) **Established patterns:** Handlers are `pub(crate) async fn` taking `State<AppState>`
and optional `Path`/`Query` extractors, returning `Result<T, AnvilError>`. The `#[tracing::instrument]`
macro annotates handlers. Tests use `tower::util::ServiceExt::oneshot()` for in-process
HTTP requests against `build_router()`. The `save_artifact()` helper in the test file
constructs synthetic PNG bytes and calls `store.save()`.

(c) **Gap between design doc and source:** None — `AnvilError::ArtifactNotFound` is already
present (resolved per `ADDENDUM_ARTIFACT_NOT_FOUND.md`), and `ArtifactStore::get()` already
exists. The only missing piece is the handler function and route registration.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | axum    | 0.8.9           | Cargo.lock     | n/a                    |
| crate  | anvilml-artifacts | 0.1.x (workspace) | workspace path dep | n/a              |

No new external dependencies are introduced. The existing `anvilml-artifacts` path
dependency already exposes `ArtifactStore::get()`.

## Approach

1. **Add `get_artifact()` handler to `artifacts.rs`.**
   - Signature: `pub(crate) async fn get_artifact(State(state): State<AppState>, Path(hash): Path<String>) -> Result<axum::response::Response, AnvilError>`
   - Call `state.artifact_store.get(&hash).await`.
   - On `Ok(Some(bytes))`: construct an `axum::response::Response` with `Body::from(bytes)`,
     set `Content-Type: image/png` header, return `Ok(response)`.
   - On `Ok(None)`: return `Err(AnvilError::ArtifactNotFound(hash))` — the dedicated 404
     variant (not `Internal`).
   - On `Err(e)`: propagate the error — an I/O error from the store maps to `AnvilError::Io`
     via the `From<std::io::Error>` impl, which returns HTTP 500.
   - Add `#[tracing::instrument(skip(state), fields(hash = %hash))]` for observability.
   - Add a `///` doc comment describing the handler's purpose, response, and error variants.

2. **Register the route in `lib.rs` `build_router()`.**
   - Add `.route("/v1/artifacts/{hash}", axum::routing::get(handlers::artifacts::get_artifact))`
     after the existing `/v1/artifacts` route. The `{hash}` syntax matches the axum 0.8
     convention used throughout the codebase (e.g. `/v1/jobs/{id}`).

3. **Add integration tests in `artifacts_tests.rs`.**
   - Follow the established pattern: `make_test_state()`, `build_router(state)`,
     `router.oneshot(req)`, `to_bytes()` for body inspection.
   - Use the existing `save_artifact()` helper to create test artifacts, then retrieve
     them via the new handler.

4. **Bump `anvilml-server` version** in `Cargo.toml` from `0.1.9` to `0.1.10`.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| `get_artifact` | `anvilml_server::handlers::artifacts` | `pub(crate) async fn get_artifact(State(state): State<AppState>, Path(hash): Path<String>) -> Result<axum::response::Response, AnvilError>` |

No new `pub` items are introduced — `get_artifact` is `pub(crate)` only, matching the
established pattern for handler functions.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/handlers/artifacts.rs` | Add `get_artifact()` handler function |
| Modify | `crates/anvilml-server/src/lib.rs` | Register `GET /v1/artifacts/{hash}` route in `build_router()` |
| Modify | `crates/anvilml-server/tests/artifacts_tests.rs` | Add >=4 new integration tests |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.9 → 0.1.10 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `artifacts_tests.rs` | `test_get_artifact_existing_hash_returns_200` | Saved artifact retrieved with status 200 and `Content-Type: image/png` | `cargo test -p anvilml-server --test artifacts_tests -- test_get_artifact_existing_hash_returns_200` exits 0 |
| `artifacts_tests.rs` | `test_get_artifact_unknown_hash_returns_404` | Unknown hash returns `StatusCode::NOT_FOUND` (404) via `AnvilError::ArtifactNotFound` | `cargo test -p anvilml-server --test artifacts_tests -- test_get_artifact_unknown_hash_returns_404` exits 0 |
| `artifacts_tests.rs` | `test_get_artifact_byte_for_byte_match` | Response body bytes exactly match the PNG bytes that were saved | `cargo test -p anvilml-server --test artifacts_tests -- test_get_artifact_byte_for_byte_match` exits 0 |
| `artifacts_tests.rs` | `test_get_artifact_content_type_header` | Response `Content-Type` header is exactly `image/png` | `cargo test -p anvilml-server --test artifacts_tests -- test_get_artifact_content_type_header` exits 0 |

Total tests in file after this task: 8 (4 existing + 4 new).

## CI Impact

No CI changes required. The task adds a new handler and tests within the existing
`anvilml-server` crate. The existing CI job `rust-linux` runs `cargo test --workspace
--features mock-hardware` which picks up the new tests automatically. The
`openapi-drift` job may need to be re-run if the OpenAPI spec includes endpoint
documentation (the handler uses `#[utoipa::path]` annotations), but since no
`utoipa` imports are present in existing handler files, this handler does not
add OpenAPI documentation — it is an internal handler only.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The
handler is platform-neutral: `ArtifactStore::get()` uses `std::fs::read()` which
works identically on Linux and Windows, and `axum::response::Response` with
`Body::from()` is platform-agnostic.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `axum::routing::get()` route ordering: `/v1/artifacts/{hash}` must not shadow `/v1/artifacts` | Low | High | Register `/v1/artifacts` before `/v1/artifacts/{hash}` in `build_router()`. Axum matches literal paths before parameterised ones, but explicit ordering makes intent clear and prevents future regressions if route order changes. |
| `AnvilError::ArtifactNotFound` returns HTTP 404 but the handler needs to distinguish "not found" from "I/O error" | Low | Medium | `ArtifactStore::get()` already returns `Ok(None)` for missing files (not an error), and only returns `Err` for genuine I/O failures (permission denied, etc.). The handler maps `Ok(None)` → `ArtifactNotFound` and propagates `Err` as-is. |
| Large artifact files could cause memory pressure when loaded into `Body::from()` | Low | Medium | Artifacts are PNG images (typically < 10 MB). The handler loads the entire file into memory, which is acceptable for this size range. A streaming response would be needed only for very large artifacts (> 100 MB), which is out of scope. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test artifacts_tests` exits 0 (>= 8 total tests)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo build -p anvilml-server` exits 0 (compilation check)
