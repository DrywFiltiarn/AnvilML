# Plan Report: P14-A4

| Field | Value |
|-------|-------|
| Task ID | P14-A4 |
| Phase | 014 — Artifact Storage |
| Description | anvilml-server: GET /v1/artifacts/:hash serves PNG |
| Depends on | P14-A1, P14-A2, P14-A3 |
| Project | anvilml |
| Planned at | 2026-06-10T00:15:00Z |
| Attempt | 1 |

## Objective

Add `get_path(hash) -> Result<PathBuf>` to `ArtifactStore` and wire a new `GET /v1/artifacts/:hash` endpoint that returns the stored PNG file with appropriate HTTP headers (Content-Type, Cache-Control, ETag), or 404 if the artifact is missing.

## Scope

### In Scope
- Add `get_path(&self, hash: &str) -> Result<PathBuf, ArtifactError>` to `artifact/store.rs`
- Create `handlers/artifacts.rs` with `serve_artifact` handler
- Wire `GET /v1/artifacts/{hash}` route in `lib.rs`
- Update `artifact/mod.rs` to export new types
- Update `handlers/mod.rs` to export the artifacts module
- Add unit + integration tests for the endpoint
- Update `Cargo.toml` if new dependencies are needed (tower-http for ServeFile/FileSystem)

### Out of Scope
- `GET /v1/artifacts` list endpoint (P14-A5, next task)
- `DELETE /v1/jobs/:id` artifact cleanup (deferred)
- Any changes to artifact save/persist logic
- Any changes to the scheduler or worker
- OpenAPI regeneration (handled by CI drift gate)

## Approach

1. **Add `get_path` to `ArtifactStore`** (`crates/anvilml-server/src/artifact/store.rs`):
   - Implement `pub async fn get_path(&self, hash: &str) -> Result<PathBuf, ArtifactError>`
   - Constructs `{artifact_dir}/{hash[0..2]}/{hash}.png` using the same prefix-sharding logic as `save()`
   - Returns `Ok(path)` unconditionally (the caller checks existence) or returns an Io error if the path is invalid
   - This mirrors the file path construction in `save()` (line 118–122 of store.rs)

2. **Create `handlers/artifacts.rs`** (`crates/anvilml-server/src/handlers/artifacts.rs`):
   - Define `serve_artifact` async function with signature matching axum's `Handler`:
     ```rust
     pub async fn serve_artifact(
         State(state): State<App>,
         Path(hash): Path<String>,
     ) -> Result<impl IntoResponse, AppError>
     ```
   - Call `state.artifact_store.get_path(&hash).await`
   - If `Err` (Io error for missing file): return 404 JSON error `{"error":"artifact_not_found","message":"artifact not found"}`
   - On `Ok(path)`: use `axum::response::File` or `tower_http::services::ServeFile` to stream the file
   - Set response headers:
     - `Content-Type: image/png`
     - `Cache-Control: public, immutable, max-age=31536000`
     - `ETag: "{hash}"` (with surrounding quotes as per HTTP spec)

3. **Wire the route in `lib.rs`**:
   - Add `.route("/v1/artifacts/{hash}", get(handlers::artifacts::serve_artifact))` to the router chain
   - Place it after existing `/v1/*` routes but before the frontend catch-all (if any)

4. **Update module exports**:
   - `artifact/mod.rs`: add `pub use store::ArtifactStore;` (already present) — no change needed
   - `handlers/mod.rs`: add `pub mod artifacts;`

5. **Add tests**:
   - Unit test in `handlers/artifacts.rs` (or inline in a `tests` module):
     - Test 404 when artifact file doesn't exist
     - Test 200 with correct headers when artifact file exists
   - Integration test in `crates/anvilml-server/tests/api_artifact_serve.rs`:
     - Save an artifact using `ArtifactStore::save()` (reuse `MINIMAL_PNG_B64` from existing test)
     - GET `/v1/artifacts/{hash}` and verify status 200, Content-Type, Cache-Control, ETag
     - GET `/v1/artifacts/nonexistent` and verify status 404

6. **Dependency check**:
   - `tower-http` is already in `Cargo.toml` with `cors` feature. Need to add `fs` feature for `ServeFile`.
   - Check workspace Cargo.toml for existing `tower-http` features.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/artifact/store.rs` | Add `get_path()` method to `ArtifactStore` |
| Create | `crates/anvilml-server/src/handlers/artifacts.rs` | New handler module with `serve_artifact` |
| Modify | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod artifacts;` |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire `GET /v1/artifacts/{hash}` route |
| Modify | `crates/anvilml-server/Cargo.toml` | Add `fs` feature to `tower-http` dependency |
| Create | `crates/anvilml-server/tests/api_artifact_serve.rs` | Integration tests for artifact serve endpoint |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/tests/api_artifact_serve.rs` | `artifact_serve_404_when_missing` | GET /v1/artifacts/{nonexistent} returns 404 with artifact_not_found error JSON |
| `crates/anvilml-server/tests/api_artifact_serve.rs` | `artifact_serve_200_with_headers` | GET /v1/artifacts/{hash} returns 200, Content-Type: image/png, Cache-Control: public, immutable, max-age=31536000, ETag with hash |
| `crates/anvilml-server/tests/api_artifact_serve.rs` | `artifact_serve_returns_correct_bytes` | The downloaded file bytes match the originally saved PNG bytes |
| `crates/anvilml-server/src/handlers/artifacts.rs` (inline test) | `get_path_returns_correct_sharded_path` | `ArtifactStore::get_path()` constructs the correct two-char prefix-sharded path |

## CI Impact

This task modifies handler code and adds a new route, which triggers the **OpenAPI drift gate** in CI. After implementation, `cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json` must pass (the generated openapi.json will include the new endpoint's schema). The `tower-http` feature addition (`fs`) is a non-breaking dependency change — no CI workflow modifications needed. All existing tests must continue to pass; the new integration test is additive.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tower-http` `fs` feature not available or API changed | Low | Medium | Check workspace `Cargo.toml` for `tower-http` version; verify `ServeFile` API shape before implementation |
| Handler signature mismatch with existing `State` pattern | Low | Low | Follow the exact same `State<App>` pattern used in all existing handlers (`handlers/jobs.rs`, `handlers/models.rs`, etc.) |
| 404 error format mismatch with existing error responses | Medium | Low | Follow the uniform error body format from ANVILML_DESIGN.md §18: `{"error":"artifact_not_found","message":"artifact not found","request_id":"..."}` |
| ETag quoting — missing surrounding quotes | Medium | Low | HTTP spec requires ETag values in double quotes; use format `ETag: "{hash}"` (the hash itself has no quotes) |
| Path traversal attack via crafted hash | Low | Medium | The hash is a SHA-256 hex string (64 chars, [0-9a-f] only); no path traversal possible. The `get_path` method constructs the path deterministically from the hash with no user-controlled path segments. |

## Acceptance Criteria

- [ ] `ArtifactStore::get_path(hash)` returns the correct `PathBuf` for a given hash, using the two-char prefix sharding scheme
- [ ] `GET /v1/artifacts/{hash}` returns 404 JSON with `error: "artifact_not_found"` when the artifact file does not exist
- [ ] `GET /v1/artifacts/{hash}` returns 200 with `Content-Type: image/png` when the artifact file exists
- [ ] Response includes `Cache-Control: public, immutable, max-age=31536000` header
- [ ] Response includes `ETag: "{hash}"` header (with surrounding quotes)
- [ ] Response body is the raw PNG bytes matching the stored artifact
- [ ] `cargo test -p anvilml-server --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
