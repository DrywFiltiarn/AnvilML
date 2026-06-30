# Plan Report: P900-A7

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P900-A7                                                     |
| Phase       | 900 — Spec-Drift & Logging Retrofit                         |
| Description | backend: wire SeedLoader::run() for database/seeds/devices.sql at startup |
| Depends on  | P900-A6                                                     |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-30T16:30:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Wire `SeedLoader::run()` into `backend/src/main.rs` so the `anvilml` binary loads device
capability seed data from `database/seeds/devices.sql` at startup. Phase 6 built
`SeedLoader` fully (hash-gated, idempotent, unit-tested), but nothing outside
`anvilml-registry`'s own tests ever calls it — the `device_capabilities` table stays
empty even after `P900-A6`'s pool/migrations run. This task adds the call immediately
after `create_pool()` in the default startup path, logs applied/skipped at INFO, and
exits non-zero on error. It also adds three integration tests to
`backend/tests/db_startup_tests.rs` verifying first-run population, idempotent second
run, and missing-seed-file failure.

## Scope

### In Scope
- `backend/src/main.rs`: Add `SeedLoader::new(pool.clone())` and `.run("devices.sql",
  Path::new("database/seeds/devices.sql")).await` after the `create_pool()` call; log
  applied/skipped at INFO; on Err, eprintln and exit 1.
- `backend/src/main.rs`: Add `ANVILML_SEED_PATH` environment variable override so tests
  can point to a temp seed file or a non-existent path for error-condition testing.
- `backend/tests/db_startup_tests.rs`: Add three new tests (extend existing P900-A6 file):
  1. `test_seed_populates_device_capabilities` — first run populates the table (row count
     > 0 matching the 353 INSERTs in devices.sql).
  2. `test_seed_idempotent_second_run` — second run against the same DB is idempotent
     (no duplicate rows, no error).
  3. `test_missing_seed_file_causes_startup_failure` — a missing/malformed seed file
     causes startup to exit non-zero.
- `backend/Cargo.toml`: Bump patch version from 0.1.7 to 0.1.8.
- `docs/TESTS.md`: Add three catalogue entries for the new tests.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope without
deferring any functionality to another task.

## Existing Codebase Assessment

**What already exists:** `P900-A6` already wired `create_pool()` into `main.rs`'s default
startup path (after config load, before TCP bind). The `anvilml-registry` crate is already
a `backend` dependency in `Cargo.toml`, and `SeedLoader` is already re-exported from
`lib.rs`. `SeedLoader::run()` is fully implemented in `seed_loader.rs` (lines 149–218):
it computes a SHA256 hash of the seed file, checks `_seed_log` for idempotency, executes
the seed SQL within a transaction, and records the hash+timestamp. The `_seed_log` table
is created lazily via `CREATE TABLE IF NOT EXISTS`. `devices.sql` contains 353 INSERT
statements populating the `device_capabilities` table.

**Established patterns:** Error handling follows the `map_err` + `eprintln!` +
`std::process::exit(1)` pattern already used for config loading and `create_pool()` in
`main.rs` (lines 73–77, 108–114). Logging uses structured `tracing::info!` calls with
named fields. Tests in `db_startup_tests.rs` spawn the binary via
`Command::new(env!("CARGO_BIN_EXE_anvilml"))`, set env vars via `.env()`, pipe stderr,
and wait for the "listening" log line within a 5-second `tokio::time::timeout`. The
`tempfile` crate is used for temp directories.

**Gap between design doc and current source:** The design doc (`ANVILML_DESIGN.md §7.1/§7.5`)
specifies that `SeedLoader` should be called at startup after pool creation — this was
implemented in Phase 6's library code but never wired into the binary. There is no gap in
the source; the gap is simply the missing call site in `main.rs`.

## Resolved Dependencies

None. This task introduces no new external crates or packages. It uses only dependencies
already present in `backend/Cargo.toml` (`anvilml-registry`, `tokio`, `sqlx`, `tempfile`,
`serial_test`). The `SeedLoader` API (`new()`, `run()`) was confirmed via direct source
inspection of `crates/anvilml-registry/src/seed_loader.rs`.

