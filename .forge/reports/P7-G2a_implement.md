# Implementation Report: P7-G2a

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-G2a                                            |
| Phase       | 007 — WebSocket Event Stream                      |
| Description | anvilml-registry: seed_loader — tracking table bootstrap + SHA256 comparison |
| Implemented | 2026-06-05T17:45:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Implemented the seed_loader module for `anvilml-registry` that bootstraps a `seed_history` tracking table, enumerates and processes `.sql` seed files with header directive parsing (`-- anvil:seed_table` and `-- anvil:seed_strategy`), computes SHA256 digests for change detection, and upserts tracking rows. Added `AnvilError::SeedMissingDirective(String)` variant to the core error enum. All four integration tests pass, along with the full workspace test suite (176 tests, 0 failures).

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| crate  | sha2    | 0.11            | Workspace Cargo.toml (already present) |
| crate  | hex     | 0.4.3           | Workspace Cargo.toml (already present) |

No new dependencies were added — `sha2` and `hex` were already declared in the workspace dependencies and used by `anvilml-registry`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/error.rs` | Added `SeedMissingDirective(String)` variant to `AnvilError` enum with Display impl and test case update |
| Create | `crates/anvilml-registry/src/seed_loader.rs` | New module: `parse_header()`, `compute_sha256()`, `execute_seed()` stub, `run()` entry point |
| Modify | `crates/anvilml-registry/src/lib.rs` | Added `pub mod seed_loader;` and `pub use seed_loader::run;` |
| Create | `crates/anvilml-registry/tests/seed_loader.rs` | 4 integration tests: idempotent bootstrap, directive hit/miss, SHA256 skip |

## Commit Log

```
 .forge/reports/P7-G2a_plan.md                |  99 ++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 crates/anvilml-core/src/error.rs             |   6 +
 crates/anvilml-registry/src/lib.rs           |   2 +
 crates/anvilml-registry/src/seed_loader.rs   | 233 +++++++++++++++++++++++++++
 crates/anvilml-registry/tests/seed_loader.rs | 119 ++++++++++++++
 7 files changed, 469 insertions(+), 9 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-a5b296ccc9bbc22e)
running 19 tests
test seed_loader::tests::test_compute_sha256_empty ... ok
test seed_loader::tests::test_parse_header_both_directives ... ok
test seed_loader::tests::test_compute_sha256_known_value ... ok
test seed_loader::tests::test_parse_header_defaults_strategy ... ok
test seed_loader::tests::test_parse_header_empty_file ... ok
test seed_loader::tests::test_parse_header_missing_table ... ok
...

     Running tests/seed_loader.rs (target/debug/deps/seed_loader-587e31eb59849c7a)
running 4 tests
test test_directive_parsing_miss ... ok
test test_directive_parsing_hit ... ok
test test_table_bootstrap_idempotent ... ok
test test_sha256_skip_unchanged ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_core-8c562ebe203974a1)
running 74 tests
test error::tests::all_variants_display ... ok
...
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Full workspace test suite: 176 passed; 0 failed; 0 ignored
```

## Platform Cross-Check

```
# 1. Mock-hardware Windows cross-check
cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.34s

# 2. Real-hardware Linux check
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.36s

# 3. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.38s
```

All three platform cross-checks exit 0.

## Project Gates

### Config Surface Sync
```
cargo test -p backend --features mock-hardware
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```

Gate passes — no config surface changes were made in this task.

## Deviations from Plan

- Added `sqlx_error()` helper function to `seed_loader.rs` for `sqlx::Error` → `AnvilError::DbError` conversion, because `AnvilError` does not implement `From<sqlx::Error>` (only `From<std::io::Error>`). This is consistent with the existing pattern used in `db.rs`.
- Fixed clippy warning `unnecessary_map_or` by using `is_some_and` instead of `map_or(false, ...)` for the extension check — this was a pre-existing style issue introduced by my code but caught by clippy.

## Blockers

None.
