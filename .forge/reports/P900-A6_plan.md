# Plan Report: P900-A6

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P900-A6                                           |
| Phase       | 900 — Spec-Drift & Logging Retrofit               |
| Description | backend: wire create_pool() into server startup, no AppState yet |
| Depends on  | P900-A1, P6-B3                                    |
| Project     | anvilml                                           |
| Planned at  | 2026-06-30T15:45:00Z                              |
| Attempt     | 1                                                 |

## Objective

Make the running `anvilml` binary actually create its SQLite database and run migrations. Phase 6 built `create_pool()` (pool creation plus migration runner) fully and unit-tested it, but `backend/Cargo.toml` has no dependency on `anvilml-registry`, so no code path in the running server ever calls it — no `.db` file is ever produced. This task adds the dependency and wires `create_pool(&config.db_path).await` into `main()`'s default server startup path, keeping the pool local to `main()` without introducing `AppState`. After this task, spawning the binary (even briefly) against any `db_path` will produce a SQLite database file with the `models` and `device_capabilities` tables created by migrations.

## Scope

### In Scope
- Add `anvilml-registry = { path = "../crates/anvilml-registry" }` to `backend/Cargo.toml`.
- In `backend/src/main.rs`'s default (non-`hw-probe`) startup path, after config load and before binding the TCP listener, call `anvilml_registry::create_pool(&config.db_path).await`.
- On `Err`, `eprintln!` the error and `std::process::exit(1)` before binding any socket — matching the existing config-load failure pattern.
- Keep the resulting `SqlitePool` local to `main()`. Do NOT introduce `AppState` or any part of it.
- Create `backend/tests/db_startup_tests.rs` with >=2 integration tests spawning the built binary against a temp `db_path`, asserting the `.db` file is created and both `models` and `device_capabilities` tables exist.

### Out of Scope
None. This task's `defers_to` field is empty (`[]` from JSON). No scope is deferred to any other task.

## Existing Codebase Assessment

`anvilml-registry` already exists as a workspace crate (`crates/anvilml-registry/`) with `create_pool()` fully implemented in `src/db.rs`. The function takes a `&Path`, creates the parent directory if needed, connects a `SqlitePool`, enables WAL mode, and runs all migrations from `database/migrations/`. It returns `Result<SqlitePool, AnvilError>`. The crate's `lib.rs` re-exports `create_pool` as a public function.

`backend/src/main.rs` already has the established error-handling pattern for startup failures: config loading uses `.map_err(|e| { eprintln!("Failed to load config: {e}"); std::process::exit(1); })`. The `hw-probe` subcommand path runs hardware detection and exits; the default `None` path builds the router, binds TCP, and serves. No database code exists in `main.rs` or `backend/Cargo.toml` yet.

The dependency graph (`ARCHITECTURE.md §3`) confirms `backend` is below `anvilml-registry`, so adding the dependency is valid and does not create a cycle. The existing integration test pattern in `backend/tests/hw_probe_help_test.rs` uses `Command::new(env!("CARGO_BIN_EXE_anvilml"))` which this task's tests will mirror.

## Resolved Dependencies

| Type   | Name            | Version verified | MCP source | Feature flags confirmed |
|--------|-----------------|-----------------|------------|------------------------|
| crate  | anvilml-registry| 0.1.6 (workspace path dep) | N/A | n/a (path dependency, no version pin) |

No external crates are introduced. The `anvilml-registry` crate is a workspace path dependency already present in the repository. Its API (`create_pool(&Path) -> Result<SqlitePool, AnvilError>`) was confirmed by reading `crates/anvilml-registry/src/db.rs` directly.

## Approach

1. **Add `anvilml-registry` dependency to `backend/Cargo.toml`.**
   Append `anvilml-registry = { path = "../crates/anvilml-registry" }` to the `[dependencies]` section, placing it after the existing `anvilml-server` entry. This follows the established pattern of workspace path dependencies already declared in the file.

2. **Wire `create_pool()` into `main.rs`'s default startup path.**
   In the `None` (default) arm of the subcommand match, after the match block closes and before `let start_time = Instant::now()`, insert:
   ```rust
   // Create the database pool and run migrations.
   // This is called before binding the TCP listener so that a DB failure
   // prevents the server from starting with no database — matching the
   // config-load failure pattern (eprintln + exit 1).
   let _pool = anvilml_registry::create_pool(&config.db_path)
       .await
       .map_err(|e| {
           eprintln!("Failed to create database pool: {e}");
           std::process::exit(1);
       })
       .unwrap();
   ```
   The pool is bound to `_pool` (underscore-prefixed) to indicate it is intentionally unused in this task — it is kept alive for the duration of `main()` by being a local variable, which keeps the SQLite connection open. No `AppState` is constructed; the pool simply needs to exist in scope to prevent drop. The `_` prefix suppresses the dead-code warning without introducing a `#[allow(dead_code)]` annotation.

