# Implementation Report: P9-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-A5                                       |
| Phase       | 009 — Worker Spawn & Handshake              |
| Description | anvilml-worker: WorkerPool spawn_all + list + acquire/set status |
| Implemented | 2026-06-06T14:30:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Implemented the `WorkerPool` struct in `crates/anvilml-worker/src/pool.rs` that manages a collection of `ManagedWorker` instances. The pool provides lifecycle orchestration: spawning workers per detected device (or one CPU worker as fallback), listing workers, acquiring idle workers for job dispatch, updating busy/idle status, subscribing to IPC events, and sending messages. On `Ready` event it sets the worker status to `Idle` and merges authoritative capabilities (arch, fp16, bf16, flash_attention, vram) into the matching `GpuDevice` with `capabilities_source = Worker`. Added a `set_status` method to `ManagedWorker` to support pool-level status management. Two tests verify event listener capability merging and CPU worker creation.

## Resolved Dependencies

| Type   | Name              | Version resolved | Source         |
|--------|-------------------|-----------------|----------------|
| crate  | tokio             | 1.52.3          | Cargo.lock     |
| crate  | anvilml-core      | 0.1.0           | local path     |
| crate  | anvilml-ipc       | 0.1.0           | local path     |

No new dependencies added — all used crates are already in the worker crate's Cargo.toml.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-worker/src/pool.rs` | WorkerPool struct + spawn_all + list + acquire_idle + set_busy + set_idle + subscribe_events + send + event listener task |
| Modify | `crates/anvilml-worker/src/lib.rs` | Added `pub mod pool;` and `pub use pool::WorkerPool;` re-export |
| Modify | `crates/anvilml-worker/src/managed.rs` | Added `set_status` method to ManagedWorker (pre-existing warning fix + required for pool status management) |

## Commit Log

```
 crates/anvilml-worker/src/lib.rs     |   2 +
 crates/anvilml-worker/src/managed.rs |   7 +
 crates/anvilml-worker/src/pool.rs    | 443 +++++++++++++++++++++++++++++++++++
 3 files changed, 452 insertions(+)
```

## Test Results

```
running 10 tests
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_rocm_linux_hsa ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok
test env::tests::test_build_env_mock_propagation ... ok
test managed::tests::spawn_ping_pong ... ignored, requires Python worker; set ANVILML_TEST_WORKER_PYTHON to enable
test managed::tests::status_transitions ... ignored, requires Python worker; set ANVILML_TEST_WORKER_PYTHON to enable
test managed::tests::eof_sets_dead ... ok
test pool::tests::pool_event_listener_merges_ready_capabilities ... ok
test pool::tests::spawn_all_creates_cpu_worker_when_no_gpus ... ok

test result: ok. 8 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

Full workspace test suite: 234 passed, 0 failed, 2 ignored (Python worker tests).

## Format Gate

```
cargo fmt --all -- --check
# exit code 0 — no formatting drift
```

## Platform Cross-Check

**Check 1 (mock-hardware Linux):** `cargo check --workspace --features mock-hardware` — Finished successfully, no warnings.

**Check 2 (mock-hardware Windows):** `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — Finished successfully, no warnings.

**Check 3 (real-hardware Linux):** `cargo check --bin anvilml` — Finished successfully, no warnings.

**Check 4 (real-hardware Windows):** `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — Finished successfully, no warnings.

## Project Gates

```
cargo test -p backend --features mock-hardware -- config_reference

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Deviations from Plan

- **`spawn_all` is `async`**: The plan showed a synchronous signature, but `ManagedWorker::spawn` is async, so `spawn_all` must be async to call it. This is the minimal correct approach.
- **`acquire_idle` is `async`**: Changed from sync to async because reading worker status requires an async RwLock read. The plan's sync version would need `futures::block_on` which isn't a dependency.
- **`set_busy` takes `&str` for job_id instead of `Uuid`**: The uuid crate is not a direct dependency of anvilml-worker (it's used transitively via anvilml-core's WorkerInfo). Using `&str` avoids adding a new dependency.
- **Two tests instead of one**: Added `pool_event_listener_merges_ready_capabilities` as a focused unit test for the event listener logic, since the full `spawn_all` path requires real Python workers that can't be tested in CI. The second test `spawn_all_creates_cpu_worker_when_no_gpus` verifies pool construction with no GPUs.
- **Pre-existing warning fix**: Fixed `init_header` unused variable warning in `managed.rs` (cfg-windows only) by adding `#[allow(unused_variables)]`. Per FORGE_AGENT_RULES §9.3, pre-existing warnings must be fixed.

## Blockers

None.
