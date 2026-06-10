# Plan Report: P14-A5

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P14-A5                                            |
| Phase       | 014 — Artifact Storage                            |
| Description | anvilml-server: GET /v1/artifacts list (by job_id) |
| Depends on  | P14-A4                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-10T01:05:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add the `GET /v1/artifacts` list endpoint that returns artifact metadata filtered by optional `job_id`, `limit`, and `before` query parameters. The endpoint queries the `artifacts` SQLite table and returns a JSON array of `ArtifactMeta` objects.

## Scope

### In Scope
- Add `list(job_id: Option<String>, limit: u32, before: Option<i64>) -> Result<Vec<ArtifactMeta>>` method to `ArtifactStore` in `crates/anvilml-server/src/artifact/store.rs`.
- Add `list_artifacts` handler in `crates/anvilml-server/src/handlers/artifacts.rs` with query params: `job_id` (UUID string), `limit` (default 100, max 1000), `before` (Unix timestamp, optional).
- Wire `GET /v1/artifacts` route in `crates/anvilml-server/src/lib.rs` **before** the existing `GET /v1/artifacts/{hash}` route (axum matches in order).
- Unit tests for `ArtifactStore::list()` with an in-memory SQLite database seeded with artifact rows.
- Unit tests for the `list_artifacts` handler (empty list, filtered by job_id, limit clamping).
- Bump `anvilml-server` patch version from `0.1.7` to `0.1.8`.

### Out of Scope
- `DELETE /v1/artifacts` (not in phase scope).
- `GET /v1/artifacts/:hash` (already implemented in P14-A4).
- OpenAPI regeneration (handled by CI drift gate in ACT session).
- Changes to `anvilml-core` domain types (the local `ArtifactMeta` in `store.rs` is used).
- Changes to `anvilml-scheduler` or `anvilml-registry` crates.

## Approach

### Step 1: Add `list()` method to `ArtifactStore` (`crates/anvilml-server/src/artifact/store.rs`)

Add a new async method to the existing `impl ArtifactStore` block:

```rust
/// List artifacts, optionally filtered by job_id with pagination.
///
/// Queries the `artifacts` table and returns matching rows sorted newest-first.
#[tracing::instrument(skip(self), fields(job_id = ?job_id))]
pub async fn list(
    &self,
    job_id: Option<String>,
    limit: u32,
    before: Option<i64>,
) -> Result<Vec<ArtifactMeta>, ArtifactError> {
    // Build query with optional WHERE clause for job_id and before.
    // SELECT hash, job_id, width, height, format, seed, steps, prompt, created_at
    // FROM artifacts
    // WHERE job_id = ? AND created_at < ?
    // ORDER BY created_at DESC LIMIT ?
    // Returns Vec<ArtifactMeta>.
}
```

The SQL query uses optional `WHERE` clauses:
- If `job_id` is `Some`, add `WHERE job_id = ?`.
- If `before` is `Some`, add `AND created_at < ?` (or `WHERE` if no job_id filter).
- Order by `created_at DESC` (newest first).
- Limit to `limit`.

### Step 2: Add `list_artifacts` handler (`crates/anvilml-server/src/handlers/artifacts.rs`)

Add a query struct and handler function following the pattern from `handlers/jobs.rs`:

```rust
/// Query parameters for the GET /v1/artifacts list endpoint.
#[derive(Debug, Deserialize, Default)]
pub struct ListArtifactsQuery {
    /// Filter by job UUID (optional — return all artifacts if omitted).
    pub job_id: Option<String>,
    /// Maximum number of results (default 100, max 1000).
    pub limit: Option<u32>,
    /// Only return artifacts created before this Unix timestamp.
    pub before: Option<String>,
}

/// List artifacts with optional job_id, limit, and before filters.
///
/// Returns a JSON array of ArtifactMeta objects sorted newest-first.
#[utoipa::path(
    get,
    path = "/v1/artifacts",
    summary = "List artifacts with optional filters",
    params(
        ("job_id" = Option<Uuid>, Query, description = "Filter by job UUID"),
        ("limit" = Option<u32>, Query, description = "Maximum number of results (default 100, max 1000)"),
        ("before" = Option<i64>, Query, description = "Only artifacts created before this Unix timestamp")
    ),
    responses(
        (status = 200, description = "Artifact list", body = Vec<ArtifactMeta>),
        (status = 503, description = "Database not available", body = serde_json::Value)
    )
)]
pub async fn list_artifacts(
    State(state): State<Arc<App>>,
    Query(query): Query<ListArtifactsQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Parse before as i64 (Unix timestamp), default 0 = no filter.
    // Compute effective limit: default 100, clamped to [1, 1000].
    // Call state.artifact_store.list(job_id, effective_limit, parsed_before).
    // Return 200 with JSON array on success, 503 if no DB.
}
```

The handler follows the same error-handling pattern as `list_jobs` in `handlers/jobs.rs`:
- Check `state.db` is `Some` → return 503 if `None`.
- Parse `before` from string to `i64` (Unix timestamp). Invalid values are ignored (logged as warn, treated as no filter).
- Clamp `limit` to `[1, 1000]`, default to `100`.
- Call `artifact_store.list()`, return results as JSON.

