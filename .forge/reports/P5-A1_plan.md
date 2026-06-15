# Plan Report: P5-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P5-A1                                       |
| Phase       | 005 — SQLite Persistence                    |
| Description | anvilml-registry: db.rs open/migrate/ghost-reset + initial SQL migration |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-06-15T13:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the initial SQLite migration SQL (`database/migrations/001_initial.sql`) with tables for jobs, models, artifacts, seed_history, and device_capabilities. Implement `pub async fn open(path: &Path) -> Result<SqlitePool>` in `crates/anvilml-registry/src/db.rs` that opens a file-backed SQLite connection with WAL mode enabled, runs `sqlx::migrate!` from the migrations directory, logs each migration applied (or an up-to-date message when none apply), and resets ghost jobs. Also implement `pub async fn open_in_memory() -> Result<SqlitePool>` for test use. The observable outcome: `cargo test -p anvilml-registry -- db` exits 0, a real DB file is created on first call with all tables present, and in-memory pools work for tests.

## Scope

### In Scope
- **CREATE** `database/migrations/001_initial.sql` — SQL DDL for five tables: `jobs`, `models`, `artifacts`, `seed_history`, `device_capabilities` (with indexes), per the DDL in `SUPPORTED_DEVICES_DB.md §Migration DDL` and the `Job`/`ModelMeta`/`ArtifactMeta` types from `anvilml-core`.
- **CREATE** `crates/anvilml-registry/src/db.rs` — module with:
  - `pub async fn open(path: &Path) -> Result<SqlitePool>` — file-backed pool with WAL mode, migration runner, ghost-job reset.
  - `pub async fn open_in_memory() -> Result<SqlitePool>` — in-memory pool with migrations.
- **CREATE** `crates/anvilml-registry/tests/db_tests.rs` — integration tests for both functions.
- **MODIFY** `crates/anvilml-registry/src/lib.rs` — add `pub mod db;` declaration and `pub use db::{open, open_in_memory};` re-export.
- **MODIFY** `crates/anvilml-registry/Cargo.toml` — add `serial_test` dev-dependency for test isolation.

### Out of Scope
- The SeedLoader (P5-A2) — SHA256-gated SQL seed runner is a separate task.
- ModelScanner, ModelStore, DeviceStore (P5-A2, P5-A3) — these are separate tasks in Phase 005.
- Wiring the SqlitePool into `main.rs` and `AppState` (P5-B1) — separate task.
- Any HTTP handler changes.

## Existing Codebase Assessment

The `anvilml-registry` crate exists at `crates/anvilml-registry/` with only a stub `lib.rs` containing `pub fn stub() {}`. No source modules (`db.rs`, `scanner.rs`, etc.) have been created yet. The `database/migrations/` directory does not exist. The `crates/anvilml-registry/tests/` directory also does not exist.

The workspace already declares `sqlx` with features `["runtime-tokio", "sqlite", "json"]` at version 0.9.0 in `Cargo.toml`, and `anvilml-registry/Cargo.toml` already lists `sqlx = { workspace = true }` as a dependency. The `anvilml-core` crate exports `Job`, `JobStatus`, `JobSettings`, `ModelMeta`, `ModelKind`, `ModelDtype`, `ModelFormat`, and `ArtifactMeta` — all types needed for the migration DDL.

The `AnvilError` enum already has a `Db(#[from] sqlx::Error)` variant, so `?` propagation from sqlx operations will convert naturally. The existing `backend/src/main.rs` uses `sqlx::SqlitePool::connect("sqlite::memory:")` as a placeholder, confirming the pattern for pool creation.

Established patterns to follow:
- Error handling: `?` propagation, `Result<T>` return types, no `.unwrap()` in production code.
- Logging: structured `tracing!` macros with named fields, mandatory INFO log points from ENVIRONMENT.md §9.
- Documentation: `///` doc comments on every `pub` item, inline `//` comments at decision points.
- Test style: separate test crate files in `crates/*/tests/`, no `#[cfg(test)]` inline blocks.
- `lib.rs` discipline: only `pub mod`, `pub use`, and `//!` crate-level doc comment.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| crate  | sqlx      | 0.9.0           | Cargo.lock (MCP rust-docs unavailable) | runtime-tokio, sqlite, json |
| crate  | serial_test | 3.2.0         | Cargo.lock fallback | n/a |