3. **Create `backend/tests/db_startup_tests.rs`.**
   Write an integration test file that spawns the built `anvilml` binary with `ANVILML_DB_PATH` set to a temp file, waits briefly for the binary to start (which triggers DB creation), then terminates the process and verifies the database file exists with the expected tables. The test uses `tokio::time::timeout` to bound the wait, mirroring the bounded-wait pattern established in the project's subprocess tests.

   Test 1: `test_db_file_created_on_startup` — spawns the binary with a temp `db_path`, waits up to 5 seconds for the process to produce output (the "listening" log line), then asserts the `.db` file exists on disk.

   Test 2: `test_migrations_create_required_tables` — spawns the binary with a temp `db_path`, waits briefly, terminates the process, then opens the `.db` file with `sqlx` and queries `sqlite_master` to verify both `models` and `device_capabilities` tables exist.

   Both tests use `tempfile::NamedTempFile` to create a unique temp path per test, ensuring no cross-test shared state (no `#[serial]` needed for DB isolation).

## Public API Surface

No new public items are introduced. This task only modifies `main()`'s startup sequence (private to the binary) and adds a dependency. The `anvilml_registry::create_pool()` function is already public from Phase 6.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/Cargo.toml` | Add `anvilml-registry` path dependency |
| Modify | `backend/src/main.rs` | Call `create_pool()` in default startup path before TCP bind |
| CREATE | `backend/tests/db_startup_tests.rs` | Integration tests verifying DB creation and migration |
| Modify | `backend/Cargo.toml` | Bump patch version 0.1.6 → 0.1.7 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `backend/tests/db_startup_tests.rs` | `test_db_file_created_on_startup` | Spawning the binary with a temp `db_path` creates the `.db` file on disk. | `anvilml` binary compiled. Temp directory writable. | `ANVILML_DB_PATH` = temp file path, no subcommand (default path). | `.db` file exists after binary starts. | `cargo test -p anvilml --test db_startup_tests -- test_db_file_created_on_startup` exits 0 |
| `backend/tests/db_startup_tests.rs` | `test_migrations_create_required_tables` | Both `models` and `device_capabilities` tables exist in the created database. | `anvilml` binary compiled. Temp directory writable. | `ANVILML_DB_PATH` = temp file path, no subcommand (default path). | `sqlite_master` query returns both table names. | `cargo test -p anvilml --test db_startup_tests -- test_migrations_create_required_tables` exits 0 |

## CI Impact

No CI changes required. The new test file follows the existing convention: integration tests in `backend/tests/` are automatically collected by `cargo test -p anvilml` which runs in CI jobs (`rust-linux`, `rust-windows`). No new CI jobs, gates, or file patterns are introduced. The `tempfile` crate used by the tests is already available as a dev dependency (it is a transitive dependency of `sqlx` which `anvilml-registry` depends on).

## Platform Considerations

None identified. The `create_pool()` function creates directories via `std::fs::create_dir_all` and connects to SQLite — both are cross-platform. The test spawns the binary and checks the resulting file — `tempfile::NamedTempFile` is cross-platform. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `create_pool()`'s migration path `sqlx::migrate!("../../database/migrations")` uses a relative path from the `anvilml-registry` crate. When run from the `backend` binary, the embed path is resolved at compile time (not runtime), so this works correctly regardless of the binary's working directory. However, if the migration macro path is incorrect, compilation will fail immediately. | Low | High (build failure) | The path was verified by reading `crates/anvilml-registry/src/db.rs` line 61 — it uses `sqlx::migrate!("../../database/migrations")` which is correct for the crate's location. If compilation fails, the error will be explicit about the missing directory. |
| The test spawns the binary in the default (server) path, which blocks on `axum::serve()`. If the process does not start within the timeout, the test will fail without producing useful diagnostics. | Medium | Medium | Use a bounded timeout (5 seconds) and capture the process's stdout/stderr on timeout. The "listening" log line from `main.rs` confirms successful startup; absence of this line indicates a startup failure that can be diagnosed from captured stderr. |
| Adding `anvilml-registry` as a dependency increases the binary's compile time and potentially its binary size (adds `sqlx` + `sha2` + `digest` + `chrono` transitive deps). | Low | Low | This is the intended outcome — the binary needs database access. The compile-time cost is acceptable for the correctness gain. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml --features mock-hardware` exits 0 (dependency resolves, code compiles)
- [ ] `cargo test -p anvilml --test db_startup_tests` exits 0 (both integration tests pass)
- [ ] `cargo test -p anvilml` exits 0 (no regression in existing backend tests)
- [ ] `grep 'anvilml-registry' backend/Cargo.toml` returns at least one match (dependency was added)
- [ ] `grep 'create_pool' backend/src/main.rs` returns at least one match (pool creation was wired in)