### Step 3: Wire the route (`crates/anvilml-server/src/lib.rs`)

Add the route **before** the existing `/v1/artifacts/{hash}` route in `build_router()`:

```rust
.route("/v1/artifacts", get(handlers::artifacts::list_artifacts))
.route(
    "/v1/artifacts/{hash}",
    get(handlers::artifacts::serve_artifact),
)
```

**Order matters**: axum matches routes in definition order. The parameterless route must come first, otherwise `/v1/artifacts` would be captured as a hash value.

### Step 4: Bump `anvilml-server` version (`crates/anvilml-server/Cargo.toml`)

Change `version = "0.1.7"` → `version = "0.1.8"` in the `[package]` section.

### Step 5: Add unit tests

Add a `mod tests` block to `crates/anvilml-server/src/artifact/store.rs` (or a new test file `crates/anvilml-server/tests/artifact_list.rs` — but per the existing convention in this crate, tests are in-module, so add to `store.rs`).

Tests:
1. **`list_empty_returns_empty_array`** — create store with empty in-memory DB, call `list(None, 100, None)`, verify empty `Vec`.
2. **`list_with_job_id_filter`** — insert two artifacts with different `job_id` values, call `list(Some(job_a), 100, None)`, verify only job_a's artifact is returned.
3. **`list_limit_clamped`** — insert 5 artifacts, call with `limit=2`, verify exactly 2 returned.
4. **`list_before_filter`** — insert artifacts with different timestamps, call with `before` set to a middle timestamp, verify only newer artifacts returned.

Add a test to `crates/anvilml-server/src/handlers/artifacts.rs` for the handler:
5. **`list_artifacts_empty_returns_200_with_empty_array`** — use test app, GET `/v1/artifacts`, verify 200 + empty array.
6. **`list_artifacts_with_job_id_filter`** — insert artifacts via raw SQL, GET `/v1/artifacts?job_id=...`, verify filtered results.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/artifact/store.rs` | Add `list()` method to `ArtifactStore` |
| Modify | `crates/anvilml-server/src/handlers/artifacts.rs` | Add `ListArtifactsQuery` struct and `list_artifacts` handler |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire `GET /v1/artifacts` route before `{hash}` route |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version `0.1.7 → 0.1.8` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-server/src/artifact/store.rs` (tests) | `list_empty_returns_empty_array` | Empty DB returns `[]` |
| `crates/anvilml-server/src/artifact/store.rs` (tests) | `list_with_job_id_filter` | Filtering by `job_id` returns correct subset |
| `crates/anvilml-server/src/artifact/store.rs` (tests) | `list_limit_clamped` | Limit clamping and row count |
| `crates/anvilml-server/src/artifact/store.rs` (tests) | `list_before_filter` | `before` timestamp filter returns only newer artifacts |
| `crates/anvilml-server/src/handlers/artifacts.rs` (tests) | `list_artifacts_empty_returns_200_with_empty_array` | Handler returns 200 + `[]` when DB empty |
| `crates/anvilml-server/src/handlers/artifacts.rs` (tests) | `list_artifacts_with_job_id_filter` | Handler passes query params to store and returns filtered results |

## CI Impact

The new `GET /v1/artifacts` route changes the HTTP API surface. After implementation, the OpenAPI drift gate (`cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json`) must be run and committed. All existing CI gates (format, clippy, test, cross-check) continue to apply. No new CI workflow jobs are added.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Route ordering conflict between `/v1/artifacts` and `/v1/artifacts/{hash}` | Low | High — wrong route matches | Place `/v1/artifacts` route **before** `/v1/artifacts/{hash}` in `build_router()` |
| `job_id` type mismatch (String vs Uuid) in DB vs query | Low | Medium — query returns empty | The DB stores `job_id` as TEXT; the handler accepts `job_id` as `Option<String>` (deserialized from query param) and passes it directly to SQL — no conversion needed |
| `before` timestamp parse failure | Low | Low — silently ignored | Log a `warn!` and treat invalid `before` as "no filter", consistent with `list_jobs` pattern |
| Missing `uuid` dependency for `ToSchema` in utoipa annotation | Low | Medium — compile error | Use `Option<String>` in the query struct (already available), and reference `Uuid` only in the utoipa `params` doc for the `job_id` field |

## Acceptance Criteria

- [ ] `ArtifactStore::list(job_id, limit, before)` compiles and returns correct filtered results against an in-memory SQLite DB
- [ ] `list_artifacts` handler parses query params, delegates to store, returns JSON array
- [ ] `GET /v1/artifacts` route is wired in `build_router()` before `/v1/artifacts/{hash}`
- [ ] `cargo test -p anvilml-server --features mock-hardware` exits 0 (all existing + new tests pass)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `anvilml-server` version bumped to `0.1.8` in `Cargo.toml`
- [ ] OpenAPI drift gate green after regeneration (`backend/openapi.json` updated)
