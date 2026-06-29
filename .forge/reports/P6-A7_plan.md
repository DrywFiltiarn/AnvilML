# Plan Report: P6-A7

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-A7                                       |
| Phase       | 6 — Model Registry & Artifacts              |
| Description | anvilml-registry: SeedLoader::run() SQL execution + recording |
| Depends on  | P6-A6                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-29T21:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete the `SeedLoader` by implementing `pub async fn run(&self, seed_name: &str, seed_path: &Path) -> Result<(), AnvilError>` in `crates/anvilml-registry/src/seed_loader.rs`. This method computes the SHA256 hash of a seed file, checks idempotency via `already_applied()` (from P6-A6), and either skips (already applied) or executes the SQL and records the new hash+timestamp. This makes the seed loader a usable, idempotent, one-time runner for `database/seeds/devices.sql` at server startup. Also add `pub use seed_loader::SeedLoader;` to `lib.rs`.

## Scope

### In Scope
- Implement `pub async fn run(&self, seed_name: &str, seed_path: &Path) -> Result<(), AnvilError>` in `crates/anvilml-registry/src/seed_loader.rs`:
  - Compute SHA256 of `seed_path`'s full content using `sha2::Sha256`.
  - Call `already_applied(seed_name, sha256_hex)`.
  - If `true`, return `Ok(())` (skip, idempotent).
  - If `false`, execute the seed file's SQL against the pool within a transaction, then record the hash+timestamp into `_seed_log`.
- Add `pub use seed_loader::SeedLoader;` to `crates/anvilml-registry/src/lib.rs`.
- Write >=4 new tests in `crates/anvilml-registry/tests/seed_loader_tests.rs`.
- Bump `anvilml-registry` crate version from 0.1.5 to 0.1.6.

### Out of Scope
None. This task's `defers_to` is `[]` — no functionality may be deferred. The seed data file (`database/seeds/devices.sql`) itself is the separate one-time conversion task P6-A8, not this task's scope.

## Existing Codebase Assessment

The `SeedLoader` struct already exists in `crates/anvilml-registry/src/seed_loader.rs` with two methods implemented by P6-A6:
- `SeedLoader::new(pool: SqlitePool)` — constructs the loader.
- `pub async fn already_applied(&self, seed_name: &str, sha256: &str) -> Result<bool, AnvilError>` — checks the `_seed_log` bookkeeping table (created lazily via `CREATE TABLE IF NOT EXISTS`) and returns whether a seed was already applied with the matching hash.

The `_seed_log` table schema is: `(seed_name TEXT PRIMARY KEY, sha256 TEXT NOT NULL, applied_at TEXT NOT NULL)`. The existing code uses `sqlx::query` for DDL, `fetch_optional()` for optional-row queries, and `#[tracing::instrument]` for span logging. All tests use the `make_pool()` helper that creates an in-memory SQLite pool with a unique cache name and applies the `001_initial.sql` migration.

The `lib.rs` currently has `pub mod seed_loader;` but no `pub use seed_loader::SeedLoader;` — that re-export is handled by P6-A9 (the lib.rs closing task). The `Cargo.toml` already has `sha2 = "0.11"` and `digest = "0.11"` as dependencies, verified via MCP to be compatible with the project's Rust 1.96.0 toolchain (MSRV 1.85).

The existing test file has 3 tests from P6-A6 covering: unseen seed returns false, hash mismatch returns false, and hash match returns true. The `make_pool()` helper and `uuid::Uuid::new_v4()` pattern for unique in-memory databases are established conventions.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | sha2    | 0.11.0          | rust-docs MCP  | alloc, oid (default)   |
| crate  | digest  | 0.11.x          | in Cargo.toml  | n/a                    |

Both `sha2` and `digest` are already declared in `anvilml-registry/Cargo.toml`. No new dependencies are introduced. The `sha2::Sha256` type implements `digest::Digest`, providing `new()`, `update()`, and `finalize()` methods.

## Approach

1. **Read the seed file and compute SHA256.** In `seed_loader.rs`, add `use sha2::{Sha256, Digest};`. Implement the hash computation: read `seed_path` with `std::fs::read(seed_path)?`, create a `Sha256` hasher, feed the bytes via `hasher.update(&contents)`, finalize to get a `[u8; 32]` digest, and format as lowercase hex via `format!("{:x}", hasher.finalize())`.

2. **Implement `run()` method.** Add `pub async fn run(&self, seed_name: &str, seed_path: &Path) -> Result<(), AnvilError>` to `SeedLoader`:
   - Step A: Compute SHA256 hex of the seed file content (step 1 above).
   - Step B: Call `self.already_applied(seed_name, &sha256_hex).await?`.
   - Step C: If `true`, log at DEBUG level and return `Ok(())` — the seed was already applied with this exact content.
   - Step D: If `false`, begin a transaction (`sqlx::Transaction` via `self.pool.begin().await?`), execute the seed SQL via `tx.execute_batch(&sql).await?`, then record the hash and timestamp (`chrono::Utc::now().to_rfc3339()`) into `_seed_log` via `INSERT OR REPLACE`. Commit the transaction. Log at INFO level that the seed was applied.

   The transaction wrapping (step D) is critical: it ensures that if the SQL is malformed, the entire operation rolls back and no hash+timestamp is recorded. This prevents partial application — the seed will be re-attempted on the next `run()` call. Without a transaction, a malformed SQL statement could leave partial state in the database.

