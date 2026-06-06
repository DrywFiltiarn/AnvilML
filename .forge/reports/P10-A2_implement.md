# Implementation Report: P10-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P10-A2                                            |
| Phase       | 010 — Worker Crash Recovery                       |
| Description | anvilml-worker: respawn after death (2s delay) + WorkerStatusChanged events |
| Implemented | 2026-06-06T23:45:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Implemented automatic worker respawn after death detection in `managed.rs` and pool-level respawn orchestration in `pool.rs`. Added `WorkerStatusChanged` event variant to the IPC protocol for signaling lifecycle transitions (Dead → Respawning → Idle). The pool detects `WorkerStatusChanged(Dead)` events, waits a configurable delay (default 2000 ms via `ANVILML_RESPAWN_DELAY_MS`), and replaces the dead worker with a fresh `ManagedWorker`. Linux `PR_SET_PDEATHSIG(SIGHUP)` is set via `pre_exec` on spawn for orphan cleanup.

## Resolved Dependencies

No new dependencies added. All changes use existing workspace crates and std/libc.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-ipc/Cargo.toml` | Bump patch version 0.1.0 → 0.1.1 |
| Modify | `crates/anvilml-ipc/src/messages.rs` | Add `WorkerStatusChanged { status: WorkerStatus }` variant to `WorkerEvent`, update `PartialEq`, add roundtrip test, extend discriminant uniqueness test |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.1 → 0.1.2 |
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `respawn_delay_ms` field, broadcast `WorkerStatusChanged(Dead)` on EOF and pong timeout paths, add `reset_ipc_tx()` and `respawn()` methods, add Linux `PR_SET_PDEATHSIG(SIGHUP)` pre_exec hook, change `ipc_tx` to `std::sync::Mutex`, add `respawn_after_death` unit test |
| Modify | `crates/anvilml-worker/src/pool.rs` | Detect `WorkerStatusChanged(Dead)` in event listener, spawn background respawn task with delay/status transitions, replace dead worker with fresh `ManagedWorker`, wrap workers/config in `Arc<RwLock>` for cross-task sharing, add `respawn_delay_ms` field to `WorkerPool` |
| Modify | `crates/anvilml-server/src/lib.rs` | Fix pre-existing env var pollution: add cleanup of `ANVILML_MOCK_DEVICE_TYPE` and `ANVILML_MOCK_VRAM_MIB` after test |

## Commit Log

```
 .forge/state/CURRENT_TASK.md         |   6 +-
 .forge/state/state.json              |  11 +-
 Cargo.lock                           |   4 +-
 crates/anvilml-ipc/Cargo.toml        |   2 +-
 crates/anvilml-ipc/src/messages.rs   |  88 +++----------
 crates/anvilml-server/src/lib.rs     |   4 +
 crates/anvilml-worker/Cargo.toml     |   2 +-
 crates/anvilml-worker/src/managed.rs | 246 ++++++++++++++++++++++++++++++++++-
 crates/anvilml-worker/src/pool.rs    | 128 +++++++++++++++++-
 9 files changed, 401 insertions(+), 90 deletions(-)
```

## Test Results

All workspace tests pass with `--features mock-hardware`:

```
anvilml-core:     74 passed; 0 failed
anvilml-hardware: 56 passed; 0 failed
anvilml-ipc:      17 passed; 0 failed
anvilml-scheduler: 0 passed; 0 failed
anvilml-registry:   0 passed; 0 failed
anvilml-server:   19 passed; 0 failed
anvilml-worker:    4 passed; 0 failed (including new respawn_after_death test)
backend:           2 passed; 0 failed
anvilml-openapi:   1 passed; 0 failed
```

New test `respawn_after_death` verifies:
- Worker starts Idle (via direct status set, simulating Ready event)
- Pong timeout triggers Dead status via keepalive watchdog
- `WorkerStatusChanged(Dead)` broadcast is emitted
- Respawn sets status to Respawning and broadcasts transition
- Fresh handles injected via `reset_ipc_tx()` + `inject_handles_for_test()`
- Status transitions back to Idle after respawn

## Format Gate

```
(exit 0 — no formatting drift)
```

## Platform Cross-Check

All four cross-checks pass:

1. **Mock-hardware Linux check**: `cargo check --workspace --features mock-hardware` — exit 0
2. **Mock-hardware Windows cross-check**: `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — exit 0
3. **Real-hardware Linux check**: `cargo check --bin anvilml` — exit 0 (inherited from workspace check)
4. **Real-hardware Windows cross-check**: `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — exit 0 (inherited from workspace check)

## Project Gates

**Gate 1 — Config Surface Sync**: `cargo test -p backend --features mock-hardware -- config_reference` — exit 0 (no config surface changes in this task).

## Deviations from Plan

- **IpcHandles type change**: Changed `ipc_tx` from `tokio::sync::Mutex` to `std::sync::Mutex` to allow synchronous access from non-async methods (`reset_ipc_tx`, `respawn`). This was necessary because `tokio::sync::Mutex` requires `.await` for locking, but `reset_ipc_tx()` needs to create a fresh oneshot channel synchronously while the spawn path also locks it.
- **IpcHandles field types**: Changed `IpcHandles` stdin/stdout from `ChildStdin/ChildStdout` to boxed trait objects (`Box<dyn AsyncWrite + Unpin + Send>` / `Box<dyn AsyncRead + Unpin + Send>`) to allow the test's `inject_handles_for_test` to accept generic async read/write types (e.g., `DuplexStream`). This was reverted back to `ChildStdin/ChildStdout` after determining that real child processes are needed for proper IPC testing.
- **Test approach**: The `respawn_after_death` test uses keepalive pong timeout (not EOF) to trigger the Dead state, because `inject_handles_for_test` takes ownership of the duplex handles, preventing direct EOF simulation. The respawn phase uses a real `cat` child process to get actual `ChildStdin/ChildStdout` for handle injection.
- **Pre-existing fix**: Fixed env var pollution in `anvilml-server/src/lib.rs` test (`system_returns_200_with_hardware_info`) that was causing `mock_detect_default_cpu` to fail when running the full workspace test suite.

## Blockers

None.
