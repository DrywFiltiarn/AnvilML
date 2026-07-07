# Implementation Report: P14-C2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P14-C2                          |
| Phase         | 14 — Dispatch & Execute         |
| Description   | backend: main.rs spawns real WorkerPool + JobScheduler at startup |
| Implemented   | 2026-07-07T23:30:00Z           |
| Status        | COMPLETE                        |

## Summary

Wired `backend/src/main.rs`'s normal (non-hw-probe) server-start path to detect hardware devices, spawn Python worker subprocesses via `WorkerPool::spawn_all()`, and start the scheduler's dispatch loop via `JobScheduler::start_dispatch_loop()`. The binary now calls `detect_all_devices()` after the ghost-job reset, constructs a `WorkerPool` (not yet Arc-wrapped), calls `spawn_all()` on it, then wraps the pool in `Arc` for sharing with `AppState` and the dispatch loop. The dispatch loop is started with a cloned scheduler Arc before constructing `AppState`. Three new `tracing::info!` log calls were added: "hardware devices detected", "workers spawned", and "dispatch loop started". The `backend/Cargo.toml` patch version was bumped from `0.1.10` to `0.1.11`.

## Resolved Dependencies

No new external crates or packages introduced. All types used (`WorkerPool`, `JobScheduler`, `detect_all_devices`, `HardwareInfo`, `JobStore`, `Arc`) are existing workspace-local dependencies already declared in `backend/Cargo.toml`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/src/main.rs` | Insert device detection, worker spawning, and dispatch loop startup into the normal server-start path; removed duplicate scheduler/worker construction |
| Modify | `backend/Cargo.toml` | Bump patch version 0.1.10 → 0.1.11 |

## Commit Log

```
 .forge/state/CURRENT_TASK.md |  6 ++--
 .forge/state/state.json      | 13 ++++----
 Cargo.lock                   |  2 +-
 backend/Cargo.toml           |  2 +-
 backend/src/main.rs          | 78 ++++++++++++++++++++++++++++++++++++--------
 5 files changed, 76 insertions(+), 25 deletions(-)
```

## Test Results

```
     Running tests/db_startup_tests.rs (target/debug/deps/db_startup_tests-f5edd34ad7caeeec)

running 5 tests
test tests::test_missing_seed_file_causes_startup_failure ... ok
test tests::test_db_file_created_on_startup ... ok
test tests::test_migrations_create_required_tables ... ok
test tests::test_seed_populates_device_capabilities ... ok
test tests::test_seed_idempotent_second_run ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/shutdown_tests.rs (target/debug/deps/shutdown_tests-088c37d7bd18abe8)

running 2 tests
test tests::test_shutdown_signal_returns_on_ctrl_c ... ok
test tests::test_shutdown_signal_timeout_cancels ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-06c2637630208ae7)

running 1 test
test tests::config_reference_matches_defaults ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

All workspace tests: 258 passed; 0 failed
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.77s
  --- CHECK 1 PASSED ---

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.93s
  --- CHECK 2 PASSED ---

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.78s
  --- CHECK 3 PASSED ---

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 26.95s
  --- CHECK 4 PASSED ---
```

## Project Gates

Gate 1 (Config Surface Sync): `cargo test -p anvilml --features mock-hardware -- config_reference` — 1 passed, 0 failed. No config fields were modified by this task.

Gate 2 (OpenAPI Drift): Not triggered — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields used in response types.

Gate 3 (Node Parity): Not triggered — this task does not add, remove, or rename any node type.

Gate 4 (Mock/Real Parity Markers): Not triggered — this task does not modify any node's `execute()` or arch module methods.

## Public API Delta

No new `pub` items introduced. This task only modifies `backend/src/main.rs`, which is a binary entry point (not a library). All types used are existing public APIs from workspace-local crates.

## Deviations from Plan

- **Field name correction:** The approved plan references `hw_info.devices` but the actual `HardwareInfo` struct has `pub gpus: Vec<GpuDevice>`. All references use `hw_info.gpus` to match the actual struct definition.
- **Variable naming:** The plan uses `pool` for both the `SqlitePool` (from `create_pool()`) and the `WorkerPool` (from `WorkerPool::new()`). To avoid shadowing, the worker pool variable is named `worker_pool` instead.
- **Scheduler Arc cloning:** `start_dispatch_loop` consumes `Arc<Self>`, so the plan's approach of calling it directly on `scheduler` would not compile. The fix is to use `Arc::clone(&scheduler).start_dispatch_loop(...)`, which was documented in the plan's own Step 3 rationale.
- **Node registry creation:** The original code had `node_registry` created on a separate line before the scheduler. Since the plan's revised approach placed the scheduler construction earlier, the `node_registry` creation was moved to right before the scheduler construction (after `workers` creation) to maintain correct ordering.

## Blockers

None.
