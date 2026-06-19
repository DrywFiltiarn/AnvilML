# Implementation Report: P12-B1

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P12-B1                                            |
| Phase         | 012 — Graph Validation                            |
| Description   | POST /v1/jobs wired to validate_graph, 422 on invalid graph |
| Implemented   | 2026-06-19T20:15:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Wired the `POST /v1/jobs` endpoint in `anvilml-server` to call `validate_graph` from `anvilml-scheduler`. The handler checks if the node type registry is empty (no workers connected) and returns 503, calls `validate_graph` for semantic validation and returns 422 on failure, or returns 202 with a placeholder job ID on success. Also changed `AnvilError::InvalidGraph` HTTP status code from 400 to 422 per design spec §12.5. Created three integration tests covering all three response paths.

## Resolved Dependencies

| Type   | Name              | Version resolved | Source     |
|--------|-------------------|-----------------|------------|
| crate  | uuid              | 1.23.3          | Cargo.lock |

No new external dependencies. `uuid` was moved from `[dev-dependencies]` to `[dependencies]` in `anvilml-server/Cargo.toml` because the handler uses `Uuid::new_v4()` at runtime.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-core/src/error.rs` | Changed `InvalidGraph` status from 400 to 422, updated doc comments |
| MODIFY | `crates/anvilml-core/tests/error_tests.rs` | Updated assertion to expect 422 instead of 400 |
| CREATE | `crates/anvilml-server/src/handlers/jobs.rs` | New handler module with `submit_job` function |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Added `pub mod jobs` and `pub use jobs::submit_job` |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Imported `submit_job` and mounted `POST /v1/jobs` route |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Moved `uuid` from dev-deps to deps, bumped version 0.1.20 → 0.1.21 |
| CREATE | `crates/anvilml-server/tests/jobs_tests.rs` | Three integration tests for submit_job handler |
| MODIFY | `docs/TESTS.md` | Added entries for three new tests |

## Commit Log

```
 .forge/reports/P12-B1_plan.md              | 134 +++++++++++++++++++
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  13 +-
 Cargo.lock                                 |   2 +-
 crates/anvilml-core/src/error.rs           |  23 ++--
 crates/anvilml-core/tests/error_tests.rs   |   9 +-
 crates/anvilml-server/Cargo.toml           |   4 +-
 crates/anvilml-server/src/handlers/jobs.rs |  76 +++++++++++
 crates/anvilml-server/src/handlers/mod.rs  |   2 +
 crates/anvilml-server/src/lib.rs           |   5 +
 crates/anvilml-server/tests/jobs_tests.rs  | 203 +++++++++++++++++++++++++++++
 docs/TESTS.md                             |  34 ++++++
 12 files changed, 487 insertions(+), 24 deletions(-)
```

## Test Results

```
     Running tests/jobs_tests.rs (target/debug/deps/jobs_tests-42eca33994fc1b94)

running 3 tests
test test_submit_job_returns_503_when_no_workers ... ok
test test_submit_job_returns_422_with_unknown_node_type ... ok
test test_submit_job_returns_202_with_valid_graph ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace: 0 failures across all 14 test binaries.

## Format Gate

```
(cargo fmt --all -- --check returned exit 0 — no output)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.39s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.21s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.66s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.98s
```

All four cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
running 1 test
test config_reference ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
Not triggered — this task does not modify handler signatures, `#[utoipa::path]` annotations, or `ToSchema` derives. The new handler has no `#[utoipa::path]` annotation (deferred to a future task).

### Gate 3 — Node Parity
Not triggered — this task does not add, remove, or rename node types.

## Public API Delta

```
+pub mod jobs;
+pub use jobs::submit_job;
```

New public items:
- `pub mod jobs` — module declaration in `anvilml_server::handlers`
- `pub use jobs::submit_job` — re-export in `anvilml_server::handlers`
- `pub async fn submit_job` — handler function in `anvilml_server::handlers::jobs`

## Deviations from Plan

- **Request body format in tests:** The plan specified dispatching POST with an empty graph `{}`. However, `SubmitJobRequest` requires both `graph` and `settings` fields (no `#[serde(default)]` on either). The tests were adjusted to send `{"graph": {}, "settings": {}}`, `{"graph": {"nodes": [...]}, "settings": {}}` respectively. This is a test implementation detail — the API contract is unchanged.
- **Clippy `explicit_auto_deref` lint:** The plan specified `&*state.node_registry` for the `validate_graph` call. Clippy flagged this as `explicit-auto-deref` since the compiler auto-derefs `&Arc<T>` to `&T`. Changed to `&state.node_registry` with an inline comment explaining the auto-deref.
- **Removed `AnvilError` unused import from test file** after fixing the request body format.

## Blockers

None.
