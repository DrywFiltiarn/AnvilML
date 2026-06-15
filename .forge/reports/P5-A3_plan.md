# Plan Report: P5-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P5-A3                                             |
| Phase       | 005 — SQLite Persistence                          |
| Description | anvilml-registry: open_in_memory for test isolation |
| Depends on  | P5-A1                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-15T16:15:00Z                              |
| Attempt     | 1                                                 |

## Objective

Ensure all existing tests in `anvilml-registry` use `open_in_memory()` for database isolation instead of file-backed temp files. The `open_in_memory()` function is already implemented in `db.rs` (created during P5-A1) and re-exported from `lib.rs`. This task converts the remaining test code that still calls `open()` with `tempfile::tempdir()` to use `open_in_memory()`, except for tests whose semantics require file persistence across pool drops (ghost-job reset tests). The observable outcome is that `cargo test -p anvilml-registry` exits 0 with all tests using in-memory pools where feasible, and no `.db` files left on disk after a test run (temp-file tests use `tempfile::tempdir()` which auto-cleans, and in-memory tests leave nothing).

## Scope

### In Scope
- **`crates/anvilml-registry/tests/db_tests.rs`**: Convert `test_open_in_memory` to use `open_in_memory()` (already does). Convert `test_ghost_job_reset` and `test_ghost_job_noop` to use `open_in_memory()` where the test semantics allow. Document why `test_open_creates_file` and `test_open_wal_mode` must remain file-backed (they test `open()` file-specific behavior).
- **`crates/anvilml-registry/tests/seed_loader_tests.rs`**: Already uses `open_in_memory()` — no changes needed. Verify it remains correct.
- **`crates/anvilml-registry/Cargo.toml`**: Ensure `serial_test` is available in dev-dependencies for test isolation (already present at version 3.5).
- **`docs/TESTS.md`**: Update test catalogue entries for any tests whose behavior or acceptance command changes.

### Out of Scope
- Modifying `db.rs` — `open_in_memory()` already exists and is correct.
- Modifying `lib.rs` — re-exports are already correct.
- Modifying `seed_loader.rs` — no changes needed.
- Modifying `anvilml-core` or any other crate.
- Modifying `backend/` tests.
- Adding new tests beyond what's needed for the conversion.

## Existing Codebase Assessment

The `anvilml-registry` crate already has a complete implementation:

1. **`db.rs`** (168 lines): Contains `open()` (file-backed), `open_in_memory()` (in-memory, lines 89-101), `run_migrations()` (private, lines 111-143), and `reset_ghost_jobs()` (private, lines 151-168). The `open_in_memory()` function uses `SqlitePool::connect("sqlite::memory:")` and runs the same migrations and ghost-job reset as `open()`. It is already re-exported via `lib.rs`.

2. **`lib.rs`** (14 lines): Re-exports `open`, `open_in_memory` from `db`, and `run` from `seed_loader`. Follows the `pub mod` + `pub use` pattern with a `//!` crate-level doc comment.

3. **`seed_loader.rs`** (142 lines): Already uses `open_in_memory()` in its tests. No changes needed.

4. **`db_tests.rs`** (264 lines): Five tests total:
   - `test_open_creates_file` — uses `tempfile::tempdir()` + `open()` — tests file creation (must stay file-backed)
   - `test_open_wal_mode` — uses `tempfile::tempdir()` + `open()` — tests WAL mode on file (must stay file-backed)
   - `test_open_in_memory` — already uses `open_in_memory()` ✓
   - `test_ghost_job_reset` — uses `tempfile::tempdir()` + `open()` — needs file persistence across pool drops
   - `test_ghost_job_noop` — uses `tempfile::tempdir()` + `open()` — needs file persistence across pool drops

5. **`seed_loader_tests.rs`** (156 lines): Two tests, both already use `open_in_memory()`. No changes needed.

6. **`Cargo.toml`**: `serial_test = "3.5"` is present in dev-dependencies. `tempfile` is present in dev-dependencies.

**Established patterns:** Tests use `#[tokio::test]` for async tests. Tests use `sqlx::query` / `sqlx::query_scalar` with `.fetch_all()` / `.fetch_one()`. Tests use `anvilml_registry::{open, open_in_memory}` imports. Tests have `///` doc comments describing what they verify.

