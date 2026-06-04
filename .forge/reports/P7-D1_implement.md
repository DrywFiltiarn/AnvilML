# Implementation Report: P7-D1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-D1                                              |
| Phase       | 007 — WebSocket Event Stream                       |
| Description | anvilml-registry: fix db::open to create missing database file |
| Implemented | 2026-06-04T23:15:00Z                               |
| Status      | COMPLETE                                           |

## Summary

Replaced `SqlitePoolOptions::connect(path_str)` in `db::open` with `SqliteConnectOptions::new().filename(path).create_if_missing(true).connect_with(opts)`, which causes SQLite to automatically create the database file when it does not exist. This eliminates the first-run panic. Removed the corresponding `fs::File::create` pre-creation workaround from the server integration tests in `api_models.rs`. Added a unit test `test_open_creates_file_if_missing` that verifies file creation and pool usability on a fresh, non-existent path.

## Resolved Dependencies

No new dependencies added or modified. The `SqliteConnectOptions` type is part of the existing `sqlx 0.9.0` dependency (already present in the workspace). API shape confirmed via existing lockfile usage; no MCP lookup required for version resolution.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/db.rs` | Added `SqliteConnectOptions` to imports; replaced `connect()` call with `connect_with(opts)` using `create_if_missing(true)`; added `test_open_creates_file_if_missing` test |
| Modify | `crates/anvilml-server/tests/api_models.rs` | Removed pre-create db file workaround (comment + `fs::File::create` call); `use std::fs` retained (still used for `create_dir_all` and model file creation) |

## Commit Log

```
 .forge/reports/P7-D1_plan.md              | 108 ++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  13 ++--
 crates/anvilml-registry/src/db.rs         |  41 +++++++++---
 crates/anvilml-server/tests/api_models.rs |   2 -
 5 files changed, 150 insertions(+), 20 deletions(-)
```

## Test Results

```
cargo test --workspace --features mock-hardware

anvilml_core          74 passed; 0 failed
anvilml_hardware      59 passed; 0 failed
anvilml_ipc            0 passed; 0 failed
anvilml_openapi        0 passed; 0 failed
anvilml_registry (lib) 11 passed; 0 failed  ← includes new test_open_creates_file_if_missing
anvilml_registry (db integration) 1 passed; 0 failed
anvilml_registry (rescan) 2 passed; 0 failed
anvilml_registry (scanner) 1 passed; 0 failed
anvilml_registry (store_get) 2 passed; 0 failed
anvilml_registry (store_list) 3 passed; 0 failed
anvilml_scheduler      0 passed; 0 failed
anvilml_server (lib)   8 passed; 0 failed
anvilml_server (api_models integration) 3 passed; 0 failed  ← pre-create workaround removed, all pass
anvilml_server (ws_events integration) 1 passed; 0 failed
anvilml_worker         0 passed; 0 failed
anvilml binary        8 passed; 0 failed
config_reference       1 passed; 0 failed
Doc-tests              2 passed; 0 failed

Total: 154 passed; 0 failed
```

## Platform Cross-Check

**1. Mock-hardware Windows-gnu cross-check:**
```
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.41s
```

**2. Real-hardware Linux native:**
```
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.06s
```

**3. Real-hardware Windows-gnu cross-check:**
```
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.06s
```

All three checks exit 0.

## Project Gates

**Config Surface Sync (Gate 1):**
```
cargo test -p backend --features mock-hardware -- test_toml_key_set_matches_default

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

No OpenAPI drift gate required — no handler signatures or `utoipa` annotations modified.

## Deviations from Plan

- **Test assertion refinement:** The original plan specified asserting `path.exists()` after open, which was implemented as planned. An additional verification was added to confirm the three expected tables exist in the freshly created database. The initial assertion checked total table count (`SELECT COUNT(*) FROM sqlite_master WHERE type='table'`) and found 4 instead of 3 (extra migration system table). Changed to filter by known table names (`IN ('jobs','models','artifacts')`) matching the pattern used in `test_open_creates_tables`.
- **`use std::fs` retained:** The plan suggested removing it if unused. It remains because lines 28–29 still use `fs::create_dir_all` and `fs::File::create` for model directory setup.

## Blockers

None.
