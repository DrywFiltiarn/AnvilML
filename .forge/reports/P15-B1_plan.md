# Plan Report: P15-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P15-B1                                            |
| Phase       | 015 — Artifact Storage Wiring                     |
| Description | anvilml-server: GET /v1/artifacts list handler    |
| Depends on  | P15-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-08T20:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Expose the already-implemented `ArtifactStore::list()` method over HTTP by creating a new `artifacts.rs` handler module in `anvilml-server`, registering it in `handlers/mod.rs`, and wiring `GET /v1/artifacts` into `build_router()`. The handler accepts an optional `job_id` query parameter, delegates entirely to `state.artifact_store.list()`, and returns `200 OK` with a JSON array of `ArtifactMeta` objects. This is the first time the artifact store is reachable from outside the test suite, closing the gap identified in Phase 15's overview.

## Scope

### In Scope
- **CREATE** `crates/anvilml-server/src/handlers/artifacts.rs` with:
  - `ListArtifactsParams` struct (HTTP-layer query params: `job_id: Option<Uuid>`)
  - `list_artifacts()` async handler function
- **MODIFY** `crates/anvilml-server/src/handlers/mod.rs` to declare `pub mod artifacts;`
- **MODIFY** `crates/anvilml-server/src/lib.rs` to register `GET /v1/artifacts` in `build_router()`
- **CREATE** `crates/anvilml-server/tests/artifacts_tests.rs` with ≥4 integration tests

### Out of Scope
- `GET /v1/artifacts/:hash` (individual artifact retrieval) — deferred to P15-B2, which extends `artifacts.rs` with `get_artifact()`. P15-B2's description ("Extend crates/anvilml-server/src/handlers/artifacts.rs with get_artifact(...)") genuinely covers this scope.

## Existing Codebase Assessment

The `ArtifactStore` (in `anvilml-artifacts/src/store.rs`) is fully implemented with `save()`, `get()`, and `list()` methods. The `list()` method accepts `Option<Uuid>` for optional job_id filtering and returns `Result<Vec<ArtifactMeta>, AnvilError>`. The `AppState` struct (in `state.rs`) already has the `artifact_store: Arc<ArtifactStore>` field added by P15-A1.

Established patterns in `anvilml-server`:
- Handler functions are `pub(crate)` async functions returning `Result<Json<T>, AnvilError>` or `(StatusCode, Json<T>)`.
- Query parameter structs derive `Deserialize` and live in the same handler module.
- The `#[tracing::instrument]` attribute decorates handler functions.
- Tests use `tower::util::ServiceExt::oneshot()` with in-memory SQLite pools and the `make_test_state()` / `build_router()` pattern from `jobs_tests.rs`.
- `handlers/mod.rs` declares `pub mod` for each handler submodule.
- `lib.rs`'s `build_router()` registers routes using `axum::routing::get()`.

No gap between the design doc and current source: `AppState` already holds `artifact_store`, and `ArtifactStore::list()` has the exact signature the handler needs.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | axum    | 0.8.9           | rust-docs MCP  | none (default)         |
| crate  | serde   | 1.0             | project lock   | derive                 |
| crate  | uuid    | 1.23            | project lock   | v4, serde              |

No new external crates are introduced. All types (`axum::extract::Query`, `axum::json::Json`, `axum::routing::get`) are confirmed present in axum v0.8.9 via MCP search.

## Approach

1. **Create `crates/anvilml-server/src/handlers/artifacts.rs`** with the following contents:

   a. Module-level doc comment describing the handler per `ANVILML_DESIGN.md §13.4`.
   b. Import block: `anvilml_core::ArtifactMeta`, `anvilml_core::AnvilError`, `axum::Json`, `axum::extract::Query`, `axum::extract::State`, `serde::Deserialize`, `uuid::Uuid`.
   c. `ListArtifactsParams` struct with `///` doc comment: derives `Deserialize`, has one field `job_id: Option<Uuid>` with `#[serde(default)]` so the query parameter is optional.
   d. `list_artifacts()` async handler: takes `State(state): State<AppState>` and `Query(params): Query<ListArtifactsParams>`, returns `Result<Json<Vec<ArtifactMeta>>, AnvilError>`. Body is a single delegation line: `state.artifact_store.list(params.job_id).await.map(Json)`. Decorated with `#[tracing::instrument(skip(state), fields(job_id = ?params.job_id))]`. `///` doc comment describing what it does, the response shape, and the `AnvilError` variants it can propagate (Db from the store).
   e. Add `#[cfg(test)]` module with an inline helper `make_artifact_state()` that constructs a minimal `AppState` with an `ArtifactStore` backed by a temp dir and in-memory SQLite — used only by tests.

2. **Modify `crates/anvilml-server/src/handlers/mod.rs`**: add `pub mod artifacts;` after the existing `pub mod nodes;` line.

3. **Modify `crates/anvilml-server/src/lib.rs`**: add `.route("/v1/artifacts", axum::routing::get(handlers::artifacts::list_artifacts))` to the router chain, placed before `.with_state(app_state)` and after the existing `/v1/nodes` route.

