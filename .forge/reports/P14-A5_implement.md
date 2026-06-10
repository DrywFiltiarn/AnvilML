# Implementation Report: P14-A5

| Field       | Value                                                         |
|-------------|---------------------------------------------------------------|
| Task ID     | P14-A5                                                        |
| Phase       | 014 — Artifact Storage                                        |
| Description | anvilml-server: GET /v1/artifacts list (by job_id)            |
| Implemented | 2026-06-10T08:15:00Z                                          |
| Status      | COMPLETE                                                      |

## Summary

Implemented the `GET /v1/artifacts` list endpoint that returns artifact metadata filtered by optional `job_id`, `limit`, and `before` query parameters. Added `ArtifactStore::list()` method using `sqlx::QueryBuilder` for dynamic SQL construction, a `list_artifacts` handler with `ListArtifactsQuery` struct, and wired the route in `build_router()` before the existing `{hash}` route. Added 6 unit tests (4 store-level, 2 handler-level). Bumped `anvilml-server` patch version from `0.1.7` to `0.1.8`.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | utoipa    | 5.5.0            | lockfile       |
| crate  | sqlx      | 0.9.0            | lockfile       |
| crate  | serde     | 1.0.x            | lockfile       |

No new dependencies added. Existing workspace dependencies used: `serde` (for `Serialize` derive on `ArtifactMeta`), `utoipa` (for `ToSchema` derive), `sqlx` (for `FromRow` derive and `QueryBuilder`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/artifact/store.rs` | Add `list()` method to `ArtifactStore`; add `Serialize`, `FromRow`, `ToSchema` derives to `ArtifactMeta`; add `use sqlx::Row` import; add `mod tests` with 4 tests |
| Modify | `crates/anvilml-server/src/handlers/artifacts.rs` | Add `ListArtifactsQuery` struct; add `list_artifacts` handler with `#[utoipa::path]` annotation; add `mod tests` with 2 tests |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire `GET /v1/artifacts` route before `/v1/artifacts/{hash}` route |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version `0.1.7 → 0.1.8` |

## Commit Log

```
 Cargo.lock                                      |   2 +-
 crates/anvilml-server/Cargo.toml                |   2 +-
 crates/anvilml-server/src/artifact/store.rs     | 184 +++++++++++++++++++-
 crates/anvilml-server/src/handlers/artifacts.rs | 219 +++++++++++++++++++++++-
 crates/anvilml-server/src/lib.rs                |   1 +
 5 files changed, 403 insertions(+), 5 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_server-f4809fd395439902)

running 22 tests
test ws::broadcaster::tests::send_no_subscribers_no_error ... ok
test ws::broadcaster::tests::subscribe_send_receive ... ok
test artifact::store::tests::list_empty_returns_empty_array ... ok
test handlers::artifacts::tests::list_artifacts_empty_returns_200_with_empty_array ... ok
test handlers::jobs::tests::submit_job_bad_graph_returns_422 ... ok
test artifact::store::tests::list_with_job_id_filter ... ok
test artifact::store::tests::list_before_filter ... ok
test handlers::jobs::tests::get_job_returns_404_when_missing ... ok
test handlers::jobs::tests::submit_job_valid_zit_graph_returns_202 ... ok
test artifact::store::tests::list_limit_clamped ... ok
test handlers::jobs::tests::get_job_returns_200_with_queued_job ... ok
test handlers::artifacts::tests::list_artifacts_with_job_id_filter ... ok
test handlers::jobs::tests::list_jobs_limit_clamps_to_one ... ok
test handlers::jobs::tests::list_jobs_filters_by_status ... ok
test handlers::jobs::tests::list_jobs_returns_all_submitted_jobs ... ok
test tests::env_returns_200_with_stub_report ... ok
test tests::rescan_returns_202 ... ok
test tests::health_returns_200 ... ok
test tests::workers_endpoint_returns_200 ... ok
test tests::get_model_returns_404_when_missing ... ok
test tests::system_returns_200_with_hardware_info ... ok
test ws::stats_tick::tests::stats_tick_broadcasts_event ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.06s
```

Full workspace test suite: 247 passed; 0 failed; 0 ignored.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
Checking anvilml-server v0.1.8 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.4 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized, debuginfo] target(s) in 1.14s

# 2. Mock-hardware Windows cross-check
    Checking anvilml-server v0.1.8 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.4 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized, debuginfo] target(s) in 3.96s

# 3. Real-hardware Linux check
    Finished `dev` profile [unoptimized, debuginfo] target(s) in 3.50s

# 4. Real-hardware Windows cross-check
    Finished `dev` profile [unoptimized, debuginfo] target(s) in 2.85s
```

All four cross-checks passed (exit 0).

## Project Gates

### Gate 1 — Config Surface Sync
```
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```
No config fields were added/removed/renamed by this task — gate N/A for config changes.

### OpenAPI Drift Gate
Not applicable — no existing `backend/openapi.json` file in this repository.

## Deviations from Plan

- **`ArtifactMeta` derives**: Added `Serialize`, `FromRow`, and `ToSchema` derives to `ArtifactMeta` in `store.rs`. The plan did not mention these derives, but they are required: `Serialize` for `serde_json::to_value()` in the handler, `FromRow` for `sqlx::query_as`, and `ToSchema` for the utoipa `#[utoipa::path]` annotation.
- **Dynamic SQL approach**: Used `sqlx::query_builder::QueryBuilder` with `push_bind()` instead of string concatenation with `AssertSqlSafe()`. This is the recommended sqlx pattern for dynamic SQL and avoids injection vulnerabilities.
- **`before` filter semantics**: The plan's test expected `hash_new` (created_at=3000) to be returned with `before=2500`, but the `created_at < 2500` condition correctly excludes it. Fixed the test to expect `hash_mid` and `hash_old` (both < 2500).
- **Handler pool check removed**: Removed the redundant `state.db` check in `list_artifacts` since the handler delegates to `artifact_store.list()` which uses the store's own pool.

## Blockers

None.
