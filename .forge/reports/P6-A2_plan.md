# Plan Report: P6-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-A2                                       |
| Phase       | 006 — Model Registry & Artifacts            |
| Description | anvilml-registry: db.rs SqlitePool creation + migration runner |
| Depends on  | P6-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-29T14:35:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-registry/src/db.rs` implementing `pub async fn create_pool(db_path: &Path) -> Result<SqlitePool, AnvilError>` that opens a SQLite connection via sqlx and runs all SQL migration files from `database/migrations/` in filename-sorted order. WAL mode is enabled on every created pool. This is the entry point every other task in Phase 6 Group A depends on to obtain a working `SqlitePool`.

## Scope

### In Scope
- Create `crates/anvilml-registry/src/db.rs` with `create_pool()` function
- Add `sqlx` dependency to `crates/anvilml-registry/Cargo.toml` with features: `sqlite`, `runtime-tokio`, `migrate`, `chrono`
- Declare `mod db;` and `pub use db::create_pool;` in `crates/anvilml-registry/src/lib.rs`
- Implement migration runner: read SQL files from `database/migrations/`, sort by filename, execute each against the pool
- Enable WAL mode via `PRAGMA journal_mode=WAL` on every pool instance
- Create `crates/anvilml-registry/tests/db_tests.rs` with >=4 integration tests

### Out of Scope
- Ghost-job reset (resetting Queued/Running jobs to Failed) — no jobs table exists yet; deferred to the scheduler phase
- Model scanning, CRUD, device capability lookup, seed loading — these are separate tasks in this phase
- Any changes to `database/migrations/001_initial.sql` — that is P6-A1's scope (already completed)
- `anvilml-artifacts` crate changes — it shares this pool but has its own tasks

## Existing Codebase Assessment

**What already exists:** The `anvilml-registry` crate skeleton exists as a workspace member with an empty `lib.rs` (one-line doc comment) and a minimal `Cargo.toml` that only depends on `anvilml-core`. The migration file `database/migrations/001_initial.sql` already exists (P6-A1), defining `models` and `device_capabilities` tables. The `AnvilError` enum in `anvilml-core/src/error.rs` already has `Db(#[from] sqlx::Error)` and `Io(#[from] std::io::Error)` variants, so sqlx errors and IO errors propagate naturally via `?`. The workspace root `Cargo.toml` uses resolver "3" with edition 2024 and rust-version 1.96.0.

**Established patterns:** Every crate's `lib.rs` contains only `//!` crate-level doc comment, `pub mod` declarations, and `pub use` re-exports — no implementation code. The error type `AnvilError` is the single error enum across the project; `sqlx::Error` maps to `AnvilError::Db` via `#[from]`. Tests for crates live in `crates/{name}/tests/` as separate test crate files (integration tests), not inline `#[cfg(test)]` blocks.

**Gap between design doc and current source:** The `anvilml-registry` crate currently has no `sqlx` dependency in its `Cargo.toml` — only `anvilml-core`. This task introduces the first direct dependency on `sqlx` (beyond the transitive one through `anvilml-core`). The `tests/` directory does not yet exist for this crate.

## Resolved Dependencies

| Type   | Name   | Version verified | MCP source     | Feature flags confirmed |
|--------|--------|-----------------|----------------|------------------------|
| crate  | sqlx   | 0.9.0           | rust-docs MCP  | sqlite, runtime-tokio, migrate, chrono |

MCP confirmed: sqlx 0.9.0 is the latest version. Feature flags `sqlite`, `runtime-tokio`, `migrate`, and `chrono` are all valid. The `migrate` feature enables `sqlx::migrate!()` macro and runtime migration execution. The `macros` feature is included in default features, so `sqlx::migrate!()` works without explicitly requesting it.

## Approach

1. **Add sqlx to Cargo.toml.** In `crates/anvilml-registry/Cargo.toml`, add `sqlx = { version = "0.9.0", features = ["sqlite", "runtime-tokio", "migrate", "chrono"] }` under `[dependencies]`. The `sqlite` feature enables SQLite support; `runtime-tokio` enables the tokio async runtime; `migrate` enables `sqlx::migrate!()` and `SqlitePool::migrate()`; `chrono` enables `DateTime<Utc>` column type mapping (needed for `scanned_at` TEXT columns that store ISO 8601 timestamps).

2. **Create `crates/anvilml-registry/src/db.rs`.** Implement:
   ```rust
   pub async fn create_pool(db_path: &Path) -> Result<SqlitePool, AnvilError>
   ```
   Steps inside `create_pool`:
   a. Create the directory for `db_path` if it does not exist (using `std::fs::create_dir_all`), so that `create_pool("./data/anvilml.db")` works even if the `data/` parent directory is absent.
   b. Build a `SqlitePoolOptions` with a reasonable connection limit (default is fine — sqlx's default is 4 connections for SQLite which is appropriate).
   c. Connect via `SqlitePoolOptions::connect_with(&db_path.into())` to create/open the database file.
   d. Execute `PRAGMA journal_mode=WAL` on the pool to enable WAL mode. Use `sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await` — the result row contains `"wal"` confirming success. Log at DEBUG level that WAL mode was enabled.
   e. Run migrations via `sqlx::migrate!("../../database/migrations").run(&pool).await`. The migration directory is relative to the crate root (`crates/anvilml-registry/`), so the path `../../database/migrations` from `src/db.rs` resolves to `database/migrations/`. The `sqlx::migrate!()` macro embeds the migration file list at compile time, and `.run()` executes them in filename-sorted order (001_initial.sql runs first). This is idempotent — running migrations against an already-migrated database is a no-op.
   f. Return the `SqlitePool`.

3. **Update `crates/anvilml-registry/src/lib.rs`.** Add `mod db;` and `pub use db::create_pool;` after the existing crate-level doc comment. The file will remain well under 80 lines.

4. **Create `crates/anvilml-registry/tests/db_tests.rs`.** Write >=4 integration tests:
   a. `test_pool_creation_succeeds` — creates a temp file path, calls `create_pool()`, asserts the pool is valid by executing a simple query.
   b. `test_migrations_create_tables` — creates pool against temp file, queries `sqlite_master` to verify both `models` and `device_capabilities` tables exist.
   c. `test_wal_mode_enabled` — after pool creation, queries `PRAGMA journal_mode` and asserts the result is `"wal"`.
   d. `test_migrations_idempotent` — creates pool (which runs migrations), then creates a second pool against the same file (which runs migrations again), asserts both pools work without error.

5. **Add `serial_test` to dev-dependencies** (already present in `anvilml-core` but needed here for test isolation with temp database files). Actually, since each test creates its own temp file, no `#[serial]` annotation is needed — each test operates on a distinct file path.

## Public API Surface

| Crate/Module | Item | Signature |
|--------------|------|-----------|
| `anvilml-registry::db` | `create_pool` | `pub async fn create_pool(db_path: &Path) -> Result<SqlitePool, AnvilError>` |
| `anvilml-registry` (re-export) | `create_pool` | Re-exported as `anvilml_registry::create_pool` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/db.rs` | SqlitePool creation + migration runner |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Add sqlx dependency with features |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Add `mod db;` and `pub use db::create_pool;` |
| CREATE | `crates/anvilml-registry/tests/db_tests.rs` | >=4 integration tests for pool creation, migrations, WAL, idempotency |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-registry/tests/db_tests.rs` | `test_pool_creation_succeeds` | Pool creation against a temp file succeeds and can execute queries | Temp file path created by `tempfile::NamedTempFile` | A temporary file path (e.g. `/tmp/anvilml_test_123.db`) | Pool created, simple SELECT query returns Ok | `cargo test -p anvilml-registry --test db_tests test_pool_creation_succeeds` exits 0 |
| `crates/anvilml-registry/tests/db_tests.rs` | `test_migrations_create_tables` | Migrations create both `models` and `device_capabilities` tables | Pool created against temp file | Temp file path | `sqlite_master` query returns rows for both tables | `cargo test -p anvilml-registry --test db_tests test_migrations_create_tables` exits 0 |
| `crates/anvilml-registry/tests/db_tests.rs` | `test_wal_mode_enabled` | WAL mode is active after pool creation | Pool created against temp file | Temp file path | `PRAGMA journal_mode` returns `"wal"` | `cargo test -p anvilml-registry --test db_tests test_wal_mode_enabled` exits 0 |
| `crates/anvilml-registry/tests/db_tests.rs` | `test_migrations_idempotent` | Re-running migrations against an already-migrated db is idempotent | First pool created and migrated, second pool created on same file | Same temp file path used twice | Both pools created successfully, no migration errors | `cargo test -p anvilml-registry --test db_tests test_migrations_idempotent` exits 0 |

## CI Impact

No CI changes required. The new test file `crates/anvilml-registry/tests/db_tests.rs` is a standard Rust integration test in the crate's `tests/` directory. It is automatically discovered by `cargo test -p anvilml-registry` which already runs in the CI `rust-linux` and `rust-windows` jobs. No new CI job, gate, or configuration is needed.

## Platform Considerations

None identified. The `SqlitePool` with SQLite backend is platform-neutral. The `PRAGMA journal_mode=WAL` command works identically on Linux and Windows. The migration directory path (`../../database/migrations`) is a relative path resolved at compile time by `sqlx::migrate!()` macro, not at runtime — it is the same path regardless of platform. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `sqlx::migrate!()` macro resolves migration directory relative to the crate root at compile time; if the path is wrong, the macro fails to find any migration files at compile time (not runtime), producing a clear error message listing expected paths. | Low | Medium | Use `../../database/migrations` which resolves from `crates/anvilml-registry/src/db.rs` to `database/migrations/`. Verify by running `cargo check -p anvilml-registry` after writing the code — if the macro can't find migrations, the error message will name the exact paths it searched. |
| `PRAGMA journal_mode=WAL` may return `"wal"` on success but could fail silently on some platforms or with certain SQLite builds. | Low | Low | The `PRAGMA journal_mode` query returns a row with a single column containing the mode string. After executing `PRAGMA journal_mode=WAL`, we query `PRAGMA journal_mode` again to confirm the result is `"wal"`. If it's not, return an `AnvilError::Internal` with the actual value. |
| Temp file cleanup in tests — each test creates a temp database file that must be cleaned up to avoid leaving artifacts. | Low | Low | Use `tempfile::NamedTempFile` which automatically deletes the file when dropped. Each test gets its own temp file with a unique path, so there is no cross-test interference. |
| The `sqlx` crate pulls in a large dependency tree (sqlx-core, sqlx-macros, sqlx-sqlite, plus their transitive deps). This may increase compile time for the workspace. | Medium | Low | This is an accepted tradeoff — the registry crate needs sqlx regardless. Compile time impact is a one-time cost per CI run, not a runtime concern. The `chrono` feature adds minimal overhead. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry --test db_tests` exits 0
- [ ] `wc -l crates/anvilml-registry/src/lib.rs` reports <= 80 lines
- [ ] `cargo clippy -p anvilml-registry -- -D warnings` exits 0
- [ ] `grep -c "^## " .forge/reports/P6-A2_plan.md` reports 12 (all sections present)
