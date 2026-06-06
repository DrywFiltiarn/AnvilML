# Tasks: Phase 010 — Worker Crash Recovery

| Field | Value |
|-------|-------|
| Phase | 010 |
| Name | Worker Crash Recovery |
| Milestone group | Worker lifecycle |
| Depends on phases | 1-9 |
| Task file | `forge/tasks/tasks_phase010.json` |
| Tasks | 8 |

## Overview

Phase 10 implements the watchdog: 30s keepalive ping with force-kill on Pong timeout, automatic respawn (2s delay) after a worker dies, orphan cleanup (PR_SET_PDEATHSIG / Job Object), and bridging worker status changes onto the WebSocket. After this phase you can kill a worker process and watch the server detect it, mark it Dead, and respawn it to Idle without restarting the server.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P10-A1 | `crates/anvilml-worker/src/pool.rs` | anvilml-worker: keepalive Ping + Pong-timeout force-kill |
| P10-A2 | `crates/anvilml-worker/src/managed.rs` | anvilml-worker: respawn after death (2s delay) + WorkerStatusChanged events |
| P10-A3 | `backend/src/main.rs` | anvilml-server: broadcast worker status changes to WS |
| P10-A4 | `crates/anvilml-worker/src/pool.rs` | anvilml: test-only worker PID accessor for crash-recovery proof |
| P10-B1 | `crates/anvilml-worker/src/managed.rs` | anvilml-worker: fix double InitializeHardware write causing worker death at startup |
+| P10-B2 | `crates/anvilml-worker/src/managed.rs`, `worker/tests/test_worker_main.py` | anvilml-worker: end-to-end handshake regression test (spawn → Ready → Idle) |
+| P10-B3 | `crates/anvilml-worker/src/managed.rs` | anvilml-worker: fix epoll edge-trigger missed wakeup — deliver handles before writing InitializeHardware |
+| P10-B4 | `crates/anvilml-worker/src/managed.rs`, `worker/tests/test_worker_main.py` | anvilml-worker: end-to-end spawn→Ready→Idle regression test against epoll fix |

## Task details

#### P10-A1: anvilml-worker: keepalive Ping + Pong-timeout force-kill

- **Prereqs:** P9-A6
- **Tags:** reasoning

In pool.rs / managed.rs add a keepalive task per worker: send Ping{seq} every 30s, expect Pong{seq} within 10s; on timeout call Child::kill() (SIGKILL unix / TerminateProcess windows via tokio Child::kill). For testability allow the interval to be overridden via ANVILML_PING_INTERVAL_MS / ANVILML_PONG_TIMEOUT_MS env (used by tests). cargo test -p anvilml-worker --features mock-hardware -- keepalive exits 0: with short intervals, a mock worker that stops responding is killed. Also pass: cargo check --target x86_64-pc-windows-gnu --features mock-hardware.

#### P10-A2: anvilml-worker: respawn after death (2s delay) + WorkerStatusChanged events

- **Prereqs:** P10-A1
- **Tags:** reasoning

In managed.rs: on detecting Dead (EOF or ping timeout), broadcast WorkerStatusChanged(Dead), wait 2s (override via ANVILML_RESPAWN_DELAY_MS), transition Respawning (broadcast), re-spawn the child, re-send InitializeHardware, on Ready broadcast Idle. Orphan cleanup at spawn: Linux PR_SET_PDEATHSIG via pre_exec; Windows Job Object KILL_ON_JOB_CLOSE. cargo test -p anvilml-worker --features mock-hardware -- respawn exits 0: kill worker, observe Dead->Respawning->Idle within timeout. Also pass: cargo check --target x86_64-pc-windows-gnu --features mock-hardware.

#### P10-A3: anvilml-server: broadcast worker status changes to WS

- **Prereqs:** P10-A2
- **Tags:** —

In main.rs / server wiring: subscribe to WorkerPool.subscribe_events(); for each WorkerStatusChanged forward as WsEvent::WorkerStatusChanged via the EventBroadcaster so /v1/events clients see worker state transitions. Spawn this bridge task at startup. cargo test --workspace --features mock-hardware exits 0. Verified live in P10-A4.

