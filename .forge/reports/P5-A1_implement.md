# Implementation Report: P5-A1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P5-A1                              |
| Phase         | 005 — SQLite Persistence           |
| Description   | anvilml-registry: db.rs open/migrate/ghost-reset + initial SQL migration |
| Implemented   | 2026-06-15T14:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Created the initial SQLite migration (`database/migrations/001_initial.sql`) with five tables: `jobs`, `models`, `artifacts`, `seed_history`, and `device_capabilities`. Implemented `pub async fn open(path: &Path)` and `pub async fn open_in_memory()` in `crates/anvilml-registry/src/db.rs` that enable WAL mode, run compile-time migrations, and reset ghost jobs. Added five integration tests in `crates/anvilml-registry/tests/db_tests.rs` verifying file creation, WAL mode, in-memory pool, and ghost-job reset behavior. All 90 workspace tests pass.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | serial_test | 3.5            | Cargo.lock (MCP rust-docs unavailable) |
| crate  | tempfile  | 3.27.0           | Workspace (Cargo.lock) |
| crate  | tokio     | 1.52.3           | Workspace (Cargo.lock) |

`serial_test` version 3.5 taken from Cargo.lock (matching `anvilml-hardware`'s existing dev-dependency). `tempfile` and `tokio` are workspace dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `database/migrations/001_initial.sql` | SQL DDL for 5 tables with indexes |
| CREATE | `crates/anvilml-registry/src/db.rs` | `open()`, `open_in_memory()`, `run_migrations()`, `reset_ghost_jobs()` |
| CREATE | `crates/anvilml-registry/tests/db_tests.rs` | 5 integration tests |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Added `pub mod db;` and `pub use db::{open, open_in_memory};`; removed stub |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Bumped version 0.1.0→0.1.1; added dev-dependencies (serial_test, tempfile, tokio) |
| MODIFY | `docs/TESTS.md` | Added 5 test entries for new db_tests |

## Commit Log

```
 .forge/reports/P5-A1_plan.md              | 168 +++++++++++++++++++
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  13 +-
 Cargo.lock                                |   5 +-
 database/migrations/001_initial.sql        |  86 ++++++++++
 crates/anvilml-registry/Cargo.toml        |   7 +-
 crates/anvilml-registry/src/db.rs         | 168 +++++++++++++++++++
 crates/anvilml-registry/src/lib.rs        |   5 +-
 crates/anvilml-registry/tests/db_tests.rs | 264 ++++++++++++++++++++++++++++++
 docs/TESTS.md                             |  45 +++++
 10 files changed, 754 insertions(+), 13 deletions(-)
```

## Test Results

```
     Running tests/db_tests.rs (target/debug/deps/db_tests-e9e9180e834e21ac)

running 5 tests
test test_open_in_memory ... ok
test test_open_creates_file ... ok
test test_open_wal_mode ... ok
test test_ghost_job_noop ... ok
test test_ghost_job_reset ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace: 90 tests passed, 0 failed.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux (already verified in compile check)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.32s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.31s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.01s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.99s
```

All four cross-checks passed with zero errors.

## Project Gates

### Gate 1 — Config Surface Sync
```
running 1 test
test config_reference ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passed. No ServerConfig fields were added/removed by this task.

### Gate 2 — OpenAPI Drift
Not applicable — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields.

### Gate 3 — Node Parity
Not applicable — this task does not add, remove, or rename node types.

## Public API Delta

```
+pub mod db;
+pub use db::{open, open_in_memory};
```

New pub items:
- `pub mod db` — module (path: `anvilml_registry::db`)
- `pub async fn open(path: &Path) -> Result<SqlitePool, AnvilError>` — file-backed pool
- `pub async fn open_in_memory() -> Result<SqlitePool, AnvilError>` — in-memory pool

## Deviations from Plan

1. **Migration path**: The plan specified `sqlx::migrate!("./../../database/migrations")` (relative to src/). The `migrate!` macro actually resolves paths relative to `CARGO_MANIFEST_DIR` (the crate root, `crates/anvilml-registry/`), so the correct path is `"../../database/migrations"`. Verified by testing: `./../../database/migrations` from `CARGO_MANIFEST_DIR` resolves to `crates/database/migrations/` (wrong), while `../../database/migrations` resolves to `database/migrations/` (correct).

2. **sqlx 0.9.0 API differences**: The plan assumed a `MigrationReport` with `migrations()` method returned by `runner.run()`. In sqlx 0.9.0, `run()` returns `Result<(), MigrateError>` (unit type on success). The migration count is obtained from `runner.migrations.len()` before calling `run()`. Also, `MigrateError::NoMigration` variant does not exist in sqlx 0.9.0 — error handling was simplified to use `map_err(|e| AnvilError::Db(e.into()))`.

3. **Ghost-job test isolation**: The plan used `open_in_memory()` for ghost-job tests. In-memory databases are per-connection in SQLite — data inserted in one pool is not visible in a fresh pool. Changed to use file-backed pools in temp directories so the same database is accessible across pool connections.

4. **Table count in tests**: SQLite auto-creates `sqlite_sequence` table for `AUTOINCREMENT` columns (used by `artifacts.id`). Tests now expect 6 tables (5 user + sqlite_sequence) instead of 5.

5. **Removed `serial_test` dev-dependency usage**: The plan noted `serial_test` was not strictly needed since no tests mutate process-global state. The dependency was added to Cargo.toml for consistency with the workspace convention, but no `#[serial_test::serial]` annotations are used in this task's tests.

## Blockers

None.
