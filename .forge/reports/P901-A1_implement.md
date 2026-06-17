# Implementation Report: P901-A1

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P901-A1                                                     |
| Phase       | 901 — ManagedWorker Run-Loop and RespawnPolicy Retrofit     |
| Description | anvilml-worker: managed.rs fix run() to loop continuously instead of returning after one event |
| Implemented | 2026-06-17T13:15:00Z                                        |
| Status      | COMPLETE                                                    |

## Summary

Wrapped the single `tokio::select!` block in `ManagedWorker::run()` with a `loop { ... }` so the method processes broadcast events for the worker's full lifetime instead of returning after the first event. The `ready_timeout` is re-evaluated each iteration and only armed when the worker is in `Initializing` state. A `break` on `RecvError::Closed` provides the only exit path for a closed channel. A mandatory DEBUG log at loop start was added. The worker's held sender is dropped after subscribing so the broadcast channel closes when all external senders are dropped (enabling tests to exit cleanly).

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | tokio   | (workspace)     | Cargo.lock     | sync, time, process    |

No new external crates are introduced. The `tokio::time::sleep()`, `tokio::select!`, and `tokio::sync::broadcast::Receiver::recv()` APIs are already present in the codebase.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Wrap `tokio::select!` in `loop { ... }`; scope `ready_timeout` to Initializing state; add `break` on `RecvError::Closed`; add DEBUG log at loop start; drop `self.event_tx` after subscribing |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.7` → `0.1.8` |

## Commit Log

```
 .forge/reports/P901-A1_plan.md       | 190 ++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md         |   6 +-
 .forge/state/state.json              |  13 +-
 Cargo.lock                           |   4 +-
 crates/anvilml-worker/Cargo.toml     |   2 +-
 crates/anvilml-worker/src/managed.rs | 301 ++++++++++++++++++++---------------
 6 files changed, 373 insertions(+), 143 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-47dd47a6de069005)

running 3 tests
test managed::tests::test_managed_worker_processes_ready_event ... ok
test managed::tests::test_managed_worker_processes_completed_event ... ok
test managed::tests::test_spawned_task_updates_status ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/bridge_tests.rs
running 3 tests
test test_handles_drop_cleanly ... ok
test test_writer_sends_message ... ok
test test_reader_broadcasts_event ... ok
test result: ok. 3 passed; 0 failed

     Running tests/env_tests.rs
running 10 tests — all ok

     Running tests/keepalive_tests.rs
running 3 tests — all ok

     Running tests/managed_tests.rs
running 6 tests
test test_shutdown_cleans_up_handles ... ok
test test_ready_timeout_dead ... ok
test test_dying_event_transitions_dead ... ok
test test_spawn_reaches_idle ... ok
test test_status_transitions_idle_to_busy_to_idle ... ok
test test_keepalive_timeout_sets_dead ... ok
test result: ok. 6 passed; 0 failed

     Running tests/pool_tests.rs
running 4 tests — all ok

     Running tests/respawn_tests.rs
running 4 tests — all ok

     Running tests/spawn_tests.rs
running 7 tests — all ok

Full workspace: 151 tests passed; 0 failed; 0 ignored
```

## Format Gate

```
cargo fmt --all -- --check
(Exit 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.13s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.22s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.00s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.20s
```

All four checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
test config_reference ... ok
test result: ok. 1 passed; 0 failed
```

### Gate 2 — OpenAPI Drift
```
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
(No diff — openapi.json is up to date)
```

### Gate 3 — Node Parity
Not triggered — no changes to `worker/nodes/` or `node_registry.rs`.

## Public API Delta

```
(no new pub items introduced)
```

No new `pub` items. The only method affected is the existing `pub async fn run(mut self)` in `ManagedWorker` — signature unchanged, behaviour corrected to loop continuously.

## Deviations from Plan

1. **`tokio::time::Sleep` pinning**: The plan's suggested `sleep.as_mut().await` does not compile because `tokio::time::Sleep` does not implement `AsMut`. The fix uses `ready_timeout.take()` to extract ownership, then `std::pin::pin!(sleep).as_mut().await` to properly pin the `!Unpin` sleep type.

2. **Dropping `self.event_tx` after subscribing**: The plan assumes `drop(event_tx)` in tests will close the broadcast channel. However, the worker holds its own clone of the sender, so the channel never closes. Added `drop(self.event_tx)` after `subscribe()` so the worker no longer holds a sender. In tests, the test's clone becomes the only sender; in production, the bridge reader's clone is the only sender. Both cases correctly close the channel when the external sender is dropped.

3. **Removed `mut` on `sleep` binding**: The `ready_timeout.take()` returns an owned `Sleep`, and `pin!` takes ownership — no `mut` needed on the `sleep` variable.

## Blockers

None.