#### P10-A4: anvilml: test-only worker PID accessor for crash-recovery proof

- **Prereqs:** P10-A3
- **Tags:** —

Add #[cfg(any(test,feature="test-helpers"))] fn pid_for(&self,worker_id:&str)->Option<u32> on WorkerPool returning the child PID. This enables the runnable crash-recovery proof and the integration test in later phases. cargo test --workspace --features mock-hardware exits 0. Runnable proof: ANVILML_WORKER_MOCK=1 cargo run --features mock-hardware; note worker PID from logs; kill <pid>; watch /v1/workers (or /v1/events) show Dead then Respawning then Idle within ~3s; server stays up. Also pass: cargo check --target x86_64-pc-windows-gnu --features mock-hardware.

#### P10-B1: anvilml-worker: fix double InitializeHardware write causing worker death at startup

- **Prereqs:** P10-A4
- **Tags:** reasoning

`ManagedWorker::spawn()` writes `InitializeHardware` directly to stdin (Unix: fd-dup + synchronous write; Windows: async write + flush), then unconditionally enqueues the same message into the mpsc channel via `self.tx.send(init_msg)`. When `writer_task` drains the channel it writes a second frame to the same pipe. The Python worker receives `InitializeHardware` twice, treats the second as unexpected, and exits — closing its stdout. The Rust reader task sees EOF, sets status to `Dead`, and `spawn()` times out with `"worker did not reach Ready state in time"`, causing a panic in `pool.rs`. This is consistent across Windows and Linux.

Remove the `self.tx.send(init_msg).await` call that follows the direct stdin write in `spawn()`. `InitializeHardware` must be delivered exactly once via the direct path only. All subsequent messages (Ping, Shutdown, Execute, etc.) continue to use the mpsc channel normally.

Also remove the redundant `status.write().await = Dead` statement after the `break` at the bottom of `reader_task`. The `WorkerStatusChanged(Dead)` broadcast emitted inside the loop already transitions the status; the post-loop write produces a duplicate broadcast and a redundant lock acquisition.

`cargo test -p anvilml-worker --features mock-hardware` exits 0. `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0.

#### P10-B2: anvilml-worker: end-to-end handshake regression test (spawn → Ready → Idle)

- **Prereqs:** P10-B1
- **Tags:** reasoning

Remove `#[ignore]` from `spawn_ping_pong` and `status_transitions` in `managed.rs`. Both tests must now pass unconditionally under `ANVILML_WORKER_MOCK=1` with `ANVILML_VENV_PATH` set to the CI venv (matching the P9-B1 environment setup).

Add a new Rust test `handshake_completes_once` in `managed.rs`: subscribe to the broadcast channel before calling `spawn()`, call `spawn()`, assert status equals `Idle`, then drain the broadcast channel for 500 ms and assert exactly one `Ready` event was received with no subsequent `Dying` or second `Ready` event. This directly guards against re-introduction of the double-write bug.

Add a new pytest test `test_double_init_exits` in `worker/tests/test_worker_main.py`: send `InitializeHardware` twice in sequence to a reshly spawned worker subprocess; assert the worker sends `Ready` in response to the first, then sends `Dying` (or exits non-zero) in esponse to the second. This ensures the Python side explicitly rejects duplicate initialisation rather than silently accepting it.

