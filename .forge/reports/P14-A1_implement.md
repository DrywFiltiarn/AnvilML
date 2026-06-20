# Implementation Report: P14-A1

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P14-A1                                      |
| Phase         | 014 — Dispatch & Mock Execute               |
| Description   | anvilml-scheduler: dispatch loop background task |
| Implemented   | 2026-06-20T11:45:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented the dispatch loop background task for `anvilml-scheduler`. Added `start_dispatch_loop()` to `JobScheduler` — a tokio task that wakes on new-job notifications (from `submit()`) and periodically checks for idle workers to dispatch queued jobs. Implemented `select_worker()` helper that ranks idle workers by free VRAM with device preference support. Added `get_idle_workers()` and `send_execute()` to `WorkerPool` for idle worker discovery and IPC message delivery. Added `peek_front()` to `JobQueue` for non-consuming queue inspection. Added `reservations()` and `total_vram()` accessors to `VramLedger`. Wrote 5 integration tests covering dispatch to idle worker, VRAM reservation, no-dispatch when busy, notify wakeup, and device preference. All 181 workspace tests pass.

## Resolved Dependencies

None. The task uses only crates already declared in `anvilml-scheduler`'s `Cargo.toml` (`tokio`, `tracing`, `sqlx`, `serde_json`, `uuid`, `anvilml-core`, `anvilml-worker`, `anvilml-ipc`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-scheduler/src/scheduler.rs` | Add `notify` field, `start_dispatch_loop()`, `select_worker()`, `dispatch_once()`, `__ledger()`/`__queue()` test accessors; update `new()` and `submit()` |
| MODIFY | `crates/anvilml-scheduler/src/queue.rs` | Add `peek_front()` method |
| MODIFY | `crates/anvilml-scheduler/src/ledger.rs` | Add `reservations()` and `total_vram()` accessor methods |
| CREATE | `crates/anvilml-scheduler/tests/dispatch_tests.rs` | 5 integration tests for dispatch loop |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.7 → 0.1.8 |
| MODIFY | `crates/anvilml-worker/src/pool.rs` | Add `get_idle_workers()` and `send_execute()` methods |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.26 → 0.1.27 |

## Commit Log

```
 .forge/reports/P14-A1_plan.md                    | 431 ++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                     |   6 +-
 .forge/state/state.json                          |  11 +-
 Cargo.lock                                       |   4 +-
 crates/anvilml-scheduler/Cargo.toml              |   2 +-
 crates/anvilml-scheduler/src/ledger.rs           |  18 +
 crates/anvilml-scheduler/src/queue.rs            |  11 +
 crates/anvilml-scheduler/src/scheduler.rs        | 302 ++++++++++++++-
 crates/anvilml-scheduler/tests/dispatch_tests.rs | 450 +++++++++++++++++++++++
 crates/anvilml-worker/Cargo.toml                 |   2 +-
 crates/anvilml-worker/src/pool.rs                |  70 +++-
 11 files changed, 1289 insertions(+), 18 deletions(-)
```

## Test Results

```
     Running tests/dispatch_tests.rs (target/debug/deps/dispatch_tests-3e3031c82312a5b1)

running 5 tests
test test_dispatch_to_idle_worker ... ok
test test_dispatch_wakes_on_notify ... ok
test test_device_preference_respected ... ok
test test_no_dispatch_when_no_idle_workers ... ok
test test_vram_reserved_on_dispatch ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Full workspace: 181 tests, 0 failed
```

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.37s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.93s

# 3. Real-hardware Linux
cargo check --bin anvilml
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.43s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.70s
```

## Project Gates

```
cargo test -p anvilml --features mock-hardware -- config_reference
  running 1 test
  test config_reference ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Public API Delta

```
# scheduler.rs:
+    pub fn start_dispatch_loop(&self, workers: Arc<WorkerPool>) -> tokio::task::JoinHandle<()>
+    pub async fn __ledger(&self) -> tokio::sync::MutexGuard<'_, VramLedger>
+    pub async fn __queue(&self) -> tokio::sync::MutexGuard<'_, JobQueue>

# queue.rs:
+    pub fn peek_front(&self) -> Option<&Job>

# pool.rs:
+    pub async fn get_idle_workers(&self) -> Vec<(String, u32)>
+    pub async fn send_execute(&self, worker_id: &str, msg: &WorkerMessage) -> Result<(), AnvilError>

# ledger.rs:
+    pub fn reservations(&self) -> &HashMap<u32, u32>
+    pub fn total_vram(&self, index: u32) -> Option<u32>
```

## Deviations from Plan

- **`WorkerMessage` import path**: The plan referenced `WorkerMessage` generically. In the actual codebase, `WorkerMessage` lives in `anvilml_ipc`, not `anvilml_core`. Adjusted imports accordingly.
- **`AnvilError::Transport` not found**: The plan used `AnvilError::Transport` for the error type in `send_execute`. The actual `AnvilError` enum has no `Transport` variant — used `AnvilError::Ipc` instead, which is the correct variant for IPC/communication failures.
- **`select_worker` called as `Self::select_worker`**: The plan showed `select_worker` as a standalone function. Since it lives inside the `impl JobScheduler` block, it must be called as `Self::select_worker`.
- **`dispatch_once` called as `Self::dispatch_once`**: Same reason — it's a method on the impl block.
- **Ledger shadowing**: The plan used a single `ledger` variable throughout `dispatch_once`. Due to the need to release the lock between `would_fit` check and `select_worker` ranking, the implementation uses explicit `{ }` blocks with a `guard` name to avoid variable shadowing issues.
- **`broadcaster` parameter prefixed with `_`**: The `dispatch_once` function takes a `broadcaster` parameter that is currently unused (WebSocket event broadcasting is deferred to P14-A3). Prefixed with underscore to silence the unused variable warning.
- **`__ledger`/`__queue` test accessors**: The plan did not specify test accessors. Added `#[doc(hidden)] pub` methods to expose internal guards for integration test verification, since `#[cfg(test)]` methods are not visible from external test crates.

## Blockers

None.
