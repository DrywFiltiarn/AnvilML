# Plan Report: P907-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P907-A4                                           |
| Phase       | 907 — ZeroMQ IPC Transport                        |
| Description | anvilml-worker: managed.rs replace interprocess with ZeroMQ DEALER socket |
| Depends on  | none                                              |
| Project     | anvilml                                           |
| Planned at  | 2026-06-13T12:50:00Z                              |
| Attempt     | 1                                                 |

## Objective

Replace the `tokio::net::TcpListener` / `tokio::net::TcpStream` IPC transport in `managed.rs` with the `zeromq` crate's DEALER socket over `tcp://127.0.0.1:{port}`. The supervisor binds a DEALER socket on an OS-assigned port, passes the port via `ANVILML_IPC_PORT`, and the `IpcHandles` struct holds a `zeromq::Socket` instead of boxed `AsyncRead`/`AsyncWrite` trait objects. The `writer_task` and `reader_task` use zeromq `send`/`recv` with msgpack-serialised bytes, and the custom 4-byte length-prefix framing (`anvilml_ipc::framing`) is removed from the data path.

## Scope

### In Scope
- Replace `IpcHandles` struct: `Box<dyn AsyncRead>` / `Box<dyn AsyncWrite>` → `zeromq::Socket`
- Replace `spawn()` transport: `tokio::net::TcpListener`/`TcpStream` accept → `zeromq::DealerSocket::bind("tcp://127.0.0.1:0")`, extract port from returned `Endpoint::Tcp(_, port)`, pass via `ANVILML_IPC_PORT` env var (already set by `build_worker_env`)
- Replace `writer_task`: `framing::write_frame()` → `socket.send(ZmqMessage::from(msgpack_bytes)).await`
- Replace `reader_task`: `framing::read_frame()` → `socket.recv().await` + `Vec<u8>::try_from(msg)` + `rmp_serde::from_slice`
- Remove `ipc_socket_path` field from `ManagedWorker` (no longer needed — address logged at bind time)
- Remove `use anvilml_ipc::framing` import from managed.rs (keep `WorkerEvent` and `WorkerMessage`)
- Remove `ipc_addr` variable construction from `spawn()` — derive from zeromq bind endpoint instead
- Update `inject_handles_for_test` test helper to work with zeromq PAIR sockets (in-process bidirectional without DEALER identity frame complexity)
- Update existing tests: `eof_sets_dead`, `respawn_after_death`, `keepalive_pings_and_kills_on_timeout`
- Bump `anvilml-worker` crate version: `0.1.24` → `0.1.25`

### Out of Scope
- Python worker `ipc.py` changes (pyzmq integration, `ANVILML_IPC_PORT` env var reading) — handled by a separate task in Phase 907
- Removal of `framing.rs` from `anvilml-ipc` crate (retained for message type definitions)
- Changes to `pool.rs` (uses `anvilml_ipc::WorkerEvent`/`WorkerMessage` types only, no transport changes)
- Changes to `env.rs` (`ANVILML_IPC_PORT` already set by `build_worker_env`)
- New integration test file creation (covered by P907-B1)

## Approach

### Step 1 — Update `IpcHandles` struct
Replace the current `IpcHandles` struct:
```rust
struct IpcHandles {
    reader: Box<dyn tokio::io::AsyncRead + Send + Unpin + 'static>,
    writer: Box<dyn tokio::io::AsyncWrite + Send + Unpin + 'static>,
}
```
With:
```rust
struct IpcHandles {
    socket: zeromq::Socket,
}
```

### Step 2 — Rewrite `spawn()` transport logic
Replace the `tokio::net::TcpListener` bind/accept block with zeromq DEALER socket binding:

1. Create a new `zeromq::DealerSocket`: `let mut socket = zeromq::DealerSocket::new();`
2. Bind to `tcp://127.0.0.1:0`: `let endpoint = socket.bind("tcp://127.0.0.1:0").await?;`
3. Extract port from returned endpoint: pattern-match `zeromq::Endpoint::Tcp(_, port)` to get the OS-assigned port
4. Construct `ipc_addr` string from the endpoint for logging: `"tcp://127.0.0.1:{port}"`
5. Remove the `listener.accept()` timeout block — the worker connects via its own socket after spawning
6. Remove `tokio::io::split(stream)` and the boxed reader/writer construction
7. Deliver the socket via `ipc_tx.send(IpcHandles { socket })`
8. Keep the existing `InitializeHardware` send through the mpsc channel (unchanged)
9. Keep the existing ready-state polling loop (unchanged)

