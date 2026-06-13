# Plan Report: P907-A7

| Field       | Value                                                |
|-------------|------------------------------------------------------|
| Task ID     | P907-A7                                               |
| Phase       | 907 — ZeroMQ IPC Transport                           |
| Description | anvilml-worker: managed.rs update mock-hardware tests for ZeroMQ |
| Depends on  | P907-A6                                                |
| Project     | anvilml                                                |
| Planned at  | 2026-06-13T18:30:00Z                                   |
| Attempt     | 1                                                      |

## Objective

Update the mock-IPC test infrastructure in `managed.rs` to use zeromq PAIR sockets instead of DEALER sockets for in-process test communication. This eliminates DEALER/ROUTER identity frame complexity in tests while preserving the production DEALER socket path. The eight listed tests (`inject_handles_for_test`, `respawn_after_death`, `keepalive_pings_and_kills_on_timeout`, `eof_sets_dead`, `spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`) must all pass with `cargo test -p anvilml-worker --features mock-hardware` exiting 0.

## Scope

### In Scope
- Update `inject_handles_for_test()` to create a zeromq PAIR socket pair (supervisor binds, worker connects) and inject the worker-side PAIR socket via a new `TestIpcHandles` type
- Update `eof_sets_dead` test to create a PAIR socket pair directly (replacing the existing DEALER socket pair construction)
- Update `respawn_after_death` test to work with PAIR sockets via the updated `inject_handles_for_test`
- Add `TestIpcHandles` struct wrapping `zeromq::PairSocket`
- Add `run_loop_with_pair` function that runs the same combined reader/writer loop logic but accepts `TestIpcHandles` with `PairSocket`
- Add `reset_ipc_tx_for_test` test-only method that spawns a fresh `run_loop_with_pair` for respawn scenarios
- Keep production code (`spawn()`, `restart()`, `respawn()`, `IpcHandles`, `run_loop`) unchanged — they continue using `zeromq::DealerSocket`