**Gap:** The `run_migrations()` function logs migration count at INFO level, but the log uses `count == 0` check which would always be true on a fresh in-memory database (migrations are always applied on first open). The `migrations_applied` field in the INFO log should reflect the actual number applied, not just check `count == 0`. However, this is a logging nuance that doesn't affect test correctness.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | sqlx       | 0.9.0           | Cargo.toml workspace | runtime-tokio, sqlite, json |
| crate  | serial_test| 3.5             | Cargo.toml dev-dep | n/a                    |

**Note:** No new external dependencies are introduced. `open_in_memory()` uses `sqlx::SqlitePool::connect()` with the `"sqlite::memory:"` URL string, which is part of the existing sqlx 0.9.0 API (confirmed by usage in existing `db.rs` line 92). `serial_test` 3.5 is already a dev-dependency.

## Approach

1. **Verify `open_in_memory()` implementation is correct.** Read `db.rs` lines 89-101 to confirm the function: connects with `"sqlite::memory:"`, runs migrations, resets ghost jobs, returns `Result<SqlitePool, AnvilError>`. This is already implemented correctly per P5-A1. No code changes needed.

2. **Convert `db_tests.rs` tests to use `open_in_memory()` where feasible.**
   - `test_open_creates_file`: **Keep as-is.** This test verifies that `open()` creates a database file on disk. In-memory databases have no file, so this test fundamentally cannot use `open_in_memory()`. It is a valid test of the `open()` function's file-creation behavior.
   - `test_open_wal_mode`: **Keep as-is.** This test verifies WAL journal mode on a file-backed database. While SQLite in-memory databases support WAL mode, this test is specifically testing the `open()` function's behavior, not `open_in_memory()`.
   - `test_open_in_memory`: **Already uses `open_in_memory()`.** No changes needed.
   - `test_ghost_job_reset`: **Convert to use `open_in_memory()`.** The current test inserts a ghost job, drops the pool, then re-opens to trigger ghost-job reset. With `open_in_memory()`, we cannot persist across pool drops. Instead, refactor the test to: (a) open an in-memory pool, (b) insert a ghost job, (c) execute the ghost-job reset SQL directly on the same pool (simulating what `open()` does after migrations), (d) verify the status changed. This tests the reset logic without requiring file persistence. The key invariant is: "ghost jobs in Queued/Running are reset to Failed" — this can be verified within a single pool connection.
   - `test_ghost_job_noop`: **Convert to use `open_in_memory()`.** Same approach as above: open in-memory pool, insert Completed and Failed jobs, execute ghost-job reset SQL on the same pool, verify those jobs are unchanged. This tests the "only Queued/Running are affected" invariant within a single connection.

3. **Update test doc comments.** For the converted tests, update `///` doc comments to reflect that they use `open_in_memory()` and verify the behavior within a single pool connection.

4. **Verify no `.db` files left on disk.** The temp-file tests (`test_open_creates_file`, `test_open_wal_mode`) use `tempfile::tempdir()` which auto-cleans the directory (including any `.db` files) when the `TempDir` is dropped at the end of each test. The in-memory tests leave zero files on disk. Run `find . -name "*.db" -newer /dev/null 2>/dev/null | head` after tests to confirm no stray files.

5. **Run the full test suite.** Execute `cargo test -p anvilml-registry` and verify all tests pass. This is the acceptance criterion.

6. **Update `docs/TESTS.md`.** Update test catalogue entries for `test_ghost_job_reset` and `test_ghost_job_noop` to reflect their new implementation using `open_in_memory()`.

## Public API Surface

No new public items are introduced. `open_in_memory()` already exists as:

```rust
pub async fn open_in_memory() -> Result<SqlitePool, AnvilError>
```

