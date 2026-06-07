# Plan Report: P12-A5

| Field       | Value                                          |
|-------------|------------------------------------------------|
| Task ID     | P12-A5                                         |
| Phase       | 012 — Job Submission & Queue                   |
| Description | anvilml-server: GET /v1/jobs list with status/limit/before |
| Depends on  | P12-A4                                         |
| Project     | anvilml                                        |
| Planned at  | 2026-06-07T17:35:00Z                           |
| Attempt     | 1                                              |

## Objective

Add the `GET /v1/jobs` list endpoint to the `anvilml-server` HTTP API. The handler accepts optional query parameters (`status`, `limit`, `before`) and delegates to `job_store::list_jobs` which was implemented in P12-A1. Default limit is 100 (from config), clamped to a maximum of 1000.

## Scope

### In Scope
- Add `list_jobs` handler function in `crates/anvilml-server/src/handlers/jobs.rs`:
  - Parse query parameters: `status: Option<JobStatus>`, `limit: Option<u32>`, `before: Option<DateTime<Utc>>`
  - Clamp `limit` to `[1, 1000]` (configurable via `LimitsConfig::list_max_limit`)
  - Call `job_store::list_jobs(pool, status, limit, before)`
  - Return `200 OK` with JSON array of jobs, or `503 Service Unavailable` if DB is missing