### Step 3 — Rewrite `writer_task`
Replace:
```rust
framing::write_frame(&mut writer, &msg).await
writer.flush().await
```
With:
```rust
let payload = rmp_serde::to_vec_named(&serialize_message(&msg))?;
socket.send(zeromq::ZmqMessage::from(payload)).await?;
```

Where `serialize_message` is the existing `anvilml_ipc::framing::serialize_message` function. Since `framing.rs` is retained in `anvilml-ipc`, the function remains available — we just need to call it directly and pass the resulting bytes to zeromq.

However, since we're removing the `framing` import, we have two options:
(a) Move `serialize_message` into `managed.rs` as a local function (minimal duplication)
(b) Re-export `framing::serialize_message` from `anvilml-ipc` (requires updating `anvilml-ipc` lib.rs)

Option (a) is preferred for minimal scope: copy the ~50-line `serialize_message` function into `managed.rs` as a private helper. The rest of `framing.rs` (`write_frame`, `read_frame`, `worker_event_from_map`) is not needed.

### Step 4 — Rewrite `reader_task`
Replace:
```rust
framing::read_frame(&mut reader, max_mib).await
```
With:
```rust
let msg = socket.recv().await.map_err(|e| ...)?;
let bytes: Vec<u8> = msg.try_into().map_err(|_| ...)?;
let event = rmp_serde::from_slice::<serde_json::Map<String, serde_json::Value>>(&bytes)
    .map_err(|e| AnvilError::Json(e.to_string()))?;
let event = worker_event_from_map(&event)?;
```

The `worker_event_from_map` function (~180 lines) is also in `framing.rs`. Copy it into `managed.rs` as a private helper alongside `serialize_message`.

### Step 5 — Remove `ipc_socket_path` field
Remove the `ipc_socket_path: Arc<std::sync::Mutex<String>>` field from `ManagedWorker` and its initialization in `new()`. The address is already logged at bind time via `info!(ipc_addr = %ipc_addr, "bound IPC socket")`.

### Step 6 — Remove `framing` import
Remove `use anvilml_ipc::framing` from the imports. Keep `use anvilml_ipc::{WorkerEvent, WorkerMessage}`.

### Step 7 — Update test helpers
Rewrite `inject_handles_for_test` to use a zeromq PAIR socket pair for in-process testing:

```rust
#[cfg(test)]
pub async fn inject_handles_for_test(&self) -> zeromq::Socket {
    // Create a PAIR socket pair for test IPC.
    // Supervisor side: bind a PAIR socket.
    let mut supervisor_pair = zeromq::PairSocket::new();
    let endpoint = supervisor_pair.bind("tcp://127.0.0.1:0").await.unwrap();
    let port = match endpoint {
        zeromq::Endpoint::Tcp(_, port) => port,
        _ => panic!("expected TCP endpoint"),
    };

    // Worker side: connect to the bound address.
    let mut worker_pair = zeromq::PairSocket::new();
    worker_pair.connect(&format!("tcp://127.0.0.1:{port}")).await.unwrap();

    // Inject the worker-side socket into the run_loop.
    let mut guard = self.ipc_tx.lock().unwrap();
    if let Some(tx) = guard.take() {
        tx.send(IpcHandles { socket: worker_pair }).unwrap();
    }

    // Return the supervisor-side socket for the test to use.
    supervisor_pair
}
```

The function signature changes from `inject_handles_for_test<S>(&self, stream: S)` to `inject_handles_for_test(&self) -> zeromq::Socket`, and tests must be updated accordingly.

