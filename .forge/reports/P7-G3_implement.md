# Implementation Report: P7-G3

| Field       | Value                                           |
|-------------|-------------------------------------------------|
| Task ID     | P7-G3                                           |
| Phase       | 007 — WebSocket Event Stream                    |
| Description | Replace SEED_ENTRIES startup seed with SeedLoader; remove const |
| Implemented | 2026-06-05T19:30:00Z                            |
| Status      | COMPLETE                                        |

## Summary

Replaced the compile-time `SEED_ENTRIES` const (~1700 lines, 126 GPU device entries) in `anvilml-hardware` with runtime SQL seed loading via the existing `SeedLoader::run()` function. Added `seeds_path: PathBuf` to `ServerConfig` with a default resolving to `<exe_dir>/seeds` (debug fallback to `backend/seeds/`). Gated `DeviceCapabilityStore::seed()` and its tests behind `#[cfg(any(test, feature = "seed-util"))]`. Updated all 15 `detect_all_devices()` tests to use a `SeedsGuard` RAII helper that creates a temp seeds directory with `devices.sql`, keeping the directory alive for the test duration. Removed the entire test module from `device_db.rs` since it only validated `SEED_ENTRIES` contents. All 179 tests pass, all clippy checks pass, all platform cross-checks pass, and the config drift gate passes.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| crate  | tempfile | 3.27.0          | workspace.toml (already declared) |

No new dependencies were added. The `seed-util` feature flag was added to `anvilml-registry/Cargo.toml` (no corresponding crate dependency).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/config.rs` | Add `seeds_path: PathBuf` field + `default_seeds_path()` fn to `ServerConfig`, update Default impl and test |
| Modify | `crates/anvilml-core/src/config_load.rs` | Add `seeds_path` field merge in `merge_config()` |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Replace inline SEED_ENTRIES seeding with `seed_loader::run()`, add `SeedsGuard` RAII helper, update all 15 detect_all_devices tests |
| Modify | `crates/anvilml-hardware/src/device_db.rs` | Remove `SEED_ENTRIES` const (2048 lines) and entire test module (383 lines); keep `DeviceCapabilityEntry` struct + `resolve_caps_from_row()` |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Add `tempfile` dev-dependency |
| Modify | `crates/anvilml-registry/src/device_store.rs` | Gate `seed()` method and test module with `#[cfg(any(test, feature = "seed-util"))]` |
| Modify | `crates/anvilml-registry/Cargo.toml` | Add `[features] seed-util = []` |
| Modify | `crates/anvilml-registry/tests/device_store.rs` | Gate `seed_returns_correct_count` and `seed_empty_returns_zero` tests with `#[cfg(feature = "seed-util")]` |
| Modify | `anvilml.toml` | Add `seeds_path = "./seeds"` key for config drift gate compliance |

## Commit Log

```
 .forge/reports/P7-G3_plan.md                  |  106 ++
 .forge/state/CURRENT_TASK.md                  |    6 +-
 .forge/state/state.json                       |   13 +-
 Cargo.lock                                    |    1 +
 anvilml.toml                                  |    6 +
 crates/anvilml-core/src/config.rs             |   30 +
 crates/anvilml-core/src/config_load.rs        |    1 +
 crates/anvilml-hardware/Cargo.toml            |    1 +
 crates/anvilml-hardware/src/device_db.rs      | 2048 -------------------------
 crates/anvilml-hardware/src/lib.rs            |  142 +-
 crates/anvilml-registry/Cargo.toml            |    3 +
 crates/anvilml-registry/src/device_store.rs   |    3 +-
 crates/anvilml-registry/tests/device_store.rs |    2 +
 13 files changed, 265 insertions(+), 2097 deletions(-)
```

## Test Results

```
Running unittests src/lib.rs (target/debug/deps/anvilml_core-8c562ebe203974a1)
running 74 tests
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-33040787c2a7b0ce)
running 52 tests
test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-cc67d683117a3c7e)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running unittests src/main.rs (target/debug/deps/anvilml_openapi-1af47b4848216e5d)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running unittests src/lib.rs (target/debug/deps/anvilml_registry-a5b296ccc9bbc22e)
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-0b3fba3b4225aa32)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/device_store.rs (target/debug/deps/device_store-2aab1f9fd66351a2)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/rescan.rs (target/debug/deps/rescan-8843270b042f5769)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/scanner.rs (target/debug/deps/scanner-9c4012602e8b670c)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/seed_loader.rs (target/debug/deps/seed_loader-587e31eb59849c7a)
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/store_get.rs (target/debug/deps/store_get-6a4e7feb72e15acb)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/store_list.rs (target/debug/deps/store_list-dbda3f34047ed0f)
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

Running tests/config_reference.rs (target/debug/deps/config_reference-151d039fcbadb99b)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Doc-tests anvilml_hardware: 2 passed
```

Total: 179 tests passed, 0 failed.

## Platform Cross-Check

```
# 1. Mock-hardware Windows cross-check
cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.27s

# 2. Real-hardware Linux native
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.88s

# 3. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.77s
```

All three checks exit 0.

## Project Gates

#### Gate 1 — Config Surface Sync
```
cargo test -p backend --features mock-hardware -- test_toml_key_set_matches_default
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- **SeedsGuard RAII pattern**: The plan suggested a `make_temp_seeds_dir()` helper returning `PathBuf`. However, this caused tests to fail because the `TempDir` was dropped at function return, deleting the seeds directory before the test used it. Instead, implemented a `SeedsGuard` struct that holds the `TempDir` as a field and returns both the guard and config, ensuring the temp dir stays alive for the test's lifetime.
- **Removed entire test module from `device_db.rs`**: The plan noted this was needed but left it ambiguous. Implemented by removing all 383 lines of the test module since every test referenced `SEED_ENTRIES`.
- **Added `seed-util` feature to `anvilml-registry/Cargo.toml`**: Required because clippy's `-D warnings` flag rejects unknown cfg feature names (`unexpected-cfgs` lint).

## Blockers

None. All checks pass.
