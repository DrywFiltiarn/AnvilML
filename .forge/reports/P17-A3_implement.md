# Implementation Report: P17-A3

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P17-A3                          |
| Phase         | 017 — Job & Artifact Management |
| Description   | anvilml: integration test for job + artifact deletion |
| Implemented   | 2026-06-10T17:30:00Z            |
| Status        | COMPLETE                          |

## Summary

Created `backend/tests/api_delete.rs` with 5 integration tests covering the full job and artifact deletion lifecycle: single delete with artifact removal (204 + 404), running job rejection (409), bulk delete all terminal jobs (3 removed, Running preserved), bulk delete by specific status (1 removed, Failed preserved), and nonexistent job rejection (404). All tests use `open_in_memory()` for isolated DB pools, `tempfile::tempdir()` for isolated artifact directories, `#[serial]` for deterministic ordering, and `temp_env::async_with_vars` for env var scoping.

## Resolved Dependencies

| Type   | Name     | Version resolved | Source         |
|--------|----------|------------------|----------------|
| crate  | serial_test | (workspace)  | lockfile       |
| crate  | temp-env | 0.3              | lockfile       |
| crate  | tempfile | (workspace)      | lockfile       |

No new dependencies were added. All dependencies are pre-existing dev-dependencies in `backend/Cargo.toml`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `backend/tests/api_delete.rs` | Integration test file with 5 tests for job + artifact deletion |
| Modify | `backend/Cargo.toml` | Bump patch version `0.1.6 → 0.1.7` (test file added to backend crate) |

No production source files are modified.

## Commit Log

```
 .forge/reports/P17-A3_plan.md | 156 +++++++++
 .forge/state/CURRENT_TASK.md  |   6 +-
 .forge/state/state.json       |  13 +-
 Cargo.lock                    |   2 +-
 backend/Cargo.toml            |   2 +-
 backend/tests/api_delete.rs   | 752 ++++++++++++++++++++++++++++++++++++++++++
 6 files changed, 920 insertions(+), 11 deletions(-)
```

## Test Results

```
running 5 tests
test bulk_delete_by_status_removes_only_matching ... ok
test delete_completed_job_removes_artifact_and_row ... ok
test bulk_delete_all_terminal_jobs ... ok
test delete_nonexistent_job_returns_404 ... ok
test delete_running_job_returns_409 ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.62s
```

Full workspace test suite: 256 tests, 0 failures.

## Format Gate

```
(no output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s

# 2. Mock-hardware Windows cross-check
Checking anvilml-scheduler v0.1.17 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.11 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.6 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.98s

# 3. Real-hardware Linux check
Checking anvilml-scheduler v0.1.17 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.11 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.6 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.67s

# 4. Real-hardware Windows cross-check
Checking anvilml-scheduler v0.1.17 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.11 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.6 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.12s
```

All four checks exit 0.

## Project Gates

```
Gate 1 — Config Surface Sync:
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

## Deviations from Plan

- **Version bump added**: The plan's "Files Affected" table only listed the test file. Per FORGE_AGENT_RULES §12.1, test files in a crate trigger a patch version bump. Bumped `backend/Cargo.toml` from `0.1.6` to `0.1.7`.
- **`build_test_app()` returns artifact directory**: The plan's `build_test_app()` was described as returning `(App, SqlitePool, Arc<JobScheduler>, Arc<WorkerPool>, Arc<EventBroadcaster>)`. The actual implementation also returns the `PathBuf` for the artifact directory, enabling tests to construct artifact file paths for verification.
- **Added `BodyExt` import**: The `hyper::body::Incoming::collect()` method requires the `http_body_util::BodyExt` trait to be in scope. Added `use http_body_util::BodyExt;` to the imports.
- **Cloned `artifact_store` before scheduler**: The `ArtifactStore` must be cloned before passing to `JobScheduler` (it's moved into the scheduler), then passed again to `App::new`.

## Blockers

None.