The `rust-docs` MCP server (`mcp-package-docs`) is unavailable (crashed on import). sqlx 0.9.0 is confirmed from `Cargo.lock`. The `serial_test` crate is a new dev-dependency for test isolation (env-var tests need `#[serial]`). Its version is taken from crates.io latest stable as of the current date.

## Approach

1. **Create `database/migrations/` directory.** This is the directory the `sqlx::migrate!` macro will scan. It must exist before migrations can run.

2. **Create `database/migrations/001_initial.sql`** with five table definitions:
   - `jobs` table: columns `id` (TEXT PRIMARY KEY, UUID hex), `status` (TEXT NOT NULL), `graph` (TEXT NOT NULL), `settings` (TEXT NOT NULL), `created_at` (TEXT NOT NULL, ISO8601), `started_at` (TEXT), `completed_at` (TEXT), `worker_id` (TEXT), `error` (TEXT), `queue_position` (INTEGER). Add index on `status` for filtering.
   - `models` table: columns `id` (TEXT PRIMARY KEY, SHA256 hex), `name` (TEXT NOT NULL), `path` (TEXT NOT NULL), `kind` (TEXT NOT NULL), `dtype` (TEXT NOT NULL), `format` (TEXT NOT NULL), `size_bytes` (INTEGER NOT NULL), `scanned_at` (TEXT NOT NULL).
   - `artifacts` table: columns `id` (INTEGER PRIMARY KEY AUTOINCREMENT), `job_id` (TEXT NOT NULL, FK→jobs.id), `hash` (TEXT NOT NULL UNIQUE), `path` (TEXT NOT NULL), `size_bytes` (INTEGER NOT NULL), `created_at` (TEXT NOT NULL). Add index on `job_id`.
   - `seed_history` table: columns `file` (TEXT PRIMARY KEY), `sha256` (TEXT NOT NULL), `applied_at` (TEXT NOT NULL).
   - `device_capabilities` table: per `SUPPORTED_DEVICES_DB.md §Migration DDL` — columns `vendor_id`, `device_id`, `name`, `arch`, `fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention` (all INTEGER NOT NULL DEFAULT 0), PRIMARY KEY (vendor_id, device_id). Add unique index.
   - Rationale: Use TEXT for UUID and DateTime<Utc> fields because SQLite has no native UUID or DateTime type — TEXT with ISO8601 format is the standard sqlx pattern. Use INTEGER AUTOINCREMENT for artifacts.id so it can serve as a stable internal primary key while `hash` provides the content-addressed uniqueness.

3. **Create `crates/anvilml-registry/src/db.rs`** with the following functions:
   - `pub async fn open(path: &Path) -> Result<SqlitePool, AnvilError>`:
     a. Build `SqliteConnectOptions` with the path, enable WAL mode via `.journal_mode(JournalMode::Wal)`, and set `.create_if_missing(true)`.
     b. Connect via `SqlitePool::connect_with(opts)`.
     c. Log "database created" at INFO if the file did not exist before (check with `path.exists()` before connecting; log `path=` field).
     d. Run `sqlx::migrate!("./../../database/migrations")` — this is a compile-time macro that applies all numbered migrations. The path is relative to the crate source directory (`crates/anvilml-registry/src/`), so `./../../database/migrations` resolves to `database/migrations/`.
     e. After migrations, run the ghost-job reset: `UPDATE jobs SET status = 'Failed', error = 'server_restart' WHERE status IN ('Queued', 'Running')`. Count affected rows and log at INFO with `ghost_jobs_reset=`.
     f. Return the pool.
     g. Rationale for `sqlx::migrate!` path: the macro embeds the migration directory path at compile time. The relative path from the crate's src/ directory to database/migrations/ is `./../../database/migrations`. This is the same pattern used by the workspace convention.
     h. Rationale for using `JournalMode::Wal`: WAL mode provides better concurrent read performance and prevents the "database is locked" errors that plague journal mode under concurrent access from multiple tasks.
   - `pub async fn open_in_memory() -> Result<SqlitePool, AnvilError>`:
     a. Connect to `"sqlite::memory:"` via `SqlitePool::connect`.
     b. Run `sqlx::migrate!` with the same path as `open()`.
     c. Run ghost-job reset (no-op on empty tables).
     d. Return the pool.
     e. Rationale: the in-memory pool uses the exact same migration and reset logic as the file-backed pool, ensuring test behavior matches production behavior.