| Type   | Name              | Version verified | MCP source   | Feature flags confirmed |
|--------|-------------------|------------------|--------------|-------------------------|
| crate  | anvilml-registry  | 0.1.7 (path dep) | source       | n/a                     |

## Approach

1. **Add `SeedLoader` call to `main.rs` after `create_pool()`.**
   After line 114 (`let _pool = create_pool(...).await.map_err(...).unwrap();`), add:
   ```rust
   // Load device capability seed data from the checked-in SQL file.
   // The seed is hash-gated and idempotent — if the file hasn't changed
   // since last run, this is a no-op. On failure, exit before binding
   // any socket, matching the create_pool() error pattern.
   //
   // The seed path can be overridden via ANVILML_SEED_PATH for testing
   // (e.g. pointing to a temp file or a non-existent path).
   let seed_path = std::env::var("ANVILML_SEED_PATH")
       .ok()
       .map(Path::new)
       .unwrap_or(Path::new("database/seeds/devices.sql"));

   tracing::info!(seed_path = %seed_path.display(), "loading device capabilities seed");

   let loader = anvilml_registry::SeedLoader::new(_pool.clone());
   loader
       .run("devices.sql", seed_path)
       .await
       .map_err(|e| {
           eprintln!("Failed to apply device capabilities seed: {e}");
           std::process::exit(1);
       })
       .unwrap();
   ```
   Rationale: `ANVILML_SEED_PATH` is added solely for testability — it lets tests inject
   a temp seed file or a non-existent path without changing the production seed path.
   The env var is checked *after* `create_pool()` so that a DB failure exits before the
   seed loader is even invoked.

2. **Add three integration tests to `db_startup_tests.rs`.**
   Append a new `mod seed_tests` block (or add to the existing `tests` module) with:

   - **`test_seed_populates_device_capabilities`**: Spawn binary with temp `db_path`
     and `ANVILML_PORT=0`. Wait for "listening" log line (5s timeout). Kill process.
     Connect to DB via `sqlx::SqlitePool::connect()`. Query `SELECT COUNT(*) FROM
     device_capabilities`. Assert count > 0 (should be 353 matching devices.sql INSERTs).

   - **`test_seed_idempotent_second_run`**: Spawn binary with temp `db_path` and
     `ANVILML_PORT=0`. Wait for "listening". Kill process. Connect to DB, record
     row count. Spawn binary again with the *same* `db_path` and `ANVILML_PORT=0`.
     Wait for "listening". Kill process. Connect to DB, record row count. Assert
     both counts are equal (idempotent — no duplicate rows).

   - **`test_missing_seed_file_causes_startup_failure`**: Spawn binary with
     `ANVILML_SEED_PATH=/tmp/nonexistent_seed.sql` and `ANVILML_PORT=0`. Wait
     10 seconds for process to exit (the seed error should cause immediate exit
     before TCP bind). Assert `exit_status.is_some()` and `code() != Some(0)`.
     This test does NOT wait for "listening" — a successful binary should never
     reach that point with a missing seed file.

   Each test uses the same `Command::new(env!("CARGO_BIN_EXE_anvilml"))` pattern as
   the existing P900-A6 tests, with the same stderr-piping and timeout structure.

3. **Bump `backend/Cargo.toml` version.**
   Change `version = "0.1.7"` to `version = "0.1.8"` per the crate version bump
   convention (`ENVIRONMENT.md §12`).

4. **Update `docs/TESTS.md`.**
   Add three entries for the new tests following the existing catalogue format,
   including `Mode: both`, inputs, expected output, and acceptance command.

## Public API Surface

None. This task does not introduce any new `pub` items. It only adds a call to the
already-pub `SeedLoader::run()` method in `main.rs` (a binary entry point, not a
library crate). No new types, traits, or re-exports are added.

