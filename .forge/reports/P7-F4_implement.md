# Implementation Report: P7-F4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-F4                                              |
| Phase       | 007 — WebSocket Event Stream (Group F)             |
| Description | anvilml-hardware: detect_all_devices seeds and queries device_capabilities |
| Implemented | 2026-06-05T17:30:00Z                               |
| Status      | COMPLETE                                           |

## Summary

Made `detect_all_devices` in `crates/anvilml-hardware/src/lib.rs` async with a new `pool: &SqlitePool` parameter. At function entry, the device capability store is created and seeded with `SEED_ENTRIES`. Both Branch 2 (mock-hardware) and Branch 3 (real enumeration) now use `store.get()` for PCI-ID lookups instead of inline `SEED_ENTRIES.iter().find()`. Added `open_in_memory()` to `crates/anvilml-registry/src/db.rs` returning a single-connection fully-migrated in-memory pool. Updated `backend/src/main.rs` to open the database before hardware detection. All tests converted to `#[tokio::test]` with per-test in-memory pools.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source        |
|--------|-----------|-----------------|---------------|
| crate  | tokio     | (dev-dep added) | lockfile      |
| crate  | sqlx      | (workspace dep) | workspace     |

No new runtime dependencies were added. `tokio` was added as a dev-dependency for `#[tokio::test]` in `anvilml-hardware`. `SqlitePool` is re-exported from `anvilml-registry` via the existing workspace `sqlx` dependency.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/db.rs` | Added `pub async fn open_in_memory()` with single-connection pool for SQLite `:memory:` migration correctness |
| Modify | `crates/anvilml-registry/src/lib.rs` | Re-exported `SqlitePool` and `open_in_memory` |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Added `tokio` dev-dependency for `#[tokio::test]` |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Made `detect_all_devices` async with pool param; seeded store at entry; replaced inline SEED_ENTRIES lookups with `store.get()` in Branches 2 and 3; converted all tests to `#[tokio::test]`; added `#[serial]` to mock env-var tests to prevent pollution |
| Modify | `backend/src/main.rs` | Reordered: open DB before hardware detection; pass `&db` to `detect_all_devices` at both call sites; added `.await` |
| Modify | `crates/anvilml-server/src/lib.rs` | Updated `system_returns_200_with_hardware_info` test to use `open_in_memory()` and pass pool to async `detect_all_devices` |

## Commit Log

```
 .forge/reports/P7-F4_plan.md              | 106 +++++++++++++++++++
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  13 +--
 Cargo.lock                                |   1 +
 backend/src/main.rs                       |  20 ++--
 crates/anvilml-hardware/Cargo.toml        |   1 +
 crates/anvilml-hardware/src/lib.rs        | 210 +++++++++++++++++-------------
 crates/anvilml-registry/src/db.rs         |  37 +++++++
 crates/anvilml-registry/src/lib.rs        |   3 +-
 crates/anvilml-server/src/lib.rs          |   9 +-
 10 files changed, 295 insertions(+), 111 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-8c562ebe203974a1)
running 74 tests
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-3cac844b130828fb)
running 63 tests
test result: ok. 63 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-cc67d683117a3c7e)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml_openapi-1af47b4848216e5d)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-a5b296ccc9bbc22e)
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-0b3fba3b4225aa32)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/device_store.rs (target/debug/deps/device_store-2aab1f9fd66351a2)
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/rescan.rs (target/debug/deps/rescan-8843270b042f5769)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner.rs (target/debug/deps/scanner-9c4012602e8b670c)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_get.rs (target/debug/deps/store_get-6e7feb72e15acb)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_list.rs (target/debug/deps/store_list-dbda31f34047ed0f)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-14998e2438e24622)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-13e7c761293bb089)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_models.rs (target/debug/deps/api_models-9da6b09b63325b67)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_events.rs (target/debug/deps/api_ws_events-abc20fdd727bce00)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-d87253a66ce7d0a9)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-afb92a072e8e1596)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-7ddf479867cb1a11)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_core
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_hardware
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Total: 174 passed; 0 failed
```

## Platform Cross-Check

```
# 1. Mock-hardware Windows cross-check
cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.32s

# 2. Real-hardware Linux native
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.25s

# 3. Real-hardware Windows-gnu cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.67s
```

All three checks exited 0.

## Project Gates

```
# Config drift gate
cargo test -p backend --features mock-hardware -- test_toml_key_set_matches_default
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate passed.

## Deviations from Plan

1. **`open_in_memory()` uses single-connection pool**: The plan specified `open(Path::new(":memory:"))` but SQLite's `:memory:` databases are per-connection, so the migrator would run on one connection while queries hit another in a multi-connection pool. Used `.max_connections(1)` to ensure the migrator and queries share the same in-memory database.

2. **`#[serial]` retained on mock env-var tests**: The plan said to remove `#[serial]` from all mock tests. However, `MockDetector` reads from global environment variables (`ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`) at runtime. Without `#[serial]`, parallel mock test execution causes env var pollution (e.g., one test sets VRAM=32768 while another expects 12288). Retained `#[serial]` on the 6 tests that set these env vars.

3. **`SqlitePool` re-exported from `anvilml-registry`**: The plan assumed `sqlx::SqlitePool` would be accessible in `anvilml-hardware`, but it's not a direct dependency. Re-exported `SqlitePool` from `anvilml-registry` instead.

4. **Fixed test in `anvilml-server/src/lib.rs`**: The existing `system_returns_200_with_hardware_info` test called `detect_all_devices` with only one argument. Updated to use `open_in_memory()` and pass the pool. This was not explicitly listed but was required for compilation.

## Blockers

None.
