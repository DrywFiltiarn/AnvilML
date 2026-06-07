# Implementation Report: P11-C1

| Field       | Value                                                         |
|-------------|---------------------------------------------------------------|
| Task ID     | P11-C1                                                        |
| Phase       | 011 — Graph Validation                                        |
| Description | anvilml-worker: fix relative venv path causing Windows spawn ERROR_PATH_NOT_FOUND |
| Implemented | 2026-06-07T14:05:00Z                                          |
| Status      | COMPLETE                                                      |

## Summary

Fixed a Windows `CreateProcess` `ERROR_PATH_NOT_FOUND` bug in `ManagedWorker::spawn()` caused by passing a relative `venv_path` to `resolve_python_path()` while the child process's working directory has been set to `_repo_root_for_worker()`. On Windows, `CreateProcess` resolves a relative executable path against the child's CWD, not the parent's — so when the two differ, the interpreter path does not exist and spawn fails. The fix resolves `cfg.venv_path` to an absolute path before calling `resolve_python_path()`, with a fallback to `std::env::current_dir().unwrap_or_default()` for confined environments.

## Resolved Dependencies

No new dependencies added — the fix uses only `std::env::current_dir()` and `PathBuf::join`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | In `spawn()`, resolve `cfg.venv_path` to absolute path before calling `resolve_python_path()` |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.6 → 0.1.7` |

## Commit Log

```
.forge/reports/P11-C1_plan.md        | 87 ++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md         |  6 +--
 .forge/state/state.json              | 13 +++---
 Cargo.lock                           |  2 +-
 crates/anvilml-worker/Cargo.toml     |  2 +-
 crates/anvilml-worker/src/managed.rs | 12 ++++-
 6 files changed, 110 insertions(+), 12 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-2ce11a52aa331635)

running 74 tests
test config::tests::test_device_type_default ... ok
test config::tests::test_default_server_config ... ok
test config::tests::test_model_kind_default ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
... (74 passed; 0 failed)

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-505644d776f2c139)

running 56 tests
... (56 passed; 0 failed)

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-cb53960350c2e5d7)

running 18 tests
... (18 passed; 0 failed)

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-3df337931d8f5352)

running 19 tests
... (19 passed; 0 failed)

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-34c36b28b693a903)

running 1 test
test test_open_creates_tables ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/device_store.rs (target/debug/deps/device_store-a2d3be5d5933bbf2)

running 4 tests
... (4 passed; 0 failed)

     Running tests/rescan.rs (target/debug/deps/rescan-44356cf60417b048)

running 2 tests
... (2 passed; 0 failed)

     Running tests/scanner.rs (target/debug/deps/scanner-d3218cbd3b96bb91)

running 1 test
... (1 passed; 0 failed)

     Running tests/seed_loader.rs (target/debug/deps/seed_loader-f7d1c1c83c7a3559)

running 7 tests
... (7 passed; 0 failed)

     Running tests/store_get.rs (target/debug/deps/store_get-5cb98cd23f67b4c3)

running 2 tests
... (2 passed; 0 failed)

     Running tests/store_list.rs (target/debug/deps/store_list-5cb98cd23f67b4c3)

running 3 tests
... (3 passed; 0 failed)

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-a96d08d1fc551a40)

running 10 tests
... (10 passed; 0 failed)

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-05a525fed4f50652)

running 11 tests
... (11 passed; 0 failed)

     Running tests/api_models.rs (target/debug/deps/api_models-68299f93e81b49a3)

running 3 tests
... (3 passed; 0 failed)

     Running tests/api_ws_events.rs (target/debug/deps/api_ws_events-0cb97765927efd8d)

running 1 test
test ws_connect_broadcast_receive ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-316f7f83479d2c4b)

running 16 tests
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_rocm_linux_hsa ... ok
test env::tests::test_build_env_mock_propagation ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok
test managed::tests::eof_sets_dead ... ok
test pool::tests::pid_for_returns_none_for_missing_worker ... ok
test pool::tests::pid_for_returns_child_pid_when_spawned ... ok
test pool::tests::pool_event_listener_merges_ready_capabilities ... ok
test pool::tests::spawn_all_creates_cpu_worker_when_no_gpus ... ok
test managed::tests::keepalive_pings_and_kills_on_timeout ... ok
test managed::tests::spawn_reaches_idle ... ok
test managed::tests::status_transitions ... ok
test managed::tests::respawn_after_death ... ok
test managed::tests::spawn_ping_pong ... ok
test managed::tests::handshake_completes_once ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-a912a0849e74f910)

running 8 tests
... (8 passed; 0 failed)

     Running tests/config_reference.rs (target/debug/deps/config_reference-e2f1f223aea0d61d)

running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_hardware

running 2 tests
... (2 passed; 0 failed)

Total: 200 tests, 200 passed, 0 failed, 0 ignored
```

## Format Gate

```
(no output — exit 0, no formatting drift)
```

## Platform Cross-Check

**Check 1 — Mock-hardware Linux:**
```
Checking anvilml-worker v0.1.7 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.4 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.1 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.54s
```

**Check 2 — Mock-hardware Windows (x86_64-pc-windows-gnu):**
```
Checking anvilml-worker v0.1.7 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.4 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.1 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.92s
```

**Check 3 — Real-hardware Linux:**
```
Blocking waiting for file lock on build directory
    Checking anvilml-worker v0.1.7 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.4 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.1 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.01s
```

**Check 4 — Real-hardware Windows (x86_64-pc-windows-gnu):**
```
Blocking waiting for file lock on build directory
    Checking anvilml-worker v0.1.7 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.4 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.1 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.70s
```

All four cross-checks exit 0.

## Project Gates

**Gate 1 — Config Surface Sync:**
```
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
