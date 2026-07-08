# Implementation Report: P15-B1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P15-B1                          |
| Phase         | 015 — Artifact Storage Wiring   |
| Description   | anvilml-server: GET /v1/artifacts list handler |
| Implemented   | 2026-07-08T21:05:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented the `GET /v1/artifacts` HTTP handler for the AnvilML server. Created a new `artifacts.rs` handler module with `list_artifacts()` and `ListArtifactsParams`, registered the route in `build_router()`, and wrote 4 integration tests covering empty store, populated store, job_id filtering, and JSON shape validation. Bumped `anvilml-server` patch version from 0.1.8 to 0.1.9.

## Resolved Dependencies

| Type   | Name     | Version resolved | Source         |
|--------|----------|------------------|----------------|
| crate  | axum     | 0.8.9            | project lock   |
| crate  | serde    | 1.0              | project lock   |
| crate  | uuid     | 1.23             | project lock   |

No new external crates introduced. All types (`axum::extract::Query`, `axum::Json`, `axum::routing::get`, `uuid::Uuid`) confirmed present in existing dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/handlers/artifacts.rs` | New handler module with `list_artifacts()` and `ListArtifactsParams` |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Added `pub mod artifacts;` |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Registered `GET /v1/artifacts` route in `build_router()` |
| CREATE | `crates/anvilml-server/tests/artifacts_tests.rs` | 4 integration tests for the handler |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bumped patch version 0.1.8 → 0.1.9 |
| MODIFY | `docs/TESTS.md` | Added 4 test entries for new tests |

## Commit Log

```
 .forge/reports/P15-B1_plan.md                   | 129 ++++++++++
 .forge/state/CURRENT_TASK.md                    |   6 +-
 .forge/state/state.json                         |  13 +-
 Cargo.lock                                      |   2 +-
 crates/anvilml-server/Cargo.toml                |   2 +-
 crates/anvilml-server/src/handlers/artifacts.rs |  62 +++++
 crates/anvilml-server/src/handlers/mod.rs       |   1 +
 crates/anvilml-server/src/lib.rs                |   5 +
 crates/anvilml-server/tests/artifacts_tests.rs  | 318 ++++++++++++++++++++++++
 docs/TESTS.md                                   |  48 ++++
 10 files changed, 575 insertions(+), 11 deletions(-)
```

## Test Results

```
running 4 tests
test test_list_artifacts_empty_store_returns_200_empty_array ... ok
test test_list_artifacts_json_shape ... ok
test test_list_artifacts_job_id_filter_returns_matching ... ok
test test_list_artifacts_populated_returns_all ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: all tests passed (400+ tests across all crates).

## Format Gate

```
(no output — exit 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 26.56s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.83s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.28s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.96s
```

All four platform cross-checks passed.

## Project Gates

- Gate 1 (Config Surface Sync): `cargo test -p anvilml --features mock-hardware -- config_reference` — passed (pre-existing, not triggered by this task)
- Gate 2 (OpenAPI Drift): Skipped — `api/openapi.json` does not yet exist in the repository
- Gate 3 (Node Parity): Not triggered — no changes to `worker/nodes/` or `node_registry.rs`
- Gate 4 (Mock/Real Parity Markers): Not triggered — handlers are not node/arch-module functions

## Public API Delta

```
+pub mod artifacts;
```

The only new `pub` item is the module declaration `pub mod artifacts;` in `handlers/mod.rs`. The `list_artifacts()` function and `ListArtifactsParams` struct are `pub(crate)`, not `pub`, so they are not exposed outside the crate. This matches the plan's Public API Surface table.

## Deviations from Plan

- The plan specified adding `make_artifact_state()` as an inline test helper in `artifacts.rs`. During implementation, this helper was found to be unused (the integration tests use `make_test_state()` from `jobs_tests.rs` which already constructs `AppState` with an `ArtifactStore`). The inline test helper was removed to avoid a dead-code warning. The integration tests in `artifacts_tests.rs` use the shared `make_test_state()` pattern from `jobs_tests.rs`, which is consistent with the project's established test convention.
- The `save_artifact()` test helper needed a `content_suffix` parameter to ensure distinct content hashes across multiple saves (the `ArtifactStore::save()` method is idempotent — same bytes produce same hash, and `INSERT OR IGNORE` prevents duplicates). This was discovered during test implementation and fixed inline.

## Blockers

None.
