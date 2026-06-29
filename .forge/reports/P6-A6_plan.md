# Plan Report: P6-A6

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P6-A6                                                       |
| Phase       | 006 — Model Registry & Artifacts                            |
| Description | anvilml-registry: SeedLoader hash-check + bookkeeping table  |
| Depends on  | P6-A1, P6-A2, P6-A3, P6-A4, P6-A5                          |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-29T18:45:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Create `crates/anvilml-registry/src/seed_loader.rs` implementing the idempotency-check
half of the seed loader: a `SeedLoader` struct holding a `SqlitePool`, a `new()`
constructor, and an `already_applied()` method backed by a `_seed_log` bookkeeping
table. This gives P6-A7's `run()` method the hash-comparison logic it needs to decide
whether to skip or re-apply a seed file.

## Scope

### In Scope
- Create `crates/anvilml-registry/src/seed_loader.rs` with:
  - `SeedLoader` struct holding `pool: SqlitePool`
  - `SeedLoader::new(pool: SqlitePool) -> Self` constructor
  - Internal `already_applied(&self, seed_name: &str, sha256: &str) -> Result<bool, AnvilError>` method
  - `_seed_log` table creation via `CREATE TABLE IF NOT EXISTS` on first call to `already_applied()`
- Declare `mod seed_loader;` in `crates/anvilml-registry/src/lib.rs`
- Create `crates/anvilml-registry/tests/seed_loader_tests.rs` with >=3 tests