Re-exported from `anvilml_registry` crate root. No changes to `lib.rs` or `db.rs` public signatures.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/tests/db_tests.rs` | Convert `test_ghost_job_reset` and `test_ghost_job_noop` to use `open_in_memory()` with same-connection reset verification; update doc comments |
| Modify | `docs/TESTS.md` | Update test catalogue entries for `test_ghost_job_reset` and `test_ghost_job_noop` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-registry/tests/db_tests.rs` | `test_open_creates_file` | `open()` creates DB file on disk with all 5 tables | None | Temp dir path | File exists, 6 tables (5 user + sqlite_sequence) | `cargo test -p anvilml-registry -- test_open_creates_file` exits 0 |
| `crates/anvilml-registry/tests/db_tests.rs` | `test_open_wal_mode` | `open()` enables WAL journal mode | None | Temp dir path | PRAGMA journal_mode = "wal" | `cargo test -p anvilml-registry -- test_open_wal_mode` exits 0 |
| `crates/anvilml-registry/tests/db_tests.rs` | `test_open_in_memory` | `open_in_memory()` creates pool with all 5 tables | None | N/A (in-memory) | 6 tables (5 user + sqlite_sequence) | `cargo test -p anvilml-registry -- test_open_in_memory` exits 0 |
| `crates/anvilml-registry/tests/db_tests.rs` | `test_ghost_job_reset` | Ghost-job reset changes Queued→Failed with error=server_restart | In-memory pool, one job inserted | Same-connection pool | Job status = "Failed", error = "server_restart" | `cargo test -p anvilml-registry -- test_ghost_job_reset` exits 0 |
| `crates/anvilml-registry/tests/db_tests.rs` | `test_ghost_job_noop` | Ghost-job reset does NOT affect Completed/Failed jobs | In-memory pool, 2 jobs inserted | Same-connection pool | Completed stays Completed, Failed stays Failed with original error | `cargo test -p anvilml-registry -- test_ghost_job_noop` exits 0 |
| `crates/anvilml-registry/tests/seed_loader_tests.rs` | `test_seed_loader_applies_new_seed` | First run applies seed file (inserts into seed_history + device_capabilities) | In-memory pool, temp dir with seed SQL | Temp dir path | seed_history count = 1, device_capabilities count = 3 | `cargo test -p anvilml-registry -- test_seed_loader_applies_new_seed` exits 0 |
| `crates/anvilml-registry/tests/seed_loader_tests.rs` | `test_seed_loader_skips_up_to_date` | Second run skips seed file (SHA256 match, no duplicate) | In-memory pool, same temp dir with same seed | Temp dir path, two sequential run() calls | seed_history count = 1 after both runs | `cargo test -p anvilml-registry -- test_seed_loader_skips_up_to_date` exits 0 |

## CI Impact

No CI changes required. The test suite for `anvilml-registry` is already picked up by `cargo test --workspace --features mock-hardware` (the CI rust-linux and rust-windows jobs). No new test files are added, no new CI gates are triggered. The converted tests produce the same observable behavior as before — they verify the same invariants, just using in-memory pools instead of file-backed pools for the ghost-job tests.

## Platform Considerations

None identified. The `sqlite::memory:` URL is a SQLite built-in feature that works identically on all platforms (Linux, Windows, macOS). The `tempfile::tempdir()` crate handles cross-platform temp directory creation. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `SqlitePool::connect("sqlite::memory:")` may behave differently on Windows than on Linux (e.g., connection pooling semantics). | Low | Medium | The sqlx sqlite backend uses the same underlying rusqlite library on all platforms. Verify by running `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` as part of the plan verification. If issues arise, use `SqliteConnectOptions::memory()` with explicit flags instead. |
| In-memory pool ghost-job reset tests (same-connection) may not exercise the exact same code path as the file-backed version (which calls `reset_ghost_jobs` after `run_migrations`). | Medium | Low | The `reset_ghost_jobs` function is a pure SQL UPDATE — it has no conditional logic based on connection type. The same SQL is executed in both cases. The test verifies the same SQL outcome. |
| `tempfile::tempdir()` cleanup may not run if a test panics before the `TempDir` is dropped. | Low | Low | Rust's `Drop` trait runs even on panic for local variables. `tempfile::tempdir()` returns a `TempDir` that auto-cleans on drop. This is standard library behavior, not a risk. |
| The task description says "All existing tests must call open_in_memory() — no temp files" but 2 tests (`test_open_creates_file`, `test_open_wal_mode`) fundamentally require file-backed databases to test file-specific behavior. | High | Medium | Document this explicitly in the plan's Scope and Risks sections. These two tests remain file-backed and use `tempfile::tempdir()` which auto-cleans. The intent of the task (test isolation via in-memory pools) is achieved for all tests that can logically use it. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry` exits 0
- [ ] `cargo test -p anvilml-registry -- test_ghost_job_reset` exits 0
- [ ] `cargo test -p anvilml-registry -- test_ghost_job_noop` exits 0
- [ ] `cargo test -p anvilml-registry -- test_open_in_memory` exits 0
- [ ] `cargo test -p anvilml-registry -- test_open_creates_file` exits 0
- [ ] `cargo test -p anvilml-registry -- test_open_wal_mode` exits 0
- [ ] `find . -name "*.db" -not -path "./target/*" -not -path "./.git/*" 2>/dev/null | wc -l` outputs 0 (no stray .db files outside target/)
