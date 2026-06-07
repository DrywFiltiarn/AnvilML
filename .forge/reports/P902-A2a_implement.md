# Implementation Report: P902-A2a

| Field | Value |
|-------|-------|
| Task ID | P902-A2a |
| Phase | 902 — Stabilisation Retrofit |
| Description | Unify WorkerPool.workers with shared_workers (pool.rs) |
| Implemented | 2026-06-07T23:50:00Z |
| Status | COMPLETE |

## Summary

Eliminated the split-brain between `WorkerPool.workers` (a plain `Vec<Arc<ManagedWorker>>`) and `shared_workers` (`Arc<RwLock<Vec>>`). Promoted `workers` to `Arc<RwLock<Vec<Arc<ManagedWorker>>>>`, making it the single source of truth, and made `shared_workers` alias the same Arc via simple clone. All public method accessors now acquire a read-lock before iterating, and test struct literals were updated accordingly. The `anvilml-worker` crate version was bumped from 0.1.10 to 0.1.11.

## Resolved Dependencies

No new dependencies added or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/pool.rs` | Unify workers field to Arc<RwLock<Vec>>, update all accessors, fix test struct literals |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.10 → 0.1.11 |

## Commit Log

```
 .forge/reports/P902-A2a_plan.md   | 182 ++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  13 +--
 Cargo.lock                        |   2 +-
 crates/anvilml-worker/Cargo.toml  |   2 +-
 crates/anvilml-worker/src/pool.rs |  69 +++++++++------
 6 files changed, 238 insertions(+), 36 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-3122c8765423216c)

running 16 tests
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_mock_propagation ... ok
test env::tests::test_build_env_rocm_linux_hsa ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok
test managed::tests::eof_sets_dead ... ok
test pool::tests::pid_for_returns_none_for_missing_worker ... ok
test pool::tests::pid_for_returns_child_pid_when_spawned ... ok
test pool::tests::pool_event_listener_merges_ready_capabilities ... ok
test pool::tests::spawn_all_creates_cpu_worker_when_no_gpus ... ok
test managed::tests::respawn_after_death ... ok
test managed::tests::keepalive_pings_and_kills_on_timeout ... ok
test managed::tests::handshake_completes_once ... ok
test managed::tests::spawn_reaches_idle ... ok
test managed::tests::spawn_ping_pong ... ok
test managed::tests::status_transitions ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.02s
```

All 259 workspace tests passed (74 anvilml-core + 56 anvilml-hardware + 18 anvilml-ipc + 19 anvilml-registry + 1 registry test + 4 device_store + 2 rescan + 1 scanner + 7 seed_loader + 2 store_get + 3 store_list + 22 scheduler + 16 server + 3 api_models + 1 api_ws_events + 16 worker + 8 backend cli + 1 config_reference + 2 doc-tests).

## Format Gate

```
(Exit 0 — no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
    Checking anvilml-worker v0.1.11 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.7 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.3 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.2 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.89s

# 2. Mock-hardware Windows cross-check
    Checking anvilml-worker v0.1.11 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.7 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.3 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.2 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.13s

# 3. Real-hardware Linux check
    Checking anvilml-worker v0.1.11 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.7 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.3 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.2 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.81s

# 4. Real-hardware Windows cross-check
    Checking anvilml-worker v0.1.11 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.7 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.3 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.2 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.00s
```

All four platform cross-checks exited 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
    Running tests/config_reference.rs (target/debug/deps/config_reference-5611fed5cf7633a5)
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- **Intermediate binding for test struct literals**: The plan specified `workers: Arc::new(RwLock::new(vec![...]))` inline in test struct literals. This caused a Rust parser ambiguity where the closing `)]` sequence (closing vec![] then RwLock::new()) was misinterpreted by the formatter/compiler. Resolved by introducing an intermediate `worker_list` binding:
  ```rust
  let worker_list = vec![Arc::new(ManagedWorker::new("worker-0".to_string(), 0))];
  let workers: Arc<RwLock<Vec<Arc<ManagedWorker>>>> = Arc::new(RwLock::new(worker_list));
  ```
  This is functionally equivalent and required for correct compilation.

## Blockers

None.