### Out of Scope
- `SeedLoader::run()` — SQL execution, seed file hashing, and hash recording are deferred to P6-A7 (confirmed: P6-A7's context states "Extend seed_loader.rs with pub async fn run(...): compute SHA256 of seed_path's content, call P6-A6's already_applied(); if true, return Ok(()); if false, execute the seed file's SQL against the pool and record the new hash+timestamp").
- The `pub use seed_loader::SeedLoader;` re-export in `lib.rs` — deferred to P6-A9.

## Existing Codebase Assessment

`anvilml-registry` already has four modules (`db.rs`, `store.rs`, `device_store.rs`,
`scanner.rs`) following an established pattern: a struct holding `SqlitePool`, with
async methods that execute SQL queries via `sqlx::query`/`sqlx::query_scalar` and return
`Result<T, AnvilError>`. `AnvilError::Db` implements `From<sqlx::Error>`, so `?`
propagation works seamlessly.

Tests in `tests/db_tests.rs` use a temp file + `create_pool()`. Tests in
`tests/device_store_tests.rs` use an in-memory pool with a unique `uuid`-based cache
name to avoid the shared `:memory:` database problem, apply migrations manually via
`sqlx::migrate!()`, and use `#[tokio::test]`. This in-memory pattern is what the
`seed_loader_tests` should follow.

The design doc (§7.1) specifies `seed_loader.rs` as the fifth module in the crate's
layout, alongside the four existing modules. No `seed_loader.rs` exists yet — this
task creates it from scratch.

## Resolved Dependencies

| Type   | Name  | Version verified | MCP source     | Feature flags confirmed |
|--------|-------|-----------------|----------------|------------------------|
| crate  | sqlx  | 0.9.0           | rust-docs MCP  | sqlite, runtime-tokio, migrate, chrono |

No new dependencies are introduced. The `sqlx` crate with `sqlite` feature is already
declared in `crates/anvilml-registry/Cargo.toml`. The `CREATE TABLE IF NOT EXISTS` and
`SELECT` statements used here are core SQLite DDL/DML — fully supported by the
existing `sqlite` feature.

## Approach

1. **Create `crates/anvilml-registry/src/seed_loader.rs`.**
   - Add a module-level `//!` doc comment describing the module's purpose: SHA256-gated
     seed idempotency checking via the `_seed_log` bookkeeping table.
   - Implement `pub struct SeedLoader { pool: SqlitePool }` — a simple newtype holding
     the pool, matching the pattern used by `ModelStore`, `DeviceCapabilityStore`, etc.
   - Implement `pub fn new(pool: SqlitePool) -> Self` — a synchronous constructor that
     takes ownership of the pool.
   - Implement `async fn already_applied(&self, seed_name: &str, sha256: &str) -> Result<bool, AnvilError>`:
     a. Execute `CREATE TABLE IF NOT EXISTS _seed_log(seed_name TEXT PRIMARY KEY, sha256 TEXT NOT NULL, applied_at TEXT NOT NULL)` against the pool. This is a no-op if the table already exists (idempotent DDL), and ensures the table is present before any SELECT.
     b. Execute `SELECT sha256 FROM _seed_log WHERE seed_name = ?` with the `seed_name` parameter.
     c. If the query returns `Ok(Some(stored_hash))` and `stored_hash == sha256`, return `Ok(true)`.
     d. If the query returns `Ok(Some(stored_hash))` but `stored_hash != sha256`, return `Ok(false)` — the seed file has changed since last run.
     e. If the query returns `Err(sqlx::Error::RowNotFound)`, return `Ok(false)` — this seed has never been applied.
     f. Any other sqlx error propagates as `AnvilError::Db` via `?`.
   - Add a `///` doc comment on `already_applied()` explaining the three return cases (true = hash matches; false = unseen or hash mismatch) and that it returns `Err` only for genuine database errors.

2. **Update `crates/anvilml-registry/src/lib.rs`.**
   - Add `pub mod seed_loader;` to the module declarations.
   - Do NOT add `pub use seed_loader::SeedLoader;` — that is P6-A9's scope.

3. **Create `crates/anvilml-registry/tests/seed_loader_tests.rs`.**
   - Follow the in-memory pool pattern from `device_store_tests.rs`: unique UUID-based
     cache name, migrations applied via `sqlx::migrate!()`, one pool per test.
   - Write three tests (described below).
   - Each test has a doc comment explaining what it verifies, the precondition, and
     the expected outcome.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| struct | `anvilml_registry::SeedLoader` | `pub struct SeedLoader { pool: SqlitePool }` |
| fn | `anvilml_registry::SeedLoader::new` | `pub fn new(pool: SqlitePool) -> Self` |
| fn | `anvilml_registry::SeedLoader::already_applied` | `async fn already_applied(&self, seed_name: &str, sha256: &str) -> Result<bool, AnvilError>` |

No public re-export yet — `pub use seed_loader::SeedLoader;` is deferred to P6-A9.
The `already_applied` method is `pub` because it is the entrypoint that P6-A7's
`run()` will call; declaring it pub now avoids a breaking change when P6-A7 extends
the struct.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/seed_loader.rs` | SeedLoader struct, new(), already_applied(), _seed_log table creation |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Add `pub mod seed_loader;` |
| CREATE | `crates/anvilml-registry/tests/seed_loader_tests.rs` | >=3 integration tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-registry/tests/seed_loader_tests.rs` | `test_seed_log_created_on_first_use` | `_seed_log` table exists after first `already_applied()` call | In-memory pool with migrations applied; no `_seed_log` table yet | `seed_name="devices.sql"`, `sha256="abc123"` | `already_applied()` returns `Ok(false)` (unseen seed); `sqlite_master` query confirms `_seed_log` table exists | `cargo test -p anvilml-registry --test seed_loader_tests test_seed_log_created_on_first_use` exits 0 |
| `crates/anvilml-registry/tests/seed_loader_tests.rs` | `test_already_applied_unseen_seed_returns_false` | `already_applied()` returns `false` for a seed_name that has no row in `_seed_log` | In-memory pool; `_seed_log` table exists (created by prior test or explicit setup); no row for the given seed_name | `seed_name="devices.sql"`, `sha256="any_hash"` | `Ok(false)` — the seed has never been applied | `cargo test -p anvilml-registry --test seed_loader_tests test_already_applied_unseen_seed_returns_false` exits 0 |
| `crates/anvilml-registry/tests/seed_loader_tests.rs` | `test_already_applied_hash_mismatch_returns_false` | `already_applied()` returns `false` when a row exists but the sha256 differs from the stored value | In-memory pool; `_seed_log` has a row for `seed_name="devices.sql"` with `sha256="old_hash"` | `seed_name="devices.sql"`, `sha256="new_hash"` | `Ok(false)` — the seed file has changed since last run | `cargo test -p anvilml-registry --test seed_loader_tests test_already_applied_hash_mismatch_returns_false` exits 0 |

## CI Impact

No CI changes required. The test file lives in `crates/anvilml-registry/tests/` which
is already picked up by `cargo test --workspace` (the standard CI test command). No new
file types, gates, or CI jobs are introduced.

## Platform Considerations

None identified. SQLite in-memory databases (`file:<uuid>?mode=memory&cache=shared`)
are platform-neutral. The `_seed_log` table uses `TEXT PRIMARY KEY` and `TEXT NOT NULL`
columns — no platform-specific types or path handling. The Windows cross-check in
ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `CREATE TABLE IF NOT EXISTS` on an in-memory database with a unique cache name: the table created in one connection may not be visible in another if the cache name differs. | Low | Medium | Use a single connection (`max_connections(1)`) per test pool, matching the pattern in `device_store_tests.rs`. The CREATE and SELECT happen on the same pool/connection. |
| `sqlx::Error::RowNotFound` is not a distinct variant in sqlx 0.9.0 — `query_scalar` on a missing row returns `Err(sqlx::Error::RowNotFound)` which matches via `?` into `AnvilError::Db`, causing the method to propagate an error instead of returning `Ok(false)`. | Medium | High | Use `query_scalar().fetch_optional()` which returns `Result<Option<T>, sqlx::Error>` where `RowNotFound` is converted to `Ok(None)` — this is the correct pattern for "optional row" queries in sqlx. |
| The `_seed_log` table creation on first call interleaves with P6-A7's `run()` which also calls `already_applied()` — two concurrent calls could race on table creation. | Low | Low | This code runs at server startup (single-threaded, before any async work begins), so there is no concurrency. Document this assumption in a `//` comment. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry --test seed_loader_tests` exits 0
- [ ] `wc -l crates/anvilml-registry/src/lib.rs` reports <=80 lines
- [ ] `grep "^pub mod" crates/anvilml-registry/src/lib.rs` includes `seed_loader`
- [ ] `grep -c "#\[tokio::test\]" crates/anvilml-registry/tests/seed_loader_tests.rs` >= 3