3. **Add `#[tracing::instrument]` annotation** to `run()` with span fields for `seed_name` and `seed_path`, following the established pattern from `already_applied()`.

4. **Add `pub use seed_loader::SeedLoader;` to `lib.rs`.** One additional line in the existing re-export block.

5. **Write 4 new tests** in `tests/seed_loader_tests.rs` (see Tests section below). Each test uses `make_pool()` for isolation and creates a temp file via `tempfile::NamedTempFile` for the seed path.

6. **Bump `anvilml-registry` version** from `0.1.5` to `0.1.6` in `Cargo.toml`.

## Public API Surface

| Item | Location | Signature |
|------|----------|-----------|
| `pub use` | `crates/anvilml-registry/src/lib.rs` | `pub use seed_loader::SeedLoader;` |
| `pub async fn` | `crates/anvilml-registry/src/seed_loader.rs` | `pub async fn run(&self, seed_name: &str, seed_path: &Path) -> Result<(), AnvilError>` |

No new structs, enums, or traits. The `run()` method is the only new public item.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/seed_loader.rs` | Add `run()` method, SHA256 hashing imports, `chrono` usage for timestamp |
| Modify | `crates/anvilml-registry/src/lib.rs` | Add `pub use seed_loader::SeedLoader;` re-export |
| Modify | `crates/anvilml-registry/tests/seed_loader_tests.rs` | Add 4 new integration tests |
| Modify | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.5 → 0.1.6 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/seed_loader_tests.rs` | `test_run_first_time_applies_and_records` | First `run()` on a valid seed file executes SQL and records hash+timestamp in `_seed_log` | Fresh pool, temp file with valid INSERT SQL | seed_name="devices.sql", temp file with `INSERT INTO device_capabilities ...` | `_seed_log` has one row with the correct hash; `already_applied` returns true after | `cargo test -p anvilml-registry --test seed_loader_tests -- test_run_first_time_applies_and_records` exits 0 |
| `tests/seed_loader_tests.rs` | `test_run_skips_when_already_applied` | Second `run()` with unchanged file content skips SQL execution and returns `Ok(())` | Pool with `_seed_log` row from prior run | Same seed file, same content as recorded hash | `already_applied` returns true; no SQL re-execution; `run()` returns `Ok(())` | `cargo test -p anvilml-registry --test seed_loader_tests -- test_run_skips_when_already_applied` exits 0 |
| `tests/seed_loader_tests.rs` | `test_run_reapplies_on_changed_content` | After seed file content changes, `run()` detects hash mismatch, re-executes SQL, and updates `_seed_log` | Pool with `_seed_log` row for old hash | Seed file written with new content (different hash) than recorded | `_seed_log` has updated hash+timestamp; SQL was re-executed with new content | `cargo test -p anvilml-registry --test seed_loader_tests -- test_run_reapplies_on_changed_content` exits 0 |
| `tests/seed_loader_tests.rs` | `test_run_malformed_sql_returns_err_no_partial_state` | Malformed SQL returns `Err` without recording a hash+timestamp in `_seed_log` | Fresh pool, temp file with invalid SQL | seed_name="bad.sql", temp file with `INVALID SQL STATEMENT` | `run()` returns `Err`; `_seed_log` has no row for this seed_name; `already_applied` returns false | `cargo test -p anvilml-registry --test seed_loader_tests -- test_run_malformed_sql_returns_err_no_partial_state` exits 0 |

Total tests in file after this task: 7 (3 existing + 4 new).

## CI Impact

No CI changes required. The tests run as part of `cargo test --workspace --features mock-hardware` which already covers `anvilml-registry`. The new tests are in the existing `tests/seed_loader_tests.rs` integration test file, which is automatically picked up by the test runner.

## Platform Considerations

None identified. The SHA256 hash computation is platform-neutral (pure Rust `sha2::Sha256`). SQLite operations are platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tx.execute_batch()` on a `Transaction<'_, Sqlite>` may not exist in sqlx 0.9 — the Transaction type might only have `execute()` for single statements. | Low | Medium | Verify the Transaction API via docs.rs MCP lookup before writing. If `execute_batch` is unavailable, use `sqlx::query(&sql).execute(&mut *tx).await` in a loop splitting on `;` — or fall back to reading the file as a string and executing it directly without a transaction (documenting why). |
| The `_seed_log` table might not exist when `run()` is called (e.g. if someone calls `run()` directly without first calling `already_applied()`). | Low | Medium | `run()` calls `already_applied()` internally, which creates the table lazily. This is the same pattern used by P6-A6. |
| `chrono::Utc::now().to_rfc3339()` might not match the format expected by existing code. | Low | Low | The `_seed_log` column is `TEXT` with no format constraint. RFC3339 is a standard ISO 8601 format that sorts correctly as text. Verified against existing test code in `seed_loader_tests.rs` which uses `"2026-01-01T00:00:00Z"` format — `to_rfc3339()` produces a compatible format. |
| Transaction rollback on SQL error might not be automatic in sqlx — the transaction might need explicit `rollback()` call. | Low | High | In sqlx, dropping a `Transaction` without calling `commit()` triggers an automatic rollback via `Drop`. This is the standard pattern. Document this with an inline comment at the transaction scope end. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry --test seed_loader_tests` exits 0 (>=7 total tests)
- [ ] `cargo test -p anvilml-registry` exits 0 (full crate suite)
- [ ] `wc -l crates/anvilml-registry/src/lib.rs` reports <=80
- [ ] `cargo clippy -p anvilml-registry -- -D warnings` exits 0