`cargo test -p anvilml-worker --features mock-hardware -- handshake` exits 0. `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v` exits 0. `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0.


#### P10-B3: anvilml-worker: fix epoll edge-trigger missed wakeup — deliver handles before writing InitializeHardware

- **Prereqs:** P10-B2
- **Tags:** reasoning

`ManagedWorker::spawn()` currently writes `InitializeHardware` directly to the pipe fd before delivering `IpcHandles` to `run_loop` via the oneshot channel. Because `reader_task` has not yet called `poll_read`, the fd is not yet registered with epoll. On Linux, `ChildStdout` uses edge-triggered epoll: a notification only fires on a state transition from empty to data-available. Data written before registration is never signalled, so `reader_task` blocks forever. This is why `os.read()` on the raw fd works (bypasses epoll) while tokio's async read does not.

Three changes to `spawn()` in `crates/anvilml-worker/src/managed.rs`:

1. **Remove the 500 ms sleep.** It was a workaround for the direct-write race and is no longer needed.
2. **Deliver IpcHandles via oneshot immediately after taking stdin/stdout** — before any write. Move the `ipc_tx.lock().unwrap().take().send(IpcHandles { stdin, stdout })` call to immediately follow the `child.stdout.take()` call, before any frame write. This lets `run_loop` start `reader_task` and register the fd with epoll before any data arrives.
3. **Remove the direct synchronous write entirely.** Delete the `#[cfg(unix)]` fd-dup block and the `#[cfg(windows)]` async-write block. Send `InitializeHardware` through the mpsc channel only: `self.tx.send(init_msg).await`. The `writer_task` will write it after `reader_task` is already polling. The mpsc send may not complete until `writer_task` is scheduled, which is correct — it must not write before the reader is ready.

Also remove the `#[cfg(unix)]` imports for `std::io::Write`, `std::os::fd::{AsRawFd, FromRawFd, IntoRawFd}`, and the `libc::dup` usage in `spawn()`. Remove the `rmp_serde` direct usage in `spawn()` if it was added solely for the direct-write path (the framing crate already handles serialization via `write_frame`). Retain the `libc` dependency for `PR_SET_PDEATHSIG` in `pre_exec`.

The `wait_for_ready` polling loop at the end of `spawn()` (which polls status until `Idle` with a timeout) must remain unchanged — it is still the correct mechanism to confirm the handshake completed.

`cargo run --features mock-hardware` with `ANVILML_WORKER_MOCK=1` and `ANVILML_VENV_PATH` set must reach a live Idle worker without panicking. `cargo test -p anvilml-worker --features mock-hardware` exits 0. `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0.

#### P10-B4: anvilml-worker: end-to-end spawn→Ready→Idle regression test against epoll fix

- **Prereqs:** P10-B3
- **Tags:** reasoning

With the epoll fix in place, the three previously-failing Rust integration tests (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`) must now pass against a real Python subprocess. Verify and un-skip any remaining `#[ignore]` attributes. Add one new Rust test `spawn_reaches_idle` in `managed.rs` as the canonical regression guard: spawn a `ManagedWorker` with `ANVILML_WORKER_MOCK=1` and `ANVILML_VENV_PATH`, call `spawn()`, assert the returned `Result` is `Ok`, assert `get_status().await == WorkerStatus::Idle`. This test must pass without any sleep or timing workaround — if it requires a sleep to be reliable, the epoll fix is incomplete.

Also update `test_double_init_exits` in `worker/tests/test_worker_main.py` if the current assertion (`Ready` then silent ignore then `Shutdown` → `Dying`) was written as a workaround for the old worker behaviour: confirm the assertion matches actual Python worker behaviour under the current `worker_main.py` message loop and tighten it if possible.

`cargo test -p anvilml-worker --features mock-hardware` exits 0 with 0 ignored tests. `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v` exits 0. `ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./venv cargo run --features mock-hardware` starts, logs `status=idle` for worker-0, and `/v1/workers` returns one worker with `"status":"idle"`. `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0.

## Runnable Proof

Start the server with a worker, kill the worker process, and watch it recover.

```bash
ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./venv \
  cargo run --features mock-hardware
# note the worker PID from the startup logs, then:
kill -9 <worker_pid>
# watch the worker recover:
watch -n0.5 'curl -s http://127.0.0.1:8488/v1/workers'
```

Expected: `/v1/workers` (or the `/v1/events` stream) shows the worker go `dead` -> `respawning` -> `idle` within ~3 seconds, and the server process itself stays up the whole time. Phase done when a killed worker is automatically respawned to Idle and the server never goes down.
