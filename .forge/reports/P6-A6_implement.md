# Implementation Report: P6-A6

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P6-A6                           |
| Phase         | 006 — Model Registry & Artifacts |
| Description   | anvilml-registry: SeedLoader hash-check + bookkeeping table |
| Implemented   | 2026-06-29T21:05:00Z            |
| Status        | COMPLETE                        |

## Summary

Created `crates/anvilml-registry/src/seed_loader.rs` implementing the `SeedLoader` struct
with `new()` constructor and `already_applied()` method. The `already_applied()` method
creates the `_seed_log` bookkeeping table lazily via `CREATE TABLE IF NOT EXISTS`, then
compares a seed file's SHA256 hash against stored values to determine idempotency.
Updated `lib.rs` to declare the new module. Created four integration tests in
`tests/seed_loader_tests.rs` covering unseen seeds, hash mismatches, hash matches, and
lazy table creation. Version bumped from 0.1.4 to 0.1.5. No `pub use` re-export was
added — that is deferred to P6-A9 as planned.

## Resolved Dependencies

None. The `sqlx` crate with `sqlite` feature is already declared in
`crates/anvilml-registry/Cargo.toml`. No new dependencies were added.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/seed_loader.rs` | SeedLoader struct, new(), already_applied(), _seed_log table creation |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Added `pub mod seed_loader;` |
| CREATE | `crates/anvilml-registry/tests/seed_loader_tests.rs` | 4 integration tests |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.4 → 0.1.5 |
| MODIFY | `docs/TESTS.md` | Added 4 test catalogue entries |

## Commit Log

```
 .forge/reports/P6-A6_plan.md                       | 149 +++++++++++++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  13 +-
 Cargo.lock                                         |   2 +-
 crates/anvilml-registry/Cargo.toml                 |   2 +-
 crates/anvilml-registry/src/lib.rs                 |   1 +
 crates/anvilml-registry/src/seed_loader.rs         | 116 ++++++++++++++
 crates/anvilml-registry/tests/seed_loader_tests.rs | 176 +++++++++++++++++++++
 docs/TESTS.md                                      |  48 ++++++
 9 files changed, 502 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/seed_loader_tests.rs (target/debug/deps/seed_loader_tests-1439da9fcf420485)

running 4 tests
test test_already_applied_unseen_seed_returns_false ... ok
test test_seed_log_created_on_first_use ... ok
test test_already_applied_hash_match_returns_true ... ok
test test_already_applied_hash_mismatch_returns_false ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Doc-tests anvilml_registry

running 2 tests
test crates/anvilml-registry/src/scanner.rs - scanner::ModelScanner (line 26) - compile ... ok
test crates/anvilml-registry/src/seed_loader.rs - seed_loader::SeedLoader (line 29) - compile ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 0 failures across all crates (143 tests total, all passed).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.85s
CHECK 1: OK

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.85s
CHECK 2: OK

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.63s
CHECK 3: OK

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.02s
CHECK 4: OK
```

All four platform cross-checks passed.

## Project Gates

### Gate 1 — Config Surface Sync

```
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passed. No config fields were added or modified by this task.

### Gate 2 — OpenAPI Drift

Not triggered — this task does not modify handler function signatures, `#[utoipa::path]`
annotations, or `AppState` fields.

### Gate 3 — Node Parity

Not triggered — this task does not add, remove, or rename a node type.

### Gate 4 — Mock/Real Parity Markers

Not triggered — this task does not add or modify a node's `execute()` or an arch
module's `load()`/`sample()`/`decode()`/`compute_latent_shape()`.

## Public API Delta

```
# From lib.rs:
+pub mod seed_loader;

# From seed_loader.rs:
pub struct SeedLoader { ... }
pub fn new(pool: SqlitePool) -> Self
pub async fn already_applied(&self, seed_name: &str, sha256: &str) -> Result<bool, AnvilError>
```

All new `pub` items match the plan's Public API Surface table. No `pub use` re-export
was added — deferred to P6-A9 as planned.

## Deviations from Plan

1. **Test import path**: The plan's approach listed `use anvilml_registry::SeedLoader;`
   in the test file, but since `pub use seed_loader::SeedLoader;` is deferred to P6-A9,
   the test file uses `use anvilml_registry::seed_loader::SeedLoader;` instead. This is
   the correct pattern for integration tests when the re-export is not yet present.

2. **Doctest fix**: The initial doctest in `seed_loader.rs` used
   `use anvilml_registry::{SeedLoader, create_pool};` which failed because the re-export
   is absent. Fixed to use `use anvilml_registry::seed_loader::SeedLoader;` and
   `use anvilml_registry::create_pool;` separately.

3. **4 tests instead of 3**: The plan specified >=3 tests; I wrote 4 tests (the 3
   specified in the plan plus `test_already_applied_hash_match_returns_true` which
   verifies the `Ok(true)` return case — the plan's Tests table only covered the
   `Ok(false)` cases). This strengthens test coverage without adding complexity.

## Blockers

None.
