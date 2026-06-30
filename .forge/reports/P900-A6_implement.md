# Implementation Report: P900-A6

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P900-A6                                           |
| Phase         | 900 — Spec-Drift & Logging Retrofit               |
| Description   | backend: wire create_pool() into server startup, no AppState yet |
| Implemented   | 2026-06-30T16:22:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Wired `anvilml_registry::create_pool()` into `main()`'s default startup path so that the running `anvilml` binary creates its SQLite database and runs migrations before binding the TCP listener. Added the `anvilml-registry` path dependency to `backend/Cargo.toml`, created two integration tests in `backend/tests/db_startup_tests.rs` that verify the `.db` file is created and both required tables (`models` and `device_capabilities`) exist. Also fixed a pre-existing bug in `create_pool()` where `create_if_missing(true)` was not set on `SqliteConnectOptions`, causing SQLite to return `SQLITE_CANTOPEN` (code 14) when connecting to a database file that does not yet exist — this bug was latent because the existing tests used `NamedTempFile` which creates an empty file first.

## Resolved Dependencies

| Type   | Name            | Version resolved | Source         |
|--------|-----------------|------------------|----------------|
| crate  | anvilml-registry| 0.1.6 (path dep) | N/A (workspace) |
| crate  | tempfile        | 3.26             | Cargo registry |
| crate  | sqlx            | 0.9.0            | Cargo registry |

No external crates were introduced — `anvilml-registry` is a workspace path dependency already present in the repository. `tempfile` and `sqlx` were added to `backend/Cargo.toml`'s `[dev-dependencies]` for the integration tests.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/Cargo.toml` | Bump patch version 0.1.6 → 0.1.7; add `anvilml-registry` path dependency; add `tempfile` and `sqlx` dev-dependencies |
| Modify | `backend/src/main.rs` | Add `use anvilml_registry::create_pool;` import; call `create_pool(&config.db_path).await` in default startup path before TCP bind |
| CREATE | `backend/tests/db_startup_tests.rs` | Two integration tests: `test_db_file_created_on_startup` and `test_migrations_create_required_tables` |
| Modify | `crates/anvilml-registry/src/db.rs` | Fix `create_if_missing(true)` bug in `SqliteConnectOptions` |
| Modify | `docs/TESTS.md` | Add entries for the two new integration tests |

## Commit Log

```
 .forge/reports/P900-A6_plan.md    | 118 ++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  13 +--
 Cargo.lock                        |   5 +-
 backend/Cargo.toml                |   5 +-
 backend/src/main.rs               |  13 +++
 backend/tests/db_startup_tests.rs | 168 ++++++++++++++++++++++++++++++++++++++
 crates/anvilml-registry/src/db.rs |  11 ++-
 docs/TESTS.md                     |  24 ++++++
 9 files changed, 351 insertions(+), 12 deletions(-)
```

## Test Results

```
     Running tests/db_startup_tests.rs (target/debug/deps/db_startup_tests-1fc036c25146678e)

running 2 tests
test tests::test_db_file_created_on_startup ... ok
test tests::test_migrations_create_required_tables ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

Full workspace test suite: all 157 tests passed (0 failed).

## Format Gate

```
(no output — exit 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.64s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.16s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.16s
```

All four platform cross-checks passed.

## Project Gates

### Gate 1 — Config Surface Sync
```
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Public API Delta

```
(no output — no new pub items introduced)
```

No new public items were introduced. The `anvilml_registry::create_pool()` function is already public from Phase 6. The only code change that touches public API is the addition of `create_if_missing(true)` to `SqliteConnectOptions` in `crates/anvilml-registry/src/db.rs`, which modifies internal implementation without changing the function signature.

## Deviations from Plan

- **Fixed pre-existing bug in `create_pool()`**: The `SqliteConnectOptions::new().filename(db_path)` call was missing `.create_if_missing(true)`. By default, sqlx uses `SQLITE_OPEN_READWRITE` without `SQLITE_OPEN_CREATE`, which means SQLite returns `SQLITE_CANTOPEN` (code 14) when the database file does not exist. The existing tests passed because they used `tempfile::NamedTempFile` which creates an empty file before calling `create_pool()`. The fix adds `.create_if_missing(true)` to `SqliteConnectOptions` so that SQLite creates the database file if it does not exist. This is documented in `crates/anvilml-registry/src/db.rs` with an inline comment explaining the sqlx default behavior.

- **Test file uses `tempfile::tempdir()` instead of `tempfile::NamedTempFile`**: The plan specified `NamedTempFile`, but `NamedTempFile` creates and keeps the file open, which would conflict with the binary creating the database file. Used `tempfile::tempdir()` instead, which creates a unique temp directory and path without creating any file, allowing the binary to create the database file freely.

- **Added `ANVILML_PORT=0` to test spawns**: The plan only specified setting `ANVILML_DB_PATH`. Added `ANVILML_PORT=0` (ephemeral port) to avoid port conflicts with other tests or services, and to allow the binary to start and print the "listening" log line without needing to bind on a specific port.

- **Added `tempfile` and `sqlx` to `backend/Cargo.toml` dev-dependencies**: The plan mentioned `tempfile` as available transitively, but it is not. Added both `tempfile` and `sqlx` as explicit dev-dependencies for the integration tests.

- **Added `.stdout(Stdio::piped())` to test Command chains**: Required to prevent the child process from writing to the parent's stdout, which could interfere with test output.

## Blockers

None.
