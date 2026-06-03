# Plan Report: P5-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P5-A2                                             |
| Phase       | 005 — SQLite Persistence                          |
| Description | anvilml-registry: db::open with PRAGMAs and migration runner |
| Depends on  | P5-A1                                               |
| Project     | anvilml                                             |
| Planned at  | 2026-06-03T16:45:00Z                               |
| Attempt     | 1                                                   |

## Objective

Add the `sqlx` dependency suite to `anvilml-registry`, create `src/db.rs` with an `async fn open(path: &Path) -> Result<SqlitePool, AnvilError>` function that configures SQLite PRAGMAs (WAL journal mode, synchronous=NORMAL, foreign_keys=ON), runs all three migration files from `backend/migrations/` via `sqlx::migrate!`, and re-exports the public API from `lib.rs`. Provide an integration test that opens a temp DB, verifies migrations created the `jobs`, `models`, and `artifacts` tables.

## Scope

### In Scope
- Add `sqlx` dependency to `crates/anvilml-registry/Cargo.toml` (features: `sqlite`, `runtime-tokio`, `macros`, `migrate`)
- Add `tempfile` as a dev-dependency for tests
- Create `crates/anvilml-registry/src/db.rs` with:
  - `async fn open(path: &std::path::Path) -> Result<sqlx::SqlitePool, AnvilError>`
  - PRAGMA configuration after pool creation (journal_mode=WAL, synchronous=NORMAL, foreign_keys=ON)
  - `sqlx::migrate!` call with path relative to CARGO_MANIFEST_DIR pointing to `../../backend/migrations`
  - Conversion of `sqlx::Error` to `AnvilError::DbError` via `From` impl
- Update `crates/anvilml-registry/src/lib.rs` to declare the `db` module and re-export `open`
- Integration test in `tests/anvilml_registry_db.rs`:
  - Create a temporary SQLite file
  - Call `anvilml_registry::db::open()` on the temp path
  - Query `sqlite_master` to verify existence of `jobs`, `models`, `artifacts` tables

### Out of Scope
- Ghost-job reset query (P5-A3)
- Startup wiring in `backend/src/main.rs` (P5-A4)
- Any changes to migration SQL files (already created by P5-A1)
- Any changes to other crates' Cargo.toml or source files

## Approach

1. **Update `Cargo.toml`** — Add `sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "macros", "migrate"] }` and `tempfile = "3"` (dev-dependency). Keep existing `anvilml-core` dependency.

2. **Create `src/db.rs`** — Implement:
   - Import `sqlx::SqlitePool`, `std::path::Path`, `anvilml_core::error::AnvilError`
   - `From<sqlx::Error>` impl for `AnvilError` converting to `DbError` variant
   - `async fn open(path: &Path) -> Result<SqlitePool, AnvilError>`:
     a. Build `SqlitePoolOptions` with reasonable pool config (`.max_connections(5)`)
     b. Create pool via `.connect(path.to_str().ok_or_else(|| AnvilError::DbError("invalid path".into()))?).await?`
     c. Execute three PRAGMAs via `sqlx::query("PRAGMA ...")`: `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`
     d. Run `sqlx::migrate!("../../backend/migrations").run(&pool).await?`
     e. Return the pool
   - `mod tests` with integration test

3. **Update `lib.rs`** — Replace stub with:
   ```rust
   pub mod db;
   pub use db::open;
   ```

4. **Create `tests/anvilml_registry_db.rs`** — Integration test module:
   - `async fn test_open_creates_tables()` using `tempfile::NamedTempFile` to get a unique temp path, open the DB, query `sqlite_master`, assert 3 rows for `jobs`, `models`, `artifacts`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/Cargo.toml` | Add sqlx + tempfile dev-dependency |
| Create | `crates/anvilml-registry/src/db.rs` | db::open with PRAGMAs and migration runner |
| Modify | `crates/anvilml-registry/src/lib.rs` | Module declaration + re-export |
| Create | `tests/anvilml_registry_db.rs` | Integration test for table creation |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-registry/src/db.rs` (mod tests) | `test_open_creates_tables` | Temp DB opened via `db::open()` has `jobs`, `models`, `artifacts` tables in sqlite_master |

## CI Impact

No CI workflow file changes. The `anvilml-registry` crate is already a workspace member, so `cargo test --workspace` (and the per-crate gate `cargo test -p anvilml-registry`) will automatically pick up the new test. No fmt/clippy concerns beyond normal checks. The `mock-hardware` feature flag does not apply to this crate (it has no hardware dependency).

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `sqlx::migrate!` path resolution may fail if the relative path from CARGO_MANIFEST_DIR is wrong | Use `../../backend/migrations` (CARGO_MANIFEST_DIR for anvilml-registry is `crates/anvilml-registry/`; two levels up reaches workspace root, then into `backend/migrations`). Verify with `cargo build` and inspect compilation output. |
| sqlx version incompatibility with Rust toolchain | Use a recent stable version (0.8.x) which targets the current Rust stable toolchain. If clippy or compile errors arise, adjust to the latest compatible version. |
| `tempfile::NamedTempFile` creates an empty file that SQLite may not handle correctly for WAL mode | Open via `sqlx::SqlitePoolOptions::connect()` which will create/initialize the database; if WAL sidecar files cause issues during cleanup, use a directory-based temp path instead. |
| PRAGMA execution order matters (foreign_keys must be set per-connection) | Execute all three PRAGMAs as separate queries against the pool immediately after connection creation, before any migration or application query. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml-registry` compiles without errors
- [ ] `cargo clippy -p anvilml-registry -- -D warnings` passes clean
- [ ] `cargo test -p anvilml-registry -- db` exits 0 with the integration test passing (tables verified)
- [ ] `src/db.rs` contains `async fn open(path: &Path) -> Result<SqlitePool, AnvilError>`
- [ ] PRAGMAs journal_mode=WAL, synchronous=NORMAL, foreign_keys=ON are set in `open()`
- [ ] `sqlx::migrate!` is invoked with path resolving to `backend/migrations/`
- [ ] `lib.rs` re-exports `db::open` as `anvilml_registry::open`
- [ ] No other crates or files are modified beyond those listed in "Files Affected"