4. **Create `crates/anvilml-server/tests/artifacts_tests.rs`** with ≥4 integration tests:

   a. `test_list_artifacts_empty_store_returns_200_empty_array`: constructs state with empty store, sends GET `/v1/artifacts`, asserts 200 and body is `[]`.
   b. `test_list_artifacts_populated_returns_all`: saves two artifacts via the store's `save()` method (using distinct PNG bytes), sends GET `/v1/artifacts` with no filter, asserts 200 and array length is 2.
   c. `test_list_artifacts_job_id_filter_returns_matching`: saves two artifacts with different `job_id` values, sends GET `/v1/artifacts?job_id=<first_id>`, asserts 200 and array length is 1 with the correct job_id.
   d. `test_list_artifacts_json_shape`: saves one artifact, sends GET `/v1/artifacts`, deserialises the body into `serde_json::Value`, and asserts the presence and types of all `ArtifactMeta` fields (`hash` is string, `job_id` is string (UUID), `width`/`height`/`steps` are integers, `seed` is integer, `created_at` is string, `file_path` is string).

   Each test uses the `make_test_state()` pattern from `jobs_tests.rs` (copy the helper or use it via the test crate's access to the library). The tests use `tower::util::ServiceExt::oneshot()` and `axum::body::to_bytes()` for response inspection.

5. **Bump `anvilml-server` patch version** in `crates/anvilml-server/Cargo.toml` from `0.1.8` to `0.1.9` per `ENVIRONMENT.md §12`.

## Public API Surface

| Item | Crate/Module | Signature |
|------|-------------|-----------|
| `list_artifacts` | `anvilml-server::handlers::artifacts` (pub(crate)) | `async fn list_artifacts(State(state): State<AppState>, Query(params): Query<ListArtifactsParams>) -> Result<Json<Vec<ArtifactMeta>>, AnvilError>` |
| `ListArtifactsParams` | `anvilml-server::handlers::artifacts` (pub(crate)) | `struct ListArtifactsParams { job_id: Option<Uuid> }` |

Route registration: `GET /v1/artifacts` → `handlers::artifacts::list_artifacts` (registered in `build_router()`).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/handlers/artifacts.rs` | New handler module with `list_artifacts()` and `ListArtifactsParams` |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod artifacts;` |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Register `GET /v1/artifacts` route in `build_router()` |
| CREATE | `crates/anvilml-server/tests/artifacts_tests.rs` | ≥4 integration tests for the handler |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.8 → 0.1.9 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_list_artifacts_empty_store_returns_200_empty_array` | Empty artifact store returns HTTP 200 with JSON array `[]` | `cargo test -p anvilml-server --test artifacts_tests test_list_artifacts_empty_store_returns_200_empty_array` |
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_list_artifacts_populated_returns_all` | Populated store with no filter returns all artifacts | `cargo test -p anvilml-server --test artifacts_tests test_list_artifacts_populated_returns_all` |
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_list_artifacts_job_id_filter_returns_matching` | `?job_id=<uuid>` filter returns only matching artifacts | `cargo test -p anvilml-server --test artifacts_tests test_list_artifacts_job_id_filter_returns_matching` |
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_list_artifacts_json_shape` | Response body has correct `ArtifactMeta` field names and types | `cargo test -p anvilml-server --test artifacts_tests test_list_artifacts_json_shape` |

## CI Impact

No CI changes required. The test file is a standard integration test under `crates/anvilml-server/tests/`, which is already picked up by `cargo test --workspace --features mock-hardware`. No new file types, gates, or CI jobs are introduced.

## Platform Considerations

None identified. The handler is a pure HTTP delegation with no platform-specific code paths. The `#[cfg(unix)]` / `#[cfg(windows)]` cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ArtifactStore::list()` calls `ensure_artifacts_table()` which runs `CREATE TABLE IF NOT EXISTS` — if the in-memory test pool doesn't support this DDL, the test will fail with a DB error. | Low | High | The `list()` method already calls `ensure_artifacts_table()` before querying (confirmed in source at line 257 of `store.rs`). The test pool uses the same in-memory SQLite as `jobs_tests.rs`, which already runs migrations successfully. Verify by running the test suite. |
| `serde::Deserialize` on `ListArtifactsParams` may fail to parse the query string if `uuid` deserialisation is not configured for query params. | Low | Medium | `Option<Uuid>` with `#[serde(default)]` is the standard pattern — `serde_urlencoded` (used by axum's `Query` extractor) uses `Deserialize::deserialize` which calls `Uuid::deserialize` for the UUID type. Confirmed by existing patterns in `ListJobsParams` (jobs.rs) which uses `Option<JobStatus>`. |
| The handler's `Result<Json<Vec<ArtifactMeta>>, AnvilError>` return may not compile if `Vec<ArtifactMeta>` does not implement `Serialize`. | Very low | High | `ArtifactMeta` derives `Serialize` (confirmed in `artifact.rs` line 13). `Vec<T>` implements `Serialize` when `T` does. No issue expected. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test artifacts_tests` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `head -1 .forge/reports/P15-B1_plan.md` prints `# Plan Report: P15-B1`
- [ ] `grep "^## " .forge/reports/P15-B1_plan.md` shows all 12 section headings
- [ ] `wc -l .forge/reports/P15-B1_plan.md` reports > 40 lines