4. **Update `crates/anvilml-registry/src/lib.rs`**:
   - Add `pub mod db;` to declare the new module.
   - Add `pub use db::{open, open_in_memory};` to re-export the public functions.
   - Remove the `#[allow(dead_code)]` and `pub fn stub()` — the stub is no longer needed.
   - Keep the existing `//!` crate-level doc comment (it already describes the crate's responsibilities).

5. **Create `crates/anvilml-registry/tests/db_tests.rs`** with the following tests:
   - `test_open_creates_file`: calls `open()` with a temp dir path, verifies the DB file is created on disk, verifies all five tables exist via `sqlite_master` query, then cleans up.
   - `test_open_wal_mode`: calls `open()` with a temp dir path, verifies the WAL mode is active by querying `PRAGMA journal_mode` and asserting the result is `"wal"`.
   - `test_open_in_memory`: calls `open_in_memory()`, verifies all five tables exist via `sqlite_master` query.
   - `test_ghost_job_reset`: inserts a job row with status `'Queued'` into the in-memory pool, calls `open_in_memory()` (which runs ghost reset), then queries the job and verifies its status changed to `'Failed'` with error `'server_restart'`.
   - `test_ghost_job_noop`: inserts jobs with status `'Completed'` and `'Failed'` into the in-memory pool, calls `open_in_memory()`, then verifies those jobs are unchanged (ghost reset only targets Queued/Running).
   - Rationale for test isolation: each test uses its own `open()` or `open_in_memory()` call — no shared database connections. The temp file tests use `tempfile::tempdir()` for unique paths.

6. **Add `serial_test` dev-dependency** to `crates/anvilml-registry/Cargo.toml`:
   - Add `[dev-dependencies]` section with `serial_test = "3.2.0"` (or the latest available version).
   - Rationale: `test_ghost_job_reset` and `test_ghost_job_noop` share the in-memory pool pattern but don't need `#[serial]` since each test creates its own pool. However, adding the dependency prepares for future tests that may need it. Actually, for this task, no tests mutate process-global state, so `serial_test` is not strictly needed. I'll add it as a dev-dependency to match the project's established convention for test isolation, as noted in ENVIRONMENT.md §11.3.

7. **Apply `cargo fmt --all`** after all file changes.

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `open` | `pub async fn` | `anvilml_registry::db` | `pub async fn open(path: &Path) -> Result<SqlitePool, AnvilError>` |
| `open_in_memory` | `pub async fn` | `anvilml_registry::db` | `pub async fn open_in_memory() -> Result<SqlitePool, AnvilError>` |

Both functions:
- Accept no arguments that carry domain semantics (just `&Path` for `open`).
- Return `Result<SqlitePool, AnvilError>` — the `AnvilError::Db` variant wraps `sqlx::Error` via `#[from]`.
- Apply migrations and ghost-job reset on every call.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `database/migrations/001_initial.sql` | Initial migration: jobs, models, artifacts, seed_history, device_capabilities tables with indexes |
| CREATE | `crates/anvilml-registry/src/db.rs` | `open()`, `open_in_memory()` — pool creation, migration runner, ghost-job reset |
| CREATE | `crates/anvilml-registry/tests/db_tests.rs` | Integration tests for `open()` and `open_in_memory()` |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Add `pub mod db;` and `pub use db::{open, open_in_memory};`; remove stub |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Add `[dev-dependencies]` section with `serial_test` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-registry/tests/db_tests.rs` | `test_open_creates_file` | `open()` creates the DB file on disk and all five tables exist | Temp directory exists | Path to temp dir | DB file created, sqlite_master contains all 5 tables | `cargo test -p anvilml-registry --features mock-hardware -- db::test_open_creates_file` exits 0 |
| `crates/anvilml-registry/tests/db_tests.rs` | `test_open_wal_mode` | WAL mode is enabled after `open()` | Temp directory exists | Path to temp dir | PRAGMA journal_mode returns "wal" | `cargo test -p anvilml-registry --features mock-hardware -- db::test_open_wal_mode` exits 0 |
| `crates/anvilml-registry/tests/db_tests.rs` | `test_open_in_memory` | In-memory pool has all five tables | None | None (uses `sqlite::memory:`) | sqlite_master contains all 5 tables | `cargo test -p anvilml-registry --features mock-hardware -- db::test_open_in_memory` exits 0 |
| `crates/anvilml-registry/tests/db_tests.rs` | `test_ghost_job_reset` | Ghost jobs (Queued/Running) are reset to Failed with error | In-memory pool is empty initially | INSERT job with status='Queued' | Job status='Failed', error='server_restart' | `cargo test -p anvilml-registry --features mock-hardware -- db::test_ghost_job_reset` exits 0 |
| `crates/anvilml-registry/tests/db_tests.rs` | `test_ghost_job_noop` | Non-ghost jobs (Completed/Failed) are unchanged by ghost reset | In-memory pool is empty initially | INSERT jobs with status='Completed', 'Failed' | Jobs retain their original status | `cargo test -p anvilml-registry --features mock-hardware -- db::test_ghost_job_noop` exits 0 |

## CI Impact

No CI changes required. The new tests are picked up automatically by `cargo test --workspace --features mock-hardware`. The new migration file is in the `database/migrations/` directory which is scanned by `sqlx::migrate!` at compile time — no CI configuration changes needed. The `serial_test` dev-dependency is only compiled for tests, not for the release binary.

## Platform Considerations

None identified. The SQLite database path uses `std::path::Path`, which is platform-neutral. The `sqlite::memory:` URL is handled natively by the SQLite C library across all platforms. The `sqlx::migrate!` macro path is resolved at compile time from the crate source directory, which is consistent across Linux and Windows. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| `sqlx::migrate!` macro path is incorrect — the embedded path from `crates/anvilml-registry/src/` to `database/migrations/` may not resolve correctly at compile time, causing a build error. | Medium | High | Use the absolute path via `env!("CARGO_MANIFEST_DIR")` to compute the migrations path at compile time: `sqlx::migrate!(env!("CARGO_MANIFEST_DIR").replace("crates/anvilml-registry", "database/migrations"))`. If this is too complex, use the relative path `./../../database/migrations` and verify with `cargo check`. |
| Ghost-job reset runs before any jobs exist — the `UPDATE jobs SET ... WHERE status IN (...)` on a freshly created table will affect 0 rows, which is correct but could produce a warning or error if the table doesn't exist yet. | Low | Medium | The `open_in_memory()` function runs migrations first (creating the jobs table), then runs the ghost reset. Since migrations run before the UPDATE, the table exists. For `open()`, same ordering applies. No risk. |
| `serial_test` dev-dependency version may not be available or may conflict with existing dev-dependencies. | Low | Low | Use the latest stable version from crates.io (3.2.0 as of current date). If unavailable, fall back to `--test-threads=1` CLI flag in the acceptance command. |
| The migration SQL uses `sqlite_master` queries in tests — if the table names differ from expectations, tests will fail silently or with confusing errors. | Medium | Medium | Use `SELECT name FROM sqlite_master WHERE type='table' ORDER BY name` and assert the exact set of expected table names. This provides a clear assertion message if tables are missing. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry --features mock-hardware -- db` exits 0
- [ ] `cargo build -p anvilml-registry --features mock-hardware` exits 0 (migration macro compiles)
- [ ] `cargo fmt --all -- --check` exits 0 (format gate)
- [ ] `cargo clippy -p anvilml-registry --features mock-hardware -- -D warnings` exits 0 (lint gate)
- [ ] File `database/migrations/001_initial.sql` exists and contains CREATE TABLE statements for all five tables: jobs, models, artifacts, seed_history, device_capabilities
- [ ] Function `open()` creates a real file on disk when given a non-memory path
- [ ] Function `open_in_memory()` creates an in-memory pool that passes migration and ghost-reset logic
- [ ] Ghost-job reset changes Queued/Running jobs to Failed with error='server_restart'
- [ ] Ghost-job reset leaves Completed/Failed/Cancelled jobs unchanged
- [ ] WAL mode is active (PRAGMA journal_mode returns "wal")
- [ ] `crates/anvilml-registry/src/lib.rs` contains `pub mod db;` and `pub use db::{open, open_in_memory};`
