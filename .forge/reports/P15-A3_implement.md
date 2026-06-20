# Implementation Report: P15-A3

| Field         | Value                                                         |
|---------------|---------------------------------------------------------------|
| Task ID       | P15-A3                                                        |
| Phase         | 015 — Artifact Storage                                        |
| Description   | anvilml-server: GET /v1/artifacts and GET /v1/artifacts/:hash |
| Implemented   | 2026-06-20T17:00:00Z                                          |
| Status        | COMPLETE                                                      |

## Summary

Implemented two HTTP handlers for artifact access: `list_artifacts` (GET /v1/artifacts) returns artifact metadata as JSON, optionally filtered by job_id; `serve_artifact` (GET /v1/artifacts/:hash) serves raw PNG bytes with Content-Type: image/png. Added `ArtifactNotFound(String)` variant to `AnvilError` with 404 status mapping. Wired both routes into `build_router()`. Wrote 4 integration tests using the real TCP listener pattern from `handler_tests.rs`. Version bumped `anvilml-server` from 0.1.23 to 0.1.24.

## Resolved Dependencies

None. All types used (`axum::http::Response`, `axum::http::Body`, `axum::Json`, `axum::extract::{State, Path, Query}`, `anvilml_core::ArtifactMeta`, `anvilml_ipc::ArtifactStore`) are already present in the workspace. No new crates introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-core/src/error.rs` | Added `ArtifactNotFound(String)` variant, 404 status mapping, error kind `"artifact_not_found"`, updated doc comment count from 14 to 15 |
| CREATE | `crates/anvilml-server/src/handlers/artifacts.rs` | `list_artifacts` and `serve_artifact` handlers with utoipa ToSchema annotations, tracing instrumentation, doc comments |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Added `pub mod artifacts;` and `pub use artifacts::{list_artifacts, serve_artifact};` |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Added imports for artifact handlers, mounted `/v1/artifacts` and `/v1/artifacts/{hash}` routes in `build_router()` |
| CREATE | `crates/anvilml-server/tests/artifacts_tests.rs` | 4 integration tests: `test_list_artifacts_empty`, `test_list_artifacts_filtered`, `test_serve_artifact_returns_png`, `test_serve_artifact_not_found` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.23 → 0.1.24 |
| MODIFY | `docs/TESTS.md` | Added 5 new test entries (4 artifact handlers + 1 error variant) |

## Commit Log

```
 .forge/reports/P15-A3_plan.md                   | 152 +++++++++
 .forge/state/CURRENT_TASK.md                    |   6 +-
 .forge/state/state.json                         |  13 +-
 Cargo.lock                                      |   2 +-
 crates/anvilml-core/src/error.rs                |  17 +-
 crates/anvilml-server/Cargo.toml                |   2 +-
 crates/anvilml-server/src/handlers/artifacts.rs | 108 +++++++
 crates/anvilml-server/src/handlers/mod.rs       |   2 +
 crates/anvilml-server/src/lib.rs                |   7 +
 crates/anvilml-server/tests/artifacts_tests.rs  | 393 ++++++++++++++++++++++++
 docs/TESTS.md                                   |  45 ++++
 11 files changed, 733 insertions(+), 14 deletions(-)
```

## Test Results

```
     Running tests/artifacts_tests.rs (target/debug/deps/artifacts_tests-4c153ccaa5fcb99a)

running 4 tests
test test_list_artifacts_empty ... ok
test test_serve_artifact_not_found ... ok
test test_serve_artifact_returns_png ... ok
test test_list_artifacts_filtered ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: all 177 tests passed across all crates (0 failures).

## Format Gate

```
(No output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
# Check 1: Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.90s
--- CHECK 1 PASSED ---

# Check 2: Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.46s
--- CHECK 2 PASSED ---

# Check 3: Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.24s
--- CHECK 3 PASSED ---

# Check 4: Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.70s
--- CHECK 4 PASSED ---
```

## Project Gates

**Gate 1 — Config Surface Sync:**
```
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Gate 2 — OpenAPI Drift:**
```
(no diff — api/openapi.json is empty `{}` in current state; gate passes)
```

## Public API Delta

```
+pub mod artifacts;
+pub use artifacts::{list_artifacts, serve_artifact};
```

New `pub` items introduced:
- `pub mod artifacts` — module in `anvilml_server::handlers`
- `pub use artifacts::{list_artifacts, serve_artifact}` — re-exports in `anvilml_server::handlers`
- `pub async fn list_artifacts(State<AppState>, Query<ListArtifactsQuery>) -> Result<Json<Vec<ArtifactMeta>>, AnvilError>` — in `anvilml_server::handlers::artifacts`
- `pub async fn serve_artifact(State<AppState>, Path<String>) -> Result<Response<Body>, AnvilError>` — in `anvilml_server::handlers::artifacts`
- `pub struct ListArtifactsQuery { pub job_id: Option<Uuid> }` — in `anvilml_server::handlers::artifacts`
- `AnvilError::ArtifactNotFound(String)` — new enum variant in existing `pub enum AnvilError`

## Deviations from Plan

- **Binary body handling in tests:** The initial test implementation used `String::from_utf8_lossy` for body extraction, which corrupted PNG binary bytes (non-UTF-8 bytes were replaced with U+FFFD). Fixed by adding `extract_body_raw()` and `send_request_raw()` helper functions that operate on raw bytes. This is a deviation from the plan's test design but necessary for correctness — the plan assumed `send_request` would return accurate body bytes.
- **Arc cloning in tests:** `AppState::new()` takes ownership of the `Arc<ArtifactStore>`, so the test code had to clone the Arc before passing it to `AppState::new()` in order to save artifacts after construction. This is a structural fix required by the `AppState::new()` API signature.

## Blockers

None.
