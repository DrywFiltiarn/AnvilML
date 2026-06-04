# Plan Report: P7-D1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-D1                                              |
| Phase       | 007 — WebSocket Event Stream                       |
| Description | anvilml-registry: fix db::open to create missing database file |
| Depends on  | P7-C1                                               |
| Project     | anvilml                                             |
| Planned at  | 2026-06-04T20:30:00Z                               |
| Attempt     | 1                                                   |

## Objective

Fix `db::open` in `crates/anvilml-registry/src/db.rs` so that it creates the SQLite database file when it does not exist, eliminating the first-run panic caused by `SqlitePoolOptions::connect` not setting `SQLITE_OPEN_CREATE`. Remove the pre-creation workaround in the server integration tests.

## Scope

### In Scope
- Modify `crates/anvilml-registry/src/db.rs`: replace `SqlitePoolOptions::connect(path_str)` with `SqliteConnectOptions::new().filename(path).create_if_missing(true).connect_with(opts)`, add `SqliteConnectOptions` to imports
- Add unit test `test_open_creates_file_if_missing` in `db.rs` that opens a path that does not yet exist and asserts the file is present afterwards
- Modify `crates/anvilml-server/tests/api_models.rs`: remove the `fs::File::create(&db_path)` pre-creation call from `setup_test_env`, remove unused `use std::fs` if it becomes unused

### Out of Scope
- Any other database-related fixes (handled by P7-D3 silent error discard fixes)
- Hardware detector changes (P7-D2)
- Dependency version upgrades (P7-C1, P7-E1–E3)
- CI workflow modifications (P7-B1)

## Approach

1. **Read the current `db.rs`** — confirm the exact `SqlitePoolOptions::connect` call at line 35 and the existing import list on line 11 (`sqlx::sqlite::{SqlitePool, SqlitePoolOptions}`).

2. **Update imports in `db.rs`** — add `SqliteConnectOptions` to the `use sqlx::sqlite` import so it reads:
   ```rust
   use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
   ```

3. **Replace the connect call in `db.rs`** — change the body of `open()` from:
   ```rust
   let pool = SqlitePoolOptions::new()
       .max_connections(5)
       .connect(path.to_str().ok_or_else(|| {
           AnvilError::DbError("database path contains invalid UTF-8".into())
       })?)
       .await
       .map_err(sqlx_error)?;
   ```
   to:
   ```rust
   let opts = SqliteConnectOptions::new()
       .filename(path)
       .create_if_missing(true);
   let pool = SqlitePoolOptions::new()
       .max_connections(5)
       .connect_with(opts)
       .await
       .map_err(sqlx_error)?;
   ```
   The `filename()` builder accepts `&Path` directly (no `.to_str()` needed), and handles Windows backslash paths correctly.

4. **Add test `test_open_creates_file_if_missing`** in the existing `#[cfg(test)] mod tests` block of `db.rs`. The test:
   - Creates a path under a temp directory that does **not** yet exist as a file
   - Calls `open(path)` and asserts success
   - Asserts `path.exists()` is true after open

5. **Remove the workaround in `api_models.rs`** — delete line 32 (`fs::File::create(&db_path).expect("pre-create db file");`) and its associated comment on line 31. If `use std::fs;` (line 1) becomes unused after this removal, remove it too.

6. **Verify compilation** — run `cargo check -p anvilml-registry` to confirm the changes compile cleanly.

7. **Run tests** — execute `cargo test --workspace --features mock-hardware` and confirm exit code 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/db.rs` | Replace connect call, add `SqliteConnectOptions` import, add test |
| Modify | `crates/anvilml-server/tests/api_models.rs` | Remove pre-create db file workaround and unused `use std::fs` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-registry/src/db.rs` (new) | `test_open_creates_file_if_missing` | Opening a non-existent database path creates the file and returns an Ok pool |
| `crates/anvilml-registry/src/db.rs` (existing) | `test_open_creates_tables` | Opening a database still creates all three expected tables |
| `crates/anvilml-registry/src/db.rs` (existing) | `test_reset_ghost_jobs` | Ghost-job reset logic unchanged |
| `crates/anvilml-server/tests/api_models.rs` (all tests) | `list_models_returns_scanned_models`, `list_models_kind_filter_diffusion`, `list_models_kind_filter_no_match` | Integration tests still pass with the pre-create workaround removed — they now rely on `open()` creating the file |
| Full suite | `cargo test --workspace --features mock-hardware` | No regressions in any other crate |

## CI Impact

No CI workflow files are modified by this task. The changes are purely source-level and test-level. The existing CI matrix (fmt, clippy, test with mock-hardware) will exercise the new code automatically. No new jobs or steps are required.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `SqliteConnectOptions` API differs in sqlx 0.9 vs earlier versions | P7-C1 already upgraded sqlx to 0.9 (confirmed via lockfile); verified that `filename()`, `create_if_missing()`, and `connect_with()` are the standard builder methods available in sqlx 0.9.0 docs.rs |
| Removing `fs::File::create` from test setup causes tests to fail if `open()` silently errors | The new unit test explicitly verifies file creation; integration tests will surface any regression immediately via `cargo test --workspace` |
| `std::fs` import becomes unused in `api_models.rs` after removal of pre-create | Rust compiler will emit an unused-import warning; clippy with `-D warnings` will catch it during verification — simply remove the line |
| Path validation (invalid UTF-8) is lost by switching to `.filename(path)` | The new code passes `&Path` directly to `.filename()` which accepts any valid path without requiring UTF-8 conversion. The UTF-8 check was only needed for the old string-based connect method; no equivalent check is needed here since `.filename()` handles non-UTF-8 paths on all platforms |

## Acceptance Criteria

- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] Delete `anvilml.db` if present, then `cargo run --features mock-hardware` starts without panicking at the database open step
- [ ] The new test `test_open_creates_file_if_missing` is present and passes
- [ ] The `fs::File::create(&db_path)` workaround is removed from `setup_test_env` in `api_models.rs`