- Wire the handler as `GET /v1/jobs` in `crates/anvilml-server/src/lib.rs` (add to existing `.route("/v1/jobs", post(...))` using axum's multi-method route)
- Add unit tests in `handlers/jobs.rs`:
  - Submit 2 jobs via POST, then GET /v1/jobs lists both
  - GET /v1/jobs?status=queued filters to only queued jobs
  - GET /v1/jobs?limit=1 returns exactly one job
- Add `utoipa::path` annotation for OpenAPI schema generation

### Out of Scope
- Pagination beyond the `before` cursor (no next-page token or offset)
- Sorting options (always newest-first per existing list_jobs SQL)
- DELETE /v1/jobs bulk clear (separate task)
- GET /v1/jobs/:id (already implemented in P12-A4)
- POST /v1/jobs (already implemented in P12-A4)

## Approach

### Step 1 — Add handler to `handlers/jobs.rs`

Add a new async function `list_jobs` following the established pattern from `submit_job` and `get_job`:

```rust
use axum::extract::Query;

#[derive(serde::Deserialize)]
pub struct ListJobsQuery {
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub before: Option<String>,
}

#[utoipa::path(
    get,
    path = "/v1/jobs",
    summary = "List jobs with optional filters",
    params(
        ("status" = Option<JobStatus>, Query, description = "Filter by job status"),
        ("limit" = Option<u32>, Query, description = "Maximum number of results (default 100, max 1000)"),
        ("before" = Option<String>, Query, description = "Only jobs created before this ISO 8601 timestamp")
    ),
    responses(
        (status = 200, description = "Job list", body = Vec<anvilml_core::types::job::Job>),
        (status = 503, description = "Database not available", body = ErrorInline)
    )
)]
pub async fn list_jobs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListJobsQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    // 1. Check DB availability → 503 if missing
    // 2. Parse status string to JobStatus (case-insensitive, None if absent)
    // 3. Parse before ISO-8601 string to DateTime<Utc> (None if absent)
    // 4. Compute effective limit: query.limit.unwrap_or(100).clamp(1, 1000)
    // 5. Call job_store::list_jobs(pool, parsed_status, effective_limit, parsed_before)
    // 6. Return 200 with JSON array, or 500 on error
}
```

Key implementation details:
- `status` is received as `Option<String>` from axum's Query extractor; parse it to `JobStatus` by matching `"Queued"`, `"Running"`, `"Completed"`, `"Failed"`, `"Cancelled"` (case-insensitive via `.to_lowercase()`). If the string doesn't match any variant, treat as `None` (return all statuses) with a WARN log.
- `limit` is clamped: `query.limit.unwrap_or(100).clamp(1, 1000)`. The max of 1000 matches `LimitsConfig::list_max_limit` default.
- `before` is parsed from ISO 8601 string using `chrono::DateTime::parse_from_rfc3339`, converted to UTC via `.into()`. If parsing fails, treat as `None` with a WARN log.
- Reuse the existing `error_body` helper for error responses.

### Step 2 — Wire route in `lib.rs`

Change the existing single-method route:

```rust
// Before (P12-A4):
.route("/v1/jobs", post(handlers::jobs::submit_job))

// After (P12-A5):
.route("/v1/jobs", get(handlers::jobs::list_jobs).post(handlers::jobs::submit_job))
```

This uses axum's multi-method routing on a single path.

### Step 3 — Add tests in `handlers/jobs.rs`

Add three new test functions under the existing `#[cfg(test)] mod tests`:

1. **`list_jobs_returns_all_submitted_jobs`** — Submit 2 valid ZiT graphs via POST, then GET /v1/jobs; assert response status 200 and body array length == 2.

2. **`list_jobs_filters_by_status`** — Submit 2 jobs (both Queued), then GET /v1/jobs?status=queued; assert length == 2. Then update one to Running via DB (or submit another path that changes status); verify filtered count changes accordingly. For MVP simplicity: submit 2 Queued jobs, filter by `?status=queued` → 2 results.

3. **`list_jobs_limit_clamps_to_one`** — Submit 2 jobs, GET /v1/jobs?limit=1; assert array length == 1.

Tests use the existing `build_test_app()` fixture and `make_valid_zit_graph()` helper.

### Step 4 — Version bump

Bump `anvilml-server` crate patch version from `0.1.2` to `0.1.3` in `crates/anvilml-server/Cargo.toml`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Add `list_jobs` handler function + 3 unit tests |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire `GET /v1/jobs` route alongside existing POST |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version `0.1.2 → 0.1.3` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/src/handlers/jobs.rs` | `list_jobs_returns_all_submitted_jobs` | GET /v1/jobs returns all submitted jobs (POST 2 → GET lists 2) |
| `crates/anvilml-server/src/handlers/jobs.rs` | `list_jobs_filters_by_status` | ?status=queued filters to matching jobs only |
| `crates/anvilml-server/src/handlers/jobs.rs` | `list_jobs_limit_clamps_to_one` | ?limit=1 returns exactly one job from the DB |

## CI Impact

No CI workflow files are modified. The task stays within existing test infrastructure (`cargo test --workspace --features mock-hardware`). No new gates or jobs are needed. The OpenAPI drift gate may need to pass since a new endpoint with `utoipa::path` annotations is added, but that gate is already in the project's CI checklist and runs on handler/schema changes.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `chrono::DateTime<Utc>` parsing from query string fails for non-ISO formats | Medium | Low (treated as None with WARN log) | Graceful fallback: invalid `before` values are silently ignored, returning all jobs |
| Query parameter name collision with existing route | Low | Medium (404 or wrong handler) | Use axum's multi-method `.get().post()` on same path — well-documented pattern |
| `status` query param case sensitivity mismatch | Medium | Low (client must use correct casing) | Parse case-insensitively; if unknown, treat as `None` and warn |
| Missing `serde::Deserialize` derive for `ListJobsQuery` | Low | Build failure | Add `#[derive(serde::Deserialize, Default)]` to the struct |

## Acceptance Criteria

- [ ] `GET /v1/jobs` returns 200 with JSON array of all jobs
- [ ] `GET /v1/jobs?status=queued` returns only queued jobs (filtered)
- [ ] `GET /v1/jobs?limit=1` returns exactly one job
- [ ] `cargo test -p anvilml-server -- features mock-hardware` exits 0 with all tests passing
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] Crate version bumped to 0.1.3 in `crates/anvilml-server/Cargo.toml`