### Out of Scope
- Changes to production `spawn()`, `restart()`, or `respawn()` methods
- Changes to `IpcHandles` struct (production DEALER path)
- Changes to `run_loop` function (production DEALER path)
- Changes to tests that spawn real Python workers (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`) — they use `spawn()` which handles the DEALER socket path internally
- Changes to `keepalive_pings_and_kills_on_timeout` — it does not create mock IPC sockets; it uses the broadcast channel directly with a dummy child process
- Changes to `pool.rs` or any other crate

## Approach

### Step 1: Add `TestIpcHandles` struct

Add a new struct in the `#[cfg(test)]` module that wraps a `zeromq::PairSocket`:

```rust
#[cfg(test)]
struct TestIpcHandles {
    socket: zeromq::PairSocket,
}
```

This mirrors `IpcHandles` but uses `PairSocket` instead of `DealerSocket`.

### Step 2: Add `run_loop_with_pair` function

Add a new private async function parallel to `run_loop`:

```rust
#[cfg(test)]
async fn run_loop_with_pair(
    worker_id: String,
    mut rx: mpsc::Receiver<WorkerMessage>,
    event_tx: broadcast::Sender<(String, WorkerEvent)>,
    status: Arc<RwLock<WorkerStatus>>,
    ipc_rx: oneshot::Receiver<TestIpcHandles>,
) {
    let TestIpcHandles { mut socket } = match ipc_rx.await {
        Ok(handles) => handles,
        Err(_) => {
            warn!(worker_id = %worker_id, "IPC channel closed before handles received");
            return;
        }
    };

    // Combined reader/writer loop for the zeromq PAIR socket.
    // Same logic as run_loop but uses PairSocket's send/recv API.
    loop {
        select! {
            msg = rx.recv() => {
                match msg {
                    Some(msg) => {
                        debug!(
                            worker_id = %worker_id,
                            message_type = ?msg_discriminant(&msg),
                            "sending message to worker"
                        );
                        let payload = match serialize_message(&msg) {
                            Ok(p) => p,
                            Err(e) => {
                                warn!(error = %e, worker_id = %worker_id, "failed to serialize message");
                                break;
                            }
                        };
                        if let Err(e) = socket.send(zeromq::ZmqMessage::from(payload)).await {
                            warn!(error = %e, worker_id = %worker_id, "failed to send IPC frame");
                            break;
                        }
                    }
                    None => {
                        debug!(worker_id = %worker_id, "mpsc channel closed");
                        break;
                    }
                }
            }
            result = socket.recv() => {
                let msg = match result {
                    Ok(msg) => msg,
                    Err(e) => {
                        warn!(error = %e, worker_id = %worker_id, "socket recv error");
                        break;
                    }
                };

                let bytes: Vec<u8> = match msg.try_into() {
                    Ok(b) => b,
                    Err(_) => {
                        warn!(worker_id = %worker_id, "failed to convert zmq message to bytes");
                        break;
                    }
                };

                let event = match rmp_serde::from_slice::<serde_json::Map<String, JsonValue>>(&bytes) {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(error = %e, worker_id = %worker_id, "failed to deserialize IPC frame");
                        break;
                    }
                };

                let event = match worker_event_from_map(&event) {
                    Ok(e) => e,
                    Err(e) => {
                        warn!(error = %e, worker_id = %worker_id, "failed to parse worker event");
                        break;
                    }
                };

                debug!(
                    worker_id = %worker_id,
                    event_type = ?event_discriminant(&event),
                    "received event from worker"
                );

                update_status_from_event(&status, &event).await;

                let _ = event_tx.send((worker_id.clone(), event));
            }
        }
    }

    warn!(worker_id = %worker_id, "run_loop_with_pair exiting — worker is Dead");

    let _ = event_tx.send((
        worker_id.clone(),
        WorkerEvent::WorkerStatusChanged {
            status: WorkerStatus::Dead,
        },
    ));
}
```

The logic is identical to `run_loop`; only the socket type changes from `DealerSocket` to `PairSocket`. Both socket types have the same `send(ZmqMessage)` and `recv()` API in zeromq 0.4.

### Step 3: Update `inject_handles_for_test`

Replace the DEALER socket pair construction with a PAIR socket pair:

```rust
#[cfg(test)]
pub async fn inject_handles_for_test(&self) -> zeromq::PairSocket {
    // Create a PAIR socket pair for test IPC.
    let mut supervisor_pair = zeromq::PairSocket::new();
    let endpoint = supervisor_pair
        .bind("tcp://127.0.0.1:0")
        .await
        .expect("bind supervisor pair");
    let port = match endpoint {
        zeromq::Endpoint::Tcp(_, port) => port,
        _ => panic!("expected TCP endpoint"),
    };

    let mut worker_pair = zeromq::PairSocket::new();
    worker_pair
        .connect(&format!("tcp://127.0.0.1:{port}"))
        .await
        .expect("connect worker pair");

    // Inject the worker-side socket into the run_loop via a oneshot channel.
    let (test_ipc_tx, test_ipc_rx) = oneshot::channel::<TestIpcHandles>();
    let worker_id = self.worker_id.clone();
    let status = self.status.clone();
    let event_tx = self.event_tx.clone();

    // Create a fresh (tx, rx) pair for the mpsc channel.
    let (tx, rx) = mpsc::channel(64);

    // Spawn a run_loop_with_pair for the test.
    let handle = spawn(run_loop_with_pair(
        worker_id, rx, event_tx, status, test_ipc_rx,
    ));

    // Replace the existing handle and channel.
    *tokio::task::block_in_place(|| self.tx.lock().unwrap()) = tx;
    *self.handle.lock().unwrap() = handle;

    // Inject the worker-side PAIR socket.
    test_ipc_tx
        .send(TestIpcHandles { socket: worker_pair })
        .unwrap();

    // Return the supervisor-side socket for the test to use.
    supervisor_pair
}
```

This replaces the existing `inject_handles_for_test` entirely. Instead of injecting into the existing `ipc_tx` channel (which uses `IpcHandles`/`DealerSocket`), it creates a completely fresh run loop with `run_loop_with_pair` and `TestIpcHandles`/`PairSocket`.

### Step 4: Add `reset_ipc_tx_for_test`

Add a test-only method for respawn scenarios:

```rust
#[cfg(test)]
async fn reset_ipc_tx_for_test(&self) {
    let (tx, rx) = mpsc::channel(64);
    let worker_id = self.worker_id.clone();
    let status = self.status.clone();
    let event_tx = self.event_tx.clone();
    let (test_ipc_tx, test_ipc_rx) = oneshot::channel::<TestIpcHandles>();

    let new_handle = spawn(run_loop_with_pair(
        worker_id, rx, event_tx, status, test_ipc_rx,
    ));

    *tokio::task::block_in_place(|| self.tx.lock().unwrap()) = tx;
    *self.handle.lock().unwrap() = new_handle;

    // Return the oneshot sender so the caller can inject TestIpcHandles.
    test_ipc_tx
}
```

### Step 5: Update `eof_sets_dead` test

Replace the DEALER socket pair construction with PAIR sockets:

- Change `zeromq::DealerSocket::new()` → `zeromq::PairSocket::new()` for both supervisor and worker sides
- Change `supervisor_dealer.bind(...)` → `supervisor_pair.bind(...)`
- Change `worker_dealer.connect(...)` → `worker_pair.connect(...)`
- Inject via a fresh `run_loop_with_pair` instead of injecting directly into the existing `run_loop`

The test flow:
1. Create PAIR socket pair (supervisor binds, worker connects)
2. Create `ManagedWorker`, subscribe to events
3. Inject worker-side PAIR socket via `run_loop_with_pair`
4. Send Ready frame through supervisor PAIR socket
5. Verify Ready event received
6. Send invalid bytes
7. Verify WorkerStatusChanged(Dead) broadcast

### Step 6: Update `respawn_after_death` test

Replace the `reset_ipc_tx()` + `inject_handles_for_test()` call chain with the new test-specific path:

- Replace `worker.reset_ipc_tx().await` with `let test_tx = worker.reset_ipc_tx_for_test().await`
- After setting status to Idle and starting keepalive, inject fresh handles using the returned `test_ipc_tx`:
  ```rust
  let (inject_tx, inject_rx) = oneshot::channel::<TestIpcHandles>();
  // ... spawn run_loop_with_pair ...
  inject_tx.send(TestIpcHandles { socket: worker_pair }).unwrap();
  ```

Actually, the cleaner approach is to have `reset_ipc_tx_for_test` return the `oneshot::Sender<TestIpcHandles>` so the test can inject after resetting. Then call `inject_handles_for_test` for the second injection (respawn).

Updated flow for `respawn_after_death`:
1. Call `inject_handles_for_test()` → get supervisor PAIR socket, drop it to trigger EOF
2. Wait for Dead status
3. Set status to Idle, spawn dummy child, start keepalive
4. Wait for pong timeout → Dead
5. Verify WorkerStatusChanged(Dead) broadcast
6. Set status to Respawning, broadcast event
7. Call `reset_ipc_tx_for_test()` → get fresh run loop with `TestIpcHandles` channel
8. Inject fresh worker PAIR socket via the returned sender
9. Set status to Idle
10. Verify Idle status

### Step 7: No changes needed for these tests

The following tests use `spawn()` which handles the production DEALER socket path internally. They do not construct mock sockets and require no changes:

- `spawn_ping_pong` — spawns real Python worker via `spawn()`
- `status_transitions` — spawns real Python worker via `spawn()`
- `handshake_completes_once` — spawns real Python worker via `spawn()`
- `spawn_reaches_idle` — spawns real Python worker via `spawn()`
- `keepalive_pings_and_kills_on_timeout` — uses broadcast channel directly, no mock sockets

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `TestIpcHandles` struct, `run_loop_with_pair` function, update `inject_handles_for_test`, add `reset_ipc_tx_for_test`, update `eof_sets_dead` and `respawn_after_death` tests |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `managed.rs` (tests module) | `inject_handles_for_test` | PAIR socket pair creation and injection into run_loop |
| `managed.rs` (tests module) | `eof_sets_dead` | PAIR socket EOF triggers Dead status and WorkerStatusChanged broadcast |
| `managed.rs` (tests module) | `respawn_after_death` | Reset IPC with PAIR sockets, respawn transitions back to Idle |
| `managed.rs` (tests module) | `keepalive_pings_and_kills_on_timeout` | Pong timeout kills worker (uses broadcast channel, no socket changes) |
| `managed.rs` (tests module) | `spawn_ping_pong` | Real Python worker Ping/Pong roundtrip over DEALER socket |
| `managed.rs` (tests module) | `status_transitions` | Initializing → Idle (Ready) → Dead over real DEALER socket |
| `managed.rs` (tests module) | `handshake_completes_once` | Exactly one Ready event on spawn |
| `managed.rs` (tests module) | `spawn_reaches_idle` | Spawn reaches Idle without timing workarounds |

## CI Impact

No CI workflow files are modified. The test command `cargo test --workspace --features mock-hardware` will exercise all updated tests. The Windows cross-check `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` must also pass (PAIR sockets are pure Rust, no platform-specific FFI).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `PairSocket` API differs from `DealerSocket` in zeromq 0.4 | Low | Tests fail to compile | Verify `PairSocket::bind()`, `PairSocket::connect()`, `send()`, `recv()` signatures match the existing code; use `ZmqMessage` for send as done with `DealerSocket` |
| PAIR socket semantics differ from DEALER (symmetric, one-to-one connection) | Medium | Tests behave differently than production | PAIR is intentionally used only for tests — it's simpler and more reliable for in-process communication. Production DEALER path is unchanged |
| `reset_ipc_tx_for_test` races with existing run_loop handle | Low | Test flakiness | `reset_ipc_tx_for_test` replaces `self.handle` via `block_in_place` mutex lock; the old handle is abandoned (will finish or be aborted by tokio) |
| `inject_handles_for_test` creates a completely new mpsc channel, losing pending messages | Low | Test state loss | Tests don't send messages before injection; they create fresh workers and inject immediately |
| Windows cross-check fails due to zeromq PAIR socket on cross-compiled target | Low | Build failure | zeromq 0.4 is pure Rust with no platform-specific code for basic socket operations; cross-compilation should succeed |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 with all 8 tests passing (0 failed, 0 errors)
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
- [ ] No changes to production code paths (`spawn()`, `restart()`, `respawn()`, `IpcHandles`, `run_loop`)
- [ ] No changes to tests that use `spawn()` (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`)
- [ ] `eof_sets_dead` uses PAIR sockets instead of DEALER sockets
- [ ] `inject_handles_for_test` returns `zeromq::PairSocket` and creates a PAIR socket pair
- [ ] `respawn_after_death` works with the updated PAIR socket infrastructure
- [ ] `keepalive_pings_and_kills_on_timeout` unchanged (no mock socket construction)
- [ ] No public API changes (no new `pub` items, no signature changes to existing `pub` items)