### Step 8 — Update existing tests
- `eof_sets_dead`: Create a PAIR socket pair, inject one end, write a Ready frame (msgpack-serialised flat dict) through the other end, then drop it to trigger EOF. Use `zeromq::Socket::send`/`recv` instead of `framing::write_frame`/`read_frame`.
- `respawn_after_death`: Replace `inject_handles_for_test(pipe_a)` with `inject_handles_for_test()` returning a socket. For respawn, create a fresh PAIR pair and inject the new worker-side socket.
- `keepalive_pings_and_kills_on_timeout`: No IPC changes needed — this test sends Pongs via the broadcast channel and kills via a dummy child process.
- `spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`: These spawn real Python workers. They will need the Python side updated (out of scope for this task) — mark them with `#[ignore]` or skip if Python worker is not available.

### Step 9 — Bump crate version
Increment `anvilml-worker` version in `Cargo.toml`: `0.1.24` → `0.1.25`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Replace TcpListener/TcpStream IPC with zeromq DEALER socket; rewrite IpcHandles, spawn(), writer_task, reader_task, inject_handles_for_test; remove framing import and ipc_socket_path field; copy serialize_message + worker_event_from_map as private helpers |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump version 0.1.24 → 0.1.25 |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `managed.rs` (tests module) | `eof_sets_dead` | EOF on zeromq socket triggers Dead status and WorkerStatusChanged broadcast |
| `managed.rs` (tests module) | `respawn_after_death` | Respawn via fresh zeromq PAIR handles transitions back to Idle |
| `managed.rs` (tests module) | `keepalive_pings_and_kills_on_timeout` | Pong timeout kills worker (no IPC changes needed) |
| `managed.rs` (tests module) | `spawn_ping_pong` | Spawn real Python worker → send Ping → receive Pong (requires Python worker updates, may be ignored) |
| `managed.rs` (tests module) | `status_transitions` | Initializing → Idle → Dead via zeromq IPC |
| `managed.rs` (tests module) | `handshake_completes_once` | Exactly one Ready event during spawn handshake |
| `managed.rs` (tests module) | `spawn_reaches_idle` | spawn() reaches Idle without timing workarounds |

## CI Impact

No CI workflow changes required. The `cargo check --workspace --features mock-hardware` and `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` commands exercise this crate. The zeromq crate is a pure-Rust crate with no platform-specific FFI (it uses libzmq via its own backend), so cross-compilation should be clean. The `libc` dependency for `PDEATHSIG` remains on `#[cfg(unix)]` and is unaffected.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| zeromq 0.4.1 `DealerSocket` API differs from expectations | Medium | High | Verify API shape in PLAN session (done); test with `cargo check` early in ACT session |
| DEALER identity frame causes msg deserialization to fail | Medium | High | Use `send_multipart`/`recv_multipart` if raw send/recv produces framing artifacts; or set `zmq.IDENTITY` option on socket to disable identity frame |
| zeromq PAIR socket for test injection requires both ends connected simultaneously | Low | Medium | Use `inject_handles_for_test` to create the pair in one call, inject one end, keep the other for test use |
| Port extraction from `Endpoint` enum requires pattern matching | Low | Low | `Endpoint::Tcp(_, port)` pattern match; add error handling for unexpected transport types |
| `serialize_message` and `worker_event_from_map` duplication from `framing.rs` | Low | Low | These are ~230 lines total; acceptable duplication for decoupling managed.rs from framing |
| Python worker `ipc.py` not updated in same task, causing integration failure | High (for end-to-end) | High | This task only modifies Rust; Python side is separate task. Mock tests use in-process PAIR sockets and do not require Python worker |

## Acceptance Criteria

- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `IpcHandles` struct holds `zeromq::Socket` (not boxed AsyncRead/AsyncWrite)
- [ ] `spawn()` binds zeromq DEALER socket on `tcp://127.0.0.1:0`, extracts port, passes via `ANVILML_IPC_PORT`
- [ ] `writer_task` uses `socket.send(ZmqMessage::from(...))` instead of `framing::write_frame`
- [ ] `reader_task` uses `socket.recv()` + `rmp_serde::from_slice` instead of `framing::read_frame`
- [ ] `ipc_socket_path` field removed from `ManagedWorker`
- [ ] `anvilml_ipc::framing` import removed from managed.rs
- [ ] `inject_handles_for_test` updated to use zeromq PAIR sockets
- [ ] `anvilml-worker` version bumped to `0.1.25`
- [ ] All managed.rs unit tests pass with `cargo test --workspace --features mock-hardware`
