# Implementation Report: P18-E1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P18-E1                          |
| Phase         | 18 — HTTP/WebSocket Server Completion |
| Description   | anvilml-server: DELETE /v1/jobs/:id single-job delete handler |
| Implemented   | 2026-07-12T15:30:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented the `DELETE /v1/jobs/:id` handler that deletes a single terminal-status job (Completed/Failed/Cancelled) along with its associated artifacts. Added `JobStore::delete()` and `ArtifactStore::delete()` persistence methods, registered the DELETE route in `build_router()`, and added 5 integration tests to `jobs_tests.rs`. All 20 tests in the jobs test suite pass, clippy is clean, format checks pass, and all platform cross-checks and project gates succeed.

## Resolved Dependencies

| Type   | Name  | Version resolved | Source         |
|--------|-------|------------------|----------------|
| crate  | sqlx  | from Cargo.lock  | lockfile       |
| crate  | uuid  | from Cargo.lock  | lockfile       |

No new external dependencies introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-registry/src/job_store.rs` | Added `pub async fn delete(&self, id: Uuid)` method |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.8 → 0.1.9 |
| MODIFY | `crates/anvilml-artifacts/src/store.rs` | Added `pub async fn delete(&self, hash: &str)` method and `pub fn artifact_dir()` accessor |
| MODIFY | `crates/anvilml-artifacts/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |
| MODIFY | `crates/anvilml-server/src/handlers/jobs.rs` | Added `pub(crate) async fn delete_job()` handler |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Added `.delete(delete_job)` to `/v1/jobs/{id}` route |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.24 → 0.1.25 |
| MODIFY | `crates/anvilml-server/tests/jobs_tests.rs` | Added 5 new integration tests for delete_job |
| MODIFY | `docs/TESTS.md` | Added 5 new test catalogue entries |

## Commit Log

```
 .forge/reports/P18-E1_plan.md              | 160 ++++++++++++++
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  16 +-
 Cargo.lock                                 |   6 +-
 crates/anvilml-artifacts/Cargo.toml        |   2 +-
 crates/anvilml-artifacts/src/store.rs      |  51 +++++
 crates/anvilml-registry/Cargo.toml         |   2 +-
 crates/anvilml-registry/src/job_store.rs   |  27 +++
 crates/anvilml-server/Cargo.toml           |   2 +-
 crates/anvilml-server/src/handlers/jobs.rs |  89 ++++++++
 crates/anvilml-server/src/lib.rs           |   5 +-
 crates/anvilml-server/tests/jobs_tests.rs  | 343 +++++++++++++++++++++++++++++
 docs/TESTS.md                              |  60 +++++
 13 files changed, 751 insertions(+), 18 deletions(-)
```

## Test Results

```
     Running tests/jobs_tests.rs (target/debug/deps/jobs_tests-3c92c791b9a4cdb4)

running 20 tests
test test_cancel_unknown_id_returns_404 ... ok
test test_cancel_completed_job_returns_409 ... ok
test test_get_job_unknown_returns_404 ... ok
test test_delete_non_terminal_queued_returns_409 ... ok
test test_cancel_queued_job_returns_202 ... ok
test test_get_job_existing_returns_200 ... ok
test test_cancel_running_job_returns_202 ... ok
test test_cancel_already_cancelled_job_returns_409 ... ok
test test_delete_non_terminal_running_returns_409 ... ok
test test_delete_unknown_id_returns_404 ... ok
test test_delete_terminal_job_returns_204 ... ok
test test_list_jobs_before_param_accepted ... ok
test test_list_jobs_limit ... ok
test test_delete_terminal_job_removes_artifacts ... ok
test test_submit_job_empty_registry_returns_503 ... ok
test test_submit_job_invalid_graph_returns_400 ... ok
test test_submit_job_malformed_body_returns_400 ... ok
test test_list_jobs_no_filter_returns_all ... ok
test test_submit_job_valid_returns_202 ... ok
test test_list_jobs_status_filter ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: all crates pass (170+ tests total, 0 failures).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
cargo check --workspace --features mock-hardware
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.74s

# 2. Mock-hardware Windows:
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.74s

# 3. Real-hardware Linux:
cargo check --bin anvilml
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.03s

# 4. Real-hardware Windows:
cargo check --bin anvilml --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.52s
```

All four checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync:
cargo test -p anvilml --features mock-hardware -- config_reference
→ test tests::config_reference_matches_defaults ... ok
→ test result: ok. 1 passed; 0 failed
```

Gate 2 (OpenAPI Drift) is not triggered by this task — it only modifies handler signatures for an existing route, not new response types or schema derives.

## Public API Delta

```
+    pub fn artifact_dir(&self) -> &PathBuf {
+    pub async fn delete(&self, hash: &str) -> Result<(), AnvilError> {
+    pub async fn delete(&self, id: Uuid) -> Result<(), AnvilError> {
```

New public items:
1. `ArtifactStore::artifact_dir()` — `pub fn artifact_dir(&self) -> &PathBuf` (read-only accessor for tests)
2. `ArtifactStore::delete()` — `pub async fn delete(&self, hash: &str) -> Result<(), AnvilError>`
3. `JobStore::delete()` — `pub async fn delete(&self, id: Uuid) -> Result<(), AnvilError>`

The `delete_job` handler is `pub(crate)`, not `pub`, so it does not appear in the public API.

## Deviations from Plan

1. **`JobStatus` display format in tracing log**: The plan specified `status = %job.status` in the tracing debug call, but `JobStatus` does not implement `Display`. Resolved by serializing to snake_case text via `serde_json::to_string` and trimming quotes, matching the pattern used throughout the codebase for `JobStatus` serialization.

2. **`ArtifactStore::artifact_dir()` accessor**: Added a new public accessor `pub fn artifact_dir(&self) -> &PathBuf` to support the integration test that verifies artifact file deletion from disk. This was not in the original plan but is needed because `artifact_dir` is a private field on `ArtifactStore`.

3. **Test state cloning**: Two tests (`test_delete_terminal_job_returns_204` and `test_delete_terminal_job_removes_artifacts`) required `state.clone()` before passing to `build_router()` because the original `AppState` must be retained to access `state.db` or `state.artifact_store` after the router consumes the state. This is the same pattern used in the existing `test_cancel_running_job_returns_202` test.

## Blockers

None.
