# Implementation Report: P6-A5

| Field       | Value                                                                 |
|-------------|-----------------------------------------------------------------------|
| Task ID     | P6-A5                                                                 |
| Phase       | 006 — Model Registry                                                  |
| Description | anvilml: initial model scan at startup + registry in AppState         |
| Implemented | 2026-06-04T08:45:00Z                                                   |
| Status      | COMPLETE                                                              |

## Summary

Integrated the `ModelRegistry` into AnvilML's application lifecycle. Added a `pub registry: Arc<ModelRegistry>` field to `AppState`, updated both constructors (`new()` and `new_with_hardware()`) to accept an optional registry parameter, and updated the `Clone` impl. In `backend/src/main.rs`, after DB open and ghost-job reset, the registry is created, wrapped in `Arc`, a non-blocking tokio task is spawned for the initial model directory rescan (logging the count on success or warning on failure), and the registry Arc is passed to `AppState::new_with_hardware()`. All existing tests pass with the updated signatures.

## Resolved Dependencies

| Type   | Name              | Version resolved | Source        |
|--------|-------------------|-----------------|---------------|
| crate  | anvilml-registry  | 0.1.0 (local)   | workspace     |
| crate  | sqlx              | 0.9             | Cargo.lock    |
| crate  | tokio             | 1               | Cargo.lock    |

No new dependencies were added. The `anvilml-registry` crate already re-exports `ModelRegistry` in its public API and is a transitive dependency of both `anvilml-server` and `backend`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Add `pub registry: Arc<ModelRegistry>` field; update `new()` and `new_with_hardware()` to accept `Option<Arc<ModelRegistry>>`; update `Clone` impl |
| Modify | `backend/src/main.rs` | Create `ModelRegistry`, spawn background rescan task, pass registry to `AppState::new_with_hardware()` |
| Modify | `crates/anvilml-server/src/lib.rs` | Update 3 test functions to pass new `registry` parameter (`None`) |
| Modify | `crates/anvilml-registry/tests/rescan.rs` | Formatting fix (whitespace reformat by `cargo fmt --all`) |

## Commit Log

```
 backend/src/main.rs                     | 16 +++++++++++++++-
 crates/anvilml-registry/tests/rescan.rs |  8 ++++++--
 crates/anvilml-server/src/lib.rs        |  7 +++----
 crates/anvilml-server/src/state.rs      | 29 ++++++++++++++++++++++++++++-
 4 files changed, 52 insertions(+), 8 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-76fc372595dda5e4)

running 74 tests
test config::tests::test_device_type_default ... ok
test config::tests::test_model_kind_default ... ok
... (all 74 passed)
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-91331d83c93bb7d6)

running 59 tests
test cpu::tests::cpu_detect_returns_one_device ... ok
... (all 59 passed)
test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-9d39e30982bb9c7f)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml_openapi-2a30d10dc6863b45)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-c7f74e4b29473496)
running 10 tests
test scanner::tests::test_infer_dtype_case_insensitive ... ok
... (all 10 passed)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/anvilml_registry_db.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/rescan.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_get.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_list.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-6569c1b9eba5df84)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-bbcb80266b6dbd85)
running 3 tests
test tests::health_returns_200 ... ok
test tests::env_returns_200_with_stub_report ... ok
test tests::system_returns_200_with_hardware_info ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-72ee4379635c8b26)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-7286875b72a88e2f)
running 8 tests
test cli::tests::test_args_to_overrides_all_none ... ok
... (all 8 passed)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_core
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_hardware
running 2 tests
test crates/anvilml-hardware/src/sysfs.rs - sysfs::read_vram_from_amdgpu_sysfs ... ok
test crates/anvilml-hardware/src/sysfs.rs - sysfs::parse_pci_id ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_ipc / anvilml_registry / anvilml_scheduler / anvilml_server / anvilml_worker
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

TOTAL: 162 tests passed, 0 failed
```

## Platform Cross-Check

```
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.32s
```

Windows cross-check (x86_64-pc-windows-gnu): PASSED — zero errors.

## Project Gates

- **Config drift gate** (`cargo test -p backend --features mock-hardware -- test_toml_key_set_matches_default`): PASSED — 1 test passed, 0 failed.
- **Clippy** (`cargo clippy --workspace --features mock-hardware -- -D warnings`): PASSED — zero warnings.

## Deviations from Plan

1. **Constructor fallback logic**: The plan specified "converting `None` to `Arc::new(ModelRegistry::new(pool))` when db is `Some`, or empty default otherwise." Since `SqlitePool` does not implement `Default`, the actual implementation uses a three-way match: `(Some(r), _) => r`, `(None, Some(pool)) => Arc::new(ModelRegistry::new(pool.clone()))`, `(None, None) => Arc::new(ModelRegistry::new(SqlitePool::connect_lazy("sqlite::memory:")))`. The `connect_lazy` synchronous fallback enables tests that construct `AppState` without a database to still create a functional (in-memory) registry without requiring async constructors.

2. **Registry wrapping in main.rs**: The plan used `registry.clone()` on the bare `ModelRegistry`, but `ModelRegistry` does not implement `Clone`. The implementation wraps the registry in `Arc::new()` first, then uses `Arc::clone(&registry)` for both the spawn task and the `AppState` constructor.

3. **Test updates**: The plan suggested tests would need to be updated to pass a registry parameter. All three existing tests (`health_returns_200`, `env_returns_200_with_stub_report`, `system_returns_200_with_hardware_info`) were updated to pass `None` for the registry, relying on the synchronous in-memory pool fallback in the constructors.

## Blockers

None. All gates passed, all tests pass (162/162), Windows cross-check passes, clippy reports zero warnings.
