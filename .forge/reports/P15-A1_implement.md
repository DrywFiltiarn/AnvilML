# Implementation Report: P15-A1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P15-A1                             |
| Phase         | 15 — Artifact Storage Wiring       |
| Description   | anvilml-server: AppState gains artifact_store field |
| Implemented   | 2026-07-08T20:15:00Z               |
| Status        | COMPLETE                           |

## Summary

Added `artifact_store: Arc<ArtifactStore>` as a new field on `AppState` in `crates/anvilml-server/src/state.rs`, wired its construction in `backend/src/main.rs` using `config.artifact_dir` and the shared `SqlitePool`, and added two new tests (`test_app_state_artifact_store_constructs`, `test_app_state_artifact_store_clone_shares`) in `crates/anvilml-server/tests/state_tests.rs`. Also updated all existing test files that construct `AppState` directly (health_tests.rs, nodes_tests.rs, jobs_tests.rs) to include the new field, and bumped the `anvilml-server` crate version from 0.1.7 to 0.1.8.

## Resolved Dependencies

| Type   | Name              | Version resolved | Source           |
|--------|-------------------|------------------|------------------|
| crate  | anvilml-artifacts | 0.1.3 (workspace) | N/A (internal)  |

No new external dependencies were introduced. `anvilml-artifacts` was already a declared path dependency of `anvilml-server`. The `backend` crate gained `anvilml-artifacts` as a dependency (added to `backend/Cargo.toml`) to satisfy the `use anvilml_artifacts::ArtifactStore` import in `main.rs`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-server/src/state.rs` | Added `artifact_store: Arc<ArtifactStore>` field with doc comment; added `use anvilml_artifacts::ArtifactStore` import |
| MODIFY | `backend/src/main.rs` | Added `use anvilml_artifacts::ArtifactStore` import; constructed `ArtifactStore` from `config.artifact_dir` and `pool.clone()`; added `artifact_store` to `AppState` struct literal |
| MODIFY | `backend/Cargo.toml` | Added `anvilml-artifacts` path dependency |
| MODIFY | `crates/anvilml-server/tests/state_tests.rs` | Added two new tests; updated `make_full_state()` to accept `Arc<ArtifactStore>`; updated all 4 existing callers; added `create_test_artifact_store()` helper; updated `test_app_state_constructs` direct construction |
| MODIFY | `crates/anvilml-server/tests/health_tests.rs` | Added `ArtifactStore` import and `artifact_store` field to `make_test_state()` |
| MODIFY | `crates/anvilml-server/tests/nodes_tests.rs` | Added `ArtifactStore` import and `artifact_store` field to `make_test_state()` |
| MODIFY | `crates/anvilml-server/tests/jobs_tests.rs` | Added `ArtifactStore` import and `artifact_store` field to `make_test_state()` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bumped version 0.1.7 → 0.1.8 |
| MODIFY | `docs/TESTS.md` | Added entries for `test_app_state_artifact_store_constructs` and `test_app_state_artifact_store_clone_shares` |

## Commit Log

```
 .forge/state/CURRENT_TASK.md                |  6 +-
 .forge/state/state.json                     | 13 +++--
 Cargo.lock                                  |  3 +-
 backend/Cargo.toml                          |  1 +
 backend/src/main.rs                         | 14 ++++-
 crates/anvilml-server/Cargo.toml            |  2 +-
 crates/anvilml-server/src/state.rs          |  9 +++
 crates/anvilml-server/tests/health_tests.rs |  9 +++
 crates/anvilml-server/tests/jobs_tests.rs   |  9 +++
 crates/anvilml-server/tests/nodes_tests.rs  |  9 +++
 crates/anvilml-server/tests/state_tests.rs  | 90 ++++++++++++++++++++++++++---
 docs/TESTS.md                               | 24 ++++++++
 12 files changed, 170 insertions(+), 19 deletions(-)
```

## Test Results

```
running 7 tests in state_tests:
test test_app_state_artifact_store_clone_shares ... ok
test test_app_state_clone_preserves_all_fields ... ok
test test_app_state_with_new_fields ... ok
test test_app_state_artifact_store_constructs ... ok
test test_app_state_clone_shares_node_registry ... ok
test test_app_state_scheduler_arc_sharing ... ok
test test_app_state_constructs ... ok

Full workspace: 315 tests passed, 0 failed
```

All 315 workspace tests pass, including the 2 new tests and all 7 existing tests in `state_tests.rs`.

## Format Gate

```
(cargo fmt --all -- --check exits 0 — no output, no drift)
```

## Platform Cross-Check

```
1. Mock-hardware Linux:  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s
2. Mock-hardware Windows: Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.93s
3. Real-hardware Linux:   Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.95s
4. Real-hardware Windows: Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.53s
```

All four platform cross-checks exit 0.

## Project Gates

```
Gate 1 (config_reference): running 1 test — test tests::config_reference_matches_defaults ... ok
```

Gate 2 (OpenAPI drift) is not triggered — no handler signature changes or response schema modifications.

## Public API Delta

```
+    pub artifact_store: Arc<ArtifactStore>,
```

One new pub field: `AppState::artifact_store` of type `Arc<ArtifactStore>` in module `anvilml_server::state`. No new pub functions, structs, or traits.

## Deviations from Plan

- **Additional dependency**: Added `anvilml-artifacts` as a dependency to `backend/Cargo.toml`. The approved plan did not list this file, but it was required because `main.rs` uses `use anvilml_artifacts::ArtifactStore` and the `backend` crate did not previously declare the dependency.
- **Additional test file modifications**: Updated `health_tests.rs`, `nodes_tests.rs`, and `jobs_tests.rs` to include the `artifact_store` field in their `make_test_state()` helpers. These files construct `AppState` directly and would fail to compile without the new field.
- **Helper function signature change**: Made `create_test_artifact_store()` async (was initially synchronous) to avoid "Cannot start a runtime from within a runtime" panics when called from `#[tokio::test]` async tests.

## Blockers

None.
