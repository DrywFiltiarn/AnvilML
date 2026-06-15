# Implementation Report: P5-A2

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P5-A2                              |
| Phase         | 005 — SQLite Persistence           |
| Description   | SeedLoader SHA256-gated SQL seed runner |
| Implemented   | 2026-06-15T15:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Implemented the SHA256-gated SQL seed loader in `crates/anvilml-registry/src/seed_loader.rs` that discovers `.sql` files in a configurable directory, computes SHA256 of each file's content, compares against the `seed_history` table, and either skips (up-to-date) or executes + records (changed or new). Created the initial seed file `backend/seeds/devices.sql` with 353 `INSERT OR IGNORE INTO device_capabilities` rows for all entries in `docs/SUPPORTED_DEVICES_DB.md`. Added `sha2` and `chrono` dependencies to the crate. Both new tests pass: first-run applies seeds, second-run skips them.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source              |
|--------|---------|-----------------|---------------------|
| crate  | sha2    | 0.10.9          | Cargo.lock (MCP unavailable, lockfile fallback) |
| crate  | chrono | 0.4.45          | Workspace dependency (already declared) |

The `sha2` crate 0.10.9 was already present in `Cargo.lock` as a transitive dependency (via `digest 0.10.7`). The `chrono` crate was added from the workspace dependency (`chrono = { version = "0.4.45", features = ["serde"] }`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/seed_loader.rs` | SeedLoader: SHA256-gated SQL seed runner with `pub async fn run()` |
| CREATE | `crates/anvilml-registry/tests/seed_loader_tests.rs` | Tests: apply-new-seed and skip-up-to-date |
| CREATE | `backend/seeds/devices.sql` | Device capability seed data (353 INSERT statements from SUPPORTED_DEVICES_DB.md) |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Added `pub mod seed_loader;` and `pub use seed_loader::run;` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Added `sha2 = "0.10"`, `chrono` dep; bumped version 0.1.1 → 0.1.2 |
| MODIFY | `docs/TESTS.md` | Added entries for `test_seed_loader_applies_new_seed` and `test_seed_loader_skips_up_to_date` |

## Commit Log

```
 .forge/reports/P5-A2_plan.md                       | 124 +++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  13 +-
 Cargo.lock                                         |   4 +-
 backend/seeds/devices.sql                          | 362 +++++++++++++++++++++
 crates/anvilml-registry/Cargo.toml                 |   8 +-
 crates/anvilml-registry/src/lib.rs                 |   2 +
 crates/anvilml-registry/src/seed_loader.rs         | 142 ++++++++
 crates/anvilml-registry/tests/seed_loader_tests.rs | 156 +++++++++
 docs/TESTS.md                                      |  18 +
 10 files changed, 823 insertions(+), 12 deletions(-)
```

## Test Results

```
     Running tests/seed_loader_tests.rs (target/debug/deps/seed_loader_tests-5b0328cf427b5b2f)

running 2 tests
test test_seed_loader_skips_up_to_date ... ok
test test_seed_loader_applies_new_seed ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 80 tests passed, 0 failed.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
CHECK 1 PASSED — cargo check --workspace --features mock-hardware
CHECK 2 PASSED — cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
CHECK 3 PASSED — cargo check --bin anvilml
CHECK 4 PASSED — cargo check --bin anvilml --target x86_64-pc-windows-gnu
```

## Project Gates

```
Gate 1 (config_reference): PASSED
  cargo test -p anvilml --features mock-hardware -- config_reference
  test config_reference ... ok
  test result: ok. 1 passed; 0 failed
```

Gate 2 (OpenAPI drift) and Gate 3 (Node parity) not triggered — task does not modify handler signatures, node types, or the OpenAPI generation binary.

## Public API Delta

```
+pub mod seed_loader;
+pub use seed_loader::run;
```

New pub items:
- `pub mod seed_loader` — module path: `anvilml_registry::seed_loader`
- `pub use seed_loader::run` — re-exported function: `anvilml_registry::run` → `pub async fn run(pool: &SqlitePool, seeds_path: &Path) -> Result<(), AnvilError>`

## Deviations from Plan

- **sqlx::query vs sqlx::query_file**: The plan referenced `sqlx::query_file` but this is a compile-time macro that requires the SQL file path at build time. The implementation uses `sqlx::query` with `AssertSqlSafe` wrapping the loaded SQL text, which is the correct runtime approach for dynamically-discovered seed files.
- **sha2::Digest trait import**: The test file requires `use sha2::Digest;` to bring the `Digest` trait into scope for `Sha256::digest()` — added as a dev-dependency import.
- **clippy::unnecessary-map-or**: Fixed `map_or(false, |ext| ...)` to `is_some_and(|ext| ...)` per clippy lint.
- **PathBuf unused import**: Removed unused `PathBuf` import from `std::path` in seed_loader.rs.

## Blockers

None.
