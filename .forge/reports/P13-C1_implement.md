# Implementation Report: P13-C1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P13-C1                             |
| Phase         | 13 — Job Queue                     |
| Description   | backend: wire reset_ghost_jobs() into server startup sequence |
| Implemented   | 2026-07-07T10:45:00Z               |
| Status        | COMPLETE                           |

## Summary

Wired `reset_ghost_jobs()` into the AnvilML server startup sequence in `backend/src/main.rs`. The task added a `JobStore` import, renamed the previously-unused `_pool` binding to `pool`, and inserted a `reset_ghost_jobs()` call between the seed loader and the server start-time capture. The method transitions any stale `Queued` or `Running` jobs to `Failed` with `error = "server_restart"`, making them visible to operators for retry or discard. No new files, no new public API items, and no new dependencies were introduced.

## Resolved Dependencies

None. This task only uses existing crates already declared in `backend/Cargo.toml`: `anvilml-registry` (path dependency) and its transitive dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/src/main.rs` | Add `JobStore` import, rename `_pool` to `pool`, add `reset_ghost_jobs()` call after seed loader |
| Modify | `backend/Cargo.toml` | Bump patch version 0.1.9 → 0.1.10 |

## Commit Log

```
 .forge/reports/P13-C1_plan.md | 113 ++++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md  |   6 +--
 .forge/state/state.json       |  13 ++---
 Cargo.lock                    |   2 +-
 backend/Cargo.toml            |   2 +-
 backend/src/main.rs           |  22 +++++++-
 6 files changed, 145 insertions(+), 13 deletions(-)
```

## Test Results

```
cargo test --workspace --features mock-hardware

     Running unittests src/lib.rs (target/debug/deps/anvilml-f1927eaf820d1b61)
     running 0 tests
     test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-9dbbe54cfe5f0456)
     running 0 tests
     test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/cli_help_test.rs
     running 1 test
     test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs
     running 1 test
     test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/db_startup_tests.rs
     running 5 tests
     test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/hw_probe_help_test.rs
     running 1 test
     test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/logging_tests.rs
     running 6 tests
     test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/shutdown_tests.rs
     running 2 tests
     test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_tests.rs (anvilml_artifacts)
     running 9 tests
     test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/artifact_tests.rs (anvilml_core)
     running 3 tests
     test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_load_tests.rs (anvilml_core)
     running 13 tests
     test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_tests.rs (anvilml_core)
     running 13 tests
     test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/error_tests.rs (anvilml_core)
     running 16 tests
     test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/events_tests.rs (anvilml_core)
     running 10 tests
     test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/hardware_tests.rs (anvilml_core)
     running 9 tests
     test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/job_tests.rs (anvilml_core)
     running 4 tests
     test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/model_tests.rs (anvilml_core)
     running 4 tests
     test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/node_registry_tests.rs (anvilml_core)
     running 5 tests
     test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/node_tests.rs (anvilml_core)
     running 4 tests
     test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/worker_tests.rs (anvilml_core)
     running 4 tests
     test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/cpu_tests.rs (anvilml_hardware)
     running 6 tests
     test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/detect_tests.rs (anvilml_hardware)
     running 14 tests
     test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/mock_tests.rs (anvilml_hardware)
     running 6 tests
     test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/sysfs_tests.rs (anvilml_hardware)
     running 7 tests
     test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/vulkan_tests.rs (anvilml_hardware)
     running 8 tests
     test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/error_tests.rs (anvilml_ipc)
     running 7 tests
     test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/roundtrip_tests.rs (anvilml_ipc)
     running 26 tests
     test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/stress_test.rs (anvilml_ipc)
     running 1 test
     test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/db_tests.rs (anvilml_registry)
     running 4 tests
     test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/device_store_tests.rs (anvilml_registry)
     running 5 tests
     test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/job_store_tests.rs (anvilml_registry)
     running 9 tests
     test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner_tests.rs (anvilml_registry)
     running 20 tests
     test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/seed_loader_tests.rs (anvilml_registry)
     running 8 tests
     test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_tests.rs (anvilml_registry)
     running 5 tests
     test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/dag_tests.rs (anvilml_scheduler)
     running 35 tests
     test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/ledger_tests.rs (anvilml_scheduler)
     running 6 tests
     test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/queue_tests.rs (anvilml_scheduler)
     running 9 tests
     test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/health_tests.rs (anvilml_server)
     running 1 test
     test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/nodes_tests.rs (anvilml_server)
     running 5 tests
     test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/state_tests.rs (anvilml_server)
     running 2 tests
     test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/bridge_tests.rs (anvilml_worker)
     running 5 tests
     test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/demux_tests.rs (anvilml_worker)
     running 5 tests
     test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/env_tests.rs (anvilml_worker)
     running 7 tests
     test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/keepalive_tests.rs (anvilml_worker)
     running 5 tests
     test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/managed_tests.rs (anvilml_worker)
     running 43 tests
     test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/pool_tests.rs (anvilml_worker)
     running 5 tests
     test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/real_startup_tests.rs (anvilml_worker)
     running 1 test
     test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/respawn_tests.rs (anvilml_worker)
     running 6 tests
     test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/spawn_tests.rs (anvilml_worker)
     running 6 tests
     test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests (all crates): 0 failed

all doctests ran in 0.68s; merged doctests compilation took 0.66s
all doctests ran in 1.15s; merged doctests compilation took 1.10s

Total: 0 failures across all crates.
```

## Format Gate

```
cargo fmt --all -- --check
(no output — all files formatted)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.57s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.19s

# 3. Real-hardware Linux
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 26.07s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.33s

All four checks exited 0.
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

# Clippy (real-hardware)
cargo clippy --bin anvilml -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.13s

All gates passed.
```

## Public API Delta

```
git diff HEAD -- backend/src/main.rs | grep '^+.*pub ' | head -40
(no output)
```

No new `pub` items introduced. This task only calls existing public APIs (`JobStore::new`, `JobStore::reset_ghost_jobs`) from the binary's `main()` function.

## Deviations from Plan

- The plan's approach step 3 binds `let ghost_count = ...` without using the variable. The compiler produces an `unused_variable` warning. I prefixed the variable with an underscore (`_ghost_count`) to suppress the warning, since the `reset_ghost_jobs()` method already logs at INFO level when count > 0 (making the count value at the call site unnecessary). This is a minimal change to satisfy clippy's `-D warnings` requirement without altering behavior.

## Blockers

None.
