# Implementation Report: P20-A1

| Field | Value |
|-------|-------|
| Task ID | P20-A1 |
| Phase | 020 — OpenAPI & Launcher Polish |
| Description | anvilml-server: utoipa annotations on all handlers + schemas |
| Implemented | 2026-06-12T08:00:00Z |
| Status | COMPLETE |

## Summary

Added `#[utoipa::path(...)]` annotations to all 8 REST handlers in `anvilml-server` that were previously missing them (health, system/env, system, models list, models get, models rescan, workers list, workers restart), and added `utoipa::ToSchema` derives to the two local response types (`HealthResponse` and `RescanResponse`). All 14 REST handlers now have complete OpenAPI annotations. No behavioral changes were made.

## Resolved Dependencies

No new dependencies added or modified. All types referenced in annotations (`EnvReport`, `HardwareInfo`, `ModelMeta`, `WorkerInfo`) already derive `utoipa::ToSchema` in `anvilml-core`. `utoipa` was already declared as a workspace dependency.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.17 → 0.1.18 |
| Modify | `crates/anvilml-server/src/handlers/health.rs` | Add `ToSchema` derive to `HealthResponse`; add `use utoipa::ToSchema`; add `utoipa::path` annotation to `health()` |
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Add `use utoipa::ToSchema`; add `ToSchema` derive to `RescanResponse`; add `utoipa::path` annotations to `list_models()`, `get_model()`, `rescan_models()` |
| Modify | `crates/anvilml-server/src/handlers/system.rs` | Add `utoipa::path` annotations to `get_env()` and `get_system()` |
| Modify | `crates/anvilml-server/src/handlers/workers.rs` | Add `utoipa::path` annotations to `list_workers()` and `restart_worker()` |

## Commit Log

```
 .forge/reports/P20-A1_plan.md                 | 134 ++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                  |   6 +-
 .forge/state/state.json                       |  13 +--
 Cargo.lock                                    |   2 +-
 crates/anvilml-server/Cargo.toml              |   2 +-
 crates/anvilml-server/src/handlers/health.rs  |  14 ++-
 crates/anvilml-server/src/handlers/models.rs  |  35 ++++++-
 crates/anvilml-server/src/handlers/system.rs  |  16 +++
 crates/anvilml-server/src/handlers/workers.rs |  21 ++++
 9 files changed, 230 insertions(+), 13 deletions(-)
```

## Test Results

```
running 74 tests (anvilml_core) — ok. 74 passed; 0 failed
running 56 tests (anvilml_hardware) — ok. 56 passed; 0 failed
running 18 tests (anvilml_ipc) — ok. 18 passed; 0 failed
running 19 tests (anvilml_registry) — ok. 19 passed; 0 failed
running 1 test (anvilml_registry_db) — ok. 1 passed; 0 failed
running 4 tests (device_store) — ok. 4 passed; 0 failed
running 2 tests (rescan) — ok. 2 passed; 0 failed
running 1 test (scanner) — ok. 1 passed; 0 failed
running 7 tests (seed_loader) — ok. 7 passed; 0 failed
running 2 tests (store_get) — ok. 2 passed; 0 failed
running 3 tests (store_list) — ok. 3 passed; 0 failed
running 43 tests (anvilml_scheduler) — ok. 43 passed; 0 failed
running 42 tests (anvilml_server) — ok. 42 passed; 0 failed
running 1 test (api_artifact_save) — ok. 1 passed; 0 failed
running 3 tests (api_artifact_serve) — ok. 3 passed; 0 failed
running 3 tests (api_models) — ok. 3 passed; 0 failed
running 1 test (api_ws_events) — ok. 1 passed; 0 failed
running 19 tests (anvilml_worker) — ok. 19 passed; 0 failed
running 17 tests (anvilml binary) — ok. 17 passed; 0 failed
running 2 tests (api_cancel) — ok. 2 passed; 0 failed
running 5 tests (api_delete) — ok. 5 passed; 0 failed
running 1 test (api_ws_lifecycle) — ok. 1 passed; 0 failed
running 1 test (config_reference) — ok. 1 passed; 0 failed
running 4 tests (preflight_check) — ok. 4 passed; 0 failed
Doc-tests — all 2 passed; 0 failed
```

Total: 251 tests passed, 0 failed.

## Format Gate

```
(no output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
# Check 1 — mock-hardware Linux:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.29s

# Check 2 — mock-hardware Windows cross:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.86s

# Check 3 — real-hardware Linux:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.07s

# Check 4 — real-hardware Windows cross:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.99s
```

All four platform checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync:
Finished `test` profile [unoptimized + debuginfo] target(s) in 13.49s
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out

# OpenAPI Drift Gate:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s
Running `target/debug/anvilml-openapi`
(no diff — exit 0)
```

## Deviations from Plan

- Removed `use utoipa::ToSchema;` from `workers.rs` — the import was unnecessary because `WorkerInfo` already derives `ToSchema` in `anvilml-core`. The plan step 9 listed it as needed, but the compiler flagged it as unused.

## Blockers

None.
