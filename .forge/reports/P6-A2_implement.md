# Implementation Report: P6-A2

| Field       | Value                                           |
|-------------|-------------------------------------------------|
| Task ID     | P6-A2                                           |
| Phase       | 006 — Model Registry                            |
| Description | anvilml-registry: ModelRegistry store (upsert, get) |
| Implemented | 2026-06-04T01:05:00Z                            |
| Status      | COMPLETE                                        |

## Summary

Implemented `ModelRegistry` struct in `crates/anvilml-registry/src/store.rs` backed by a SQLite `SqlitePool`, providing `upsert` (INSERT OR REPLACE) and `get` (single model lookup by ID) methods. Added `pub mod store` and `pub use store::ModelRegistry` to `lib.rs`. Created integration tests in `tests/store_get.rs` verifying upsert+get roundtrip and get-missing-returns-none. Added `chrono` and `serde_json` as direct dependencies to the crate's `Cargo.toml`.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source        |
|--------|-----------|-----------------|---------------|
| crate  | chrono    | 0.4             | Lockfile      |
| crate  | serde_json| 1               | Lockfile      |

Note: MCP servers were not queried — these dependencies were already present in `anvilml-core/Cargo.toml` and the project's lockfile. Added as direct dependencies to `anvilml-registry/Cargo.toml` since they are used directly in `store.rs`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-registry/src/store.rs` | ModelRegistry struct with upsert and get methods |
| Edit   | `crates/anvilml-registry/src/lib.rs` | Added `pub mod store;` and `pub use store::ModelRegistry;` |
| Create | `crates/anvilml-registry/tests/store_get.rs` | Integration tests: upsert+get roundtrip, get missing returns None |
| Edit   | `crates/anvilml-registry/Cargo.toml` | Added `chrono` and `serde_json` dependencies |

## Commit Log

```
 .forge/reports/P6-A2_plan.md               |  99 +++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  13 ++--
 Cargo.lock                                 |   2 +
 crates/anvilml-registry/Cargo.toml         |   2 +
 crates/anvilml-registry/src/lib.rs         |   2 +
 crates/anvilml-registry/src/store.rs       | 103 +++++++++++++++++++++++++++++
 crates/anvilml-registry/tests/store_get.rs |  63 ++++++++++++++++++
 8 files changed, 281 insertions(+), 9 deletions(-)
```

## Test Results

```
   Compiling anvilml-core v0.1.0 (/home/dryw/AnvilML/crates/anvilml-core)
   Compiling anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.54s
     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-615a18672cfc98e4)

running 10 tests
test scanner::tests::test_infer_dtype_case_insensitive ... ok
test scanner::tests::test_infer_dtype_matches ... ok
test scanner::tests::test_infer_kind_case_insensitive ... ok
test scanner::tests::test_infer_dtype_unknown ... ok
test scanner::tests::test_infer_kind_fallback ... ok
test scanner::tests::test_infer_kind_matches ... ok
test scanner::tests::test_sha256_hex ... ok
test scanner::tests::test_vram_estimate_mib ... ok
test db::tests::test_open_creates_tables ... ok
test db::tests::test_reset_ghost_jobs ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-2b7e472f6908112e)

running 1 test
test test_open_creates_tables ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/scanner.rs (target/debug/deps/scanner-8898d730737d15e0)

running 1 test
test test_scan_dirs_two_files ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/store_get.rs (target/debug/deps/store_get-b0750b87feb8ed55)

running 2 tests
test test_get_missing_returns_none ... ok
test test_upsert_then_get_returns_equal_meta ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Doc-tests anvilml_registry

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Platform Cross-Check

```
    Checking anvilml-core v0.1.0 (/home/dryw/AnvilML/crates/anvilml-core)
    Checking anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.43s
```

## Project Gates

### Config Surface Sync (Gate 1)
```
     Running tests/config_reference.rs (target/debug/deps/config_reference-1c602ac5823733ee)

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Deviations from Plan

- Used `sqlx::query()` (non-macro) instead of `sqlx::query!` for the INSERT statement, since the project does not have a `DATABASE_URL` set and no sqlx query cache exists. This matches the pattern used in the existing `db.rs` module.
- Used `sqlx::query_as` with a typed tuple and a `type ModelRow` alias instead of `sqlx::query!` for the SELECT, to avoid the macro's compile-time schema requirement. Added `#[allow(clippy::type_complexity)]` was not needed — instead defined a named type alias `ModelRow` for the 8-field tuple, which satisfied clippy's `type_complexity` lint.
- Added `chrono` and `serde_json` as direct dependencies to `anvilml-registry/Cargo.toml` (not listed in the plan's files affected), since `store.rs` uses them directly for RFC3339 serialization/deserialization and JSON enum deserialization.

## Blockers

None.