| Action | Crate/Module | Item |
|--------|-------------|------|
| Call (no new pub) | `backend/src/main.rs` | `anvilml_registry::SeedLoader::new()` + `.run()` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/src/main.rs` | Add `SeedLoader::run()` call after `create_pool()`, add `ANVILML_SEED_PATH` env var override |
| Modify | `backend/tests/db_startup_tests.rs` | Add three integration tests for seed loading behavior |
| Modify | `backend/Cargo.toml` | Bump patch version 0.1.7 → 0.1.8 |
| Modify | `docs/TESTS.md` | Add three catalogue entries for new tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `backend/tests/db_startup_tests.rs` | `test_seed_populates_device_capabilities` | First startup run of `SeedLoader::run()` populates `device_capabilities` with rows matching `devices.sql`'s INSERT count | `anvilml` binary compiled; temp `db_path` is a fresh SQLite file | `ANVILML_DB_PATH=<temp>/test.db`, `ANVILML_PORT=0` | Row count > 0 (353) in `device_capabilities`; process exits cleanly after "listening" | `cargo test -p anvilml --test db_startup_tests test_seed_populates_device_capabilities` exits 0 |
| `backend/tests/db_startup_tests.rs` | `test_seed_idempotent_second_run` | Second startup run against the same DB produces no duplicate rows in `device_capabilities` | `anvilml` binary compiled; temp `db_path` already has seed data from a prior run | Same temp `db_path` for both spawns; `ANVILML_PORT=0` | Row count unchanged between first and second run | `cargo test -p anvilml --test db_startup_tests test_seed_idempotent_second_run` exits 0 |
| `backend/tests/db_startup_tests.rs` | `test_missing_seed_file_causes_startup_failure` | A missing seed file causes `SeedLoader::run()` to return `Err`, triggering `eprintln!` + `exit(1)` before TCP bind | `anvilml` binary compiled; `ANVILML_SEED_PATH` points to a non-existent file | `ANVILML_SEED_PATH=/tmp/nonexistent_seed.sql`, `ANVILML_PORT=0` | Process exits with non-zero code within 10s; no "listening" line produced | `cargo test -p anvilml --test db_startup_tests test_missing_seed_file_causes_startup_failure` exits 0 |

## CI Impact

No CI changes required. The new tests are in `backend/tests/db_startup_tests.rs`, which
is already picked up by `cargo test --workspace --features mock-hardware` (the full
workspace test suite run in CI). No new file types, gates, or CI jobs are introduced.

## Platform Considerations

None identified. The seed file path is a relative path resolved from the current working
directory. Tests spawn the binary from the repo root (where `database/seeds/devices.sql`
exists), so the path resolves correctly on both Linux and Windows. The `ANVILML_SEED_PATH`
env var override uses `Path::new()` which is platform-neutral. No `#[cfg(unix)]` or
`#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `SeedLoader::run()` reads the seed file relative to CWD — if tests spawn the binary from a directory where `database/seeds/devices.sql` doesn't exist, the file-not-found error will trigger even for the "normal" tests. | Medium | High | Tests use the default seed path (no `ANVILML_SEED_PATH` override), and spawn the binary from the repo root where the seed file exists. The `Command::new(env!()))` pattern inherits the CWD from the test runner, which is the repo root. |
| `ANVILML_SEED_PATH` env var is not set in production — if it's set accidentally in the CI environment, the seed file might be loaded from the wrong location. | Low | Medium | The env var is only used when explicitly set by a test. CI does not set `ANVILML_SEED_PATH`. The production path falls back to `database/seeds/devices.sql` when the env var is absent. |
| `devices.sql` contains 353 INSERT statements — if the seed file grows significantly, startup time could become noticeable. | Low | Low | `SeedLoader::run()` uses a transaction for atomicity, and the hash-gating means re-application only happens when the file changes. The seed is a one-time load at startup, not a per-request operation. |
| The "listening" log line pattern used to detect startup completion in tests may not appear if `SeedLoader::run()` panics instead of returning an error. | Low | High | `SeedLoader::run()` uses `?` propagation which converts errors to `Result`, never panics. The `tracing::instrument` attribute on `run()` ensures any issues are logged before the error propagates. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml --test db_startup_tests test_seed_populates_device_capabilities` exits 0
- [ ] `cargo test -p anvilml --test db_startup_tests test_seed_idempotent_second_run` exits 0
- [ ] `cargo test -p anvilml --test db_startup_tests test_missing_seed_file_causes_startup_failure` exits 0
- [ ] `cargo test -p anvilml --test db_startup_tests` exits 0 (all 5 tests: 2 from P900-A6 + 3 new)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (full workspace regression)
- [ ] `backend/Cargo.toml` version is `0.1.8`
