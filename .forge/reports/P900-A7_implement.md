# Implementation Report: P900-A7

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P900-A7                         |
| Phase         | 900 — Spec-Drift & Logging Retrofit |
| Description   | backend: wire SeedLoader::run() for database/seeds/devices.sql at startup |
| Implemented   | 2026-06-30T15:25:00Z            |
| Status        | COMPLETE                        |

## Summary

Wired `SeedLoader::run()` into `backend/src/main.rs` so the `anvilml` binary loads device
capability seed data from `database/seeds/devices.sql` at startup, immediately after
`create_pool()`. Added `ANVILML_SEED_PATH` environment variable override for testability.
Added three integration tests in `backend/tests/db_startup_tests.rs` verifying first-run
population (353 rows), idempotent second run, and missing-seed-file failure. Bumped
`backend/Cargo.toml` version from 0.1.7 to 0.1.8. Updated `docs/TESTS.md` with three
catalogue entries.

## Resolved Dependencies

None. This task introduces no new external crates or packages. It uses only dependencies
already present in `backend/Cargo.toml` (`anvilml-registry`, `tokio`, `sqlx`, `tempfile`,
`serial_test`). The `SeedLoader` API (`new()`, `run()`) was confirmed via direct source
inspection of `crates/anvilml-registry/src/seed_loader.rs`.

| Type   | Name              | Version verified | Source         |
|--------|-------------------|------------------|----------------|
| crate  | anvilml-registry  | 0.1.6 (path dep) | source         | n/a             |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/src/main.rs` | Add `SeedLoader::run()` call after `create_pool()`, add `ANVILML_SEED_PATH` env var override |
| Modify | `backend/tests/db_startup_tests.rs` | Add `seed_path()` helper and three integration tests; add `ANVILML_SEED_PATH` to existing tests |
| Modify | `backend/Cargo.toml` | Bump patch version 0.1.7 → 0.1.8 |
| Modify | `docs/TESTS.md` | Add three catalogue entries for new tests |

## Commit Log

```
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  13 +-
 Cargo.lock                        |   2 +-
 backend/Cargo.toml                |   2 +-
 backend/src/main.rs               |  27 +++++
 backend/tests/db_startup_tests.rs | 242 ++++++++++++++++++++++++++++++++++++++
 docs/TESTS.md                     |  36 ++++++
 7 files changed, 317 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/db_startup_tests.rs (target/debug/deps/db_startup_tests-66b98122ace96472)

running 5 tests
test tests::test_missing_seed_file_causes_startup_failure ... ok
test tests::test_migrations_create_required_tables ... ok
test tests::test_db_file_created_on_startup ... ok
test tests::test_seed_populates_device_capabilities ... ok
test tests::test_seed_idempotent_second_run ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

All workspace tests: 132 passed; 0 failed; 0 ignored
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.53s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.12s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.36s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.96s
```

## Project Gates

Gate 1 — Config Surface Sync:
```
cargo test -p anvilml --features mock-hardware -- config_reference
  test tests::config_reference_matches_defaults ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored
```

Gate 2 — OpenAPI Drift: Not triggered — task does not modify handler signatures,
`#[utoipa::path]` annotations, or `AppState` fields used in response types.

Gate 3 — Node Parity: Not triggered — task does not add, remove, or rename node types.

Gate 4 — Mock/Real Parity Markers: Not triggered — task does not add or modify
a node's `execute()` or an arch module's `load()`/`sample()`/`decode()`.

## Public API Delta

No new pub items introduced. The task only calls the already-pub `SeedLoader::new()`
and `SeedLoader::run()` from `main.rs` (a binary entry point, not a library crate).

## Deviations from Plan

1. **`ANVILML_SEED_PATH` type**: The plan specified `std::env::var("ANVILML_SEED_PATH")
   .ok().map(Path::new).unwrap_or(...)`, but this produces a type mismatch because
   `std::env::var()` returns `String` while `Path::new()` expects `&str`. Fixed by
   using `PathBuf` with a `match` expression: `match std::env::var("ANVILML_SEED_PATH")
   { Ok(path) => Path::new(&path).to_path_buf(), Err(_) => Path::new("...").to_path_buf() }`.

2. **Test seed path resolution**: The plan assumed tests spawn the binary from the repo
   root where `database/seeds/devices.sql` resolves. However, `cargo test` runs from the
   `backend/` directory. Fixed by adding a `seed_path()` helper function that computes
   the absolute path from `env!("CARGO_MANIFEST_DIR")` (which is `backend/`) and
   appending `../database/seeds/devices.sql`. This helper is used in all test spawns
   that need the seed file.

3. **Existing tests updated**: The plan did not mention modifying the two existing P900-A6
   tests (`test_db_file_created_on_startup`, `test_migrations_create_required_tables`),
   but these tests broke when `SeedLoader::run()` was added to `main.rs` because the seed
   file path was not set. Fixed by adding `ANVILML_SEED_PATH` to their spawn commands.

4. **`child.wait()` blocking**: The plan's `test_missing_seed_file_causes_startup_failure`
   used `timeout(Duration::from_secs(10), child.wait())`, but `child.wait()` is a
   synchronous call, not a future. Fixed by wrapping it in
   `tokio::task::spawn_blocking(move || child.wait())`.

## Blockers

None.
