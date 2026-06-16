# Plan Report: P8-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P8-A2                                       |
| Phase       | 008 — ZeroMQ IPC Transport                  |
| Description | RouterTransport bind and send               |
| Depends on  | P8-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-16T10:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-ipc/src/transport.rs` with a `RouterTransport` struct that wraps a tokio-async ZeroMQ ROUTER socket. Implement `bind()` to bind to `tcp://127.0.0.1:0` (OS-assigned port) and `send()` to transmit a msgpack-encoded `WorkerMessage` to a worker identified by its identity string. The observable state when this task completes is that `cargo test -p anvilml-ipc -- transport` exits 0, confirming a ROUTER socket binds on a non-zero port and can successfully send messages to a connected DEALER socket.

## Scope

### In Scope
- Create `crates/anvilml-ipc/src/transport.rs` with:
  - `pub struct RouterTransport { socket: Arc<tokio::sync::Mutex<zeromq::RouterSocket>>, port: u16 }`
  - `pub async fn bind() -> Result<Self, TransportError>` — creates a `RouterSocket`, binds to `tcp://127.0.0.1:0`, extracts the OS-assigned port from the returned `zeromq::Endpoint`, wraps the socket in `Arc<Mutex<>>`, and returns `Self`.
  - `pub async fn send(&self, worker_id: &str, msg: &WorkerMessage) -> Result<(), TransportError>` — encodes the message via `encode_message()`, constructs a `zeromq::ZmqMessage` with identity frame (`Bytes::from(worker_id)`) + payload frame, acquires the mutex lock, and calls `socket.send()`.
- Create `crates/anvilml-ipc/src/error.rs` with `TransportError` enum covering bind failure, ZeroMQ error, and encoding error.
- Update `crates/anvilml-ipc/src/lib.rs` to `pub mod transport;` and re-export `RouterTransport`, `TransportError`.
- Create `crates/anvilml-ipc/tests/transport_tests.rs` with unit tests.
- Bump `anvilml-ipc` version from `0.1.1` to `0.1.2` in `crates/anvilml-ipc/Cargo.toml`.

### Out of Scope
- `RouterTransport::recv()` — implemented in P8-A3.
- Worker pool integration — implemented in later phases.
- Python worker side — implemented in P8-B1.
- Stress test — implemented in P8-C1.

## Existing Codebase Assessment

The `anvilml-ipc` crate already exists at `crates/anvilml-ipc` with version `0.1.1`. It contains:
- `src/lib.rs` — re-exports `WorkerMessage`, `WorkerEvent`, `encode_message()`, `decode_event()`, and `IpcError` from `messages.rs`.
- `src/messages.rs` — defines `WorkerMessage` and `WorkerEvent` enums with `#[serde(tag = "_type")]` tagged serialization, `encode_message()` and `decode_event()` functions using `rmp-serde`, and the `IpcError` enum.
- `tests/roundtrip_tests.rs` — 15 roundtrip serialization tests for all message/event variants.

The `zeromq` crate (version `0.6.0`) is already declared in the workspace `Cargo.toml` with features `["tokio-runtime", "tcp-transport"]` and pulled into `anvilml-ipc` via `zeromq = { workspace = true }`. The `anvilml-core` crate provides `AnvilError` and `WorkerMessage` types.

Established patterns in this crate:
- Error types use `thiserror::Error` with `#[error("...")]` display attributes.
- Public functions return `Result<T, SpecificError>` (not `Result<T, Box<dyn Error>>`).
- Doc comments on all `pub` items follow the `///` style with description, preconditions, and error variants.
- Tests live in `crates/{name}/tests/` as separate test crate files, importing the crate via `anvilml_ipc::`.
- The crate uses `tracing` for logging (declared as a dependency).

No discrepancy between design doc and source: the design doc section 8.7 specifies the exact struct fields and method signatures that this task implements.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | zeromq     | 0.6.0           | cargo search + lockfile | tokio-runtime, tcp-transport |
| crate  | rmp-serde  | 1.3.1           | workspace Cargo.toml   | (none) |

**Note on feature flag discrepancy:** The task context specifies `features=["tokio"]` but the zeromq 0.6.0 crate uses `tokio-runtime` as the feature name (confirmed via Cargo.lock and zeromq-0.6.0/Cargo.toml). The workspace already declares `zeromq = { version = "0.6.0", features = ["tokio-runtime", "tcp-transport"] }` and `anvilml-ipc/Cargo.toml` uses `zeromq = { workspace = true }`. No dependency changes are needed — the workspace already has the correct feature.

## Approach

1. **Create `crates/anvilml-ipc/src/error.rs`** — Define `TransportError` enum with three variants:
   - `Bind(String)` — for socket bind failures (wraps `zeromq::ZmqError` or `std::io::Error`)
   - `Zmq(zeromq::ZmqError)` — for other ZeroMQ socket errors during send
   - `Encode(String)` — for message encoding failures (wraps `IpcError`)
   
   Implement `std::fmt::Display` and `std::error::Error` via `thiserror::Error`. Use `#[from]` for automatic conversion from `zeromq::ZmqError`. This is a new error type separate from `IpcError` because transport errors (network-level) are conceptually distinct from serialization errors (data-level).

2. **Create `crates/anvilml-ipc/src/transport.rs`** — Define the `RouterTransport` struct and its `impl` block:
   
   a. Define `pub struct RouterTransport { socket: Arc<tokio::sync::Mutex<zeromq::RouterSocket>>, pub port: u16 }` per the design doc §8.7. Use `Arc<tokio::sync::Mutex<>>` (not `std::sync::Mutex`) because the socket methods are async and tokio's mutex is the established pattern in this codebase (confirmed by existing `tokio` dependency in Cargo.toml).
   
   b. Implement `pub async fn bind() -> Result<Self, TransportError>`:
      - Create a new `RouterSocket` via `zeromq::RouterSocket::new()` (from `Socket` trait default impl).
      - Bind to `"tcp://127.0.0.1:0"` using `socket.bind("tcp://127.0.0.1:0").await` (from `Socket` trait).
      - Extract the port from the returned `Endpoint::Tcp(_, port)` — the `bind()` method returns the resolved `Endpoint` which includes the OS-assigned port number.
      - Wrap the socket in `Arc<tokio::sync::Mutex<>>`.
      - Return `Self { socket, port }`.
      - Log at INFO level: `tracing::info!(port = %port, "ROUTER socket bound")` — mandatory INFO log point for IPC subsystem startup.
      - **Rationale:** Binding to port 0 lets the OS pick an available port, which avoids port conflicts when multiple server instances run concurrently.
   
   c. Implement `pub async fn send(&self, worker_id: &str, msg: &WorkerMessage) -> Result<(), TransportError>`:
      - Encode the message via `encode_message(msg)` — this returns `Vec<u8>` via `rmp_serde::to_vec_named()`.
      - Construct a `zeromq::ZmqMessage` with two frames: first frame is `Bytes::from(worker_id.as_bytes())` (the worker identity), second frame is `Bytes::from(encoded_payload)`.
      - Acquire the mutex lock: `let mut socket = self.socket.lock().await`.
      - Call `socket.send(message).await` — the zeromq 0.6.0 `RouterSocket::send()` method pops the first frame as the peer identity internally, then routes the remaining frames to the matching peer.
      - Log at DEBUG level: `tracing::debug!(worker_id = %worker_id, "message sent to worker")` — mandatory DEBUG log point for IPC subsystem.
      - **Rationale:** The ROUTER socket's send API requires the identity frame as the first frame of the multipart message. The socket pops it internally to route to the correct peer. This differs from the DEALER socket where no identity framing is needed.

3. **Update `crates/anvilml-ipc/src/lib.rs`** — Add `pub mod transport;` and re-export `RouterTransport` and `TransportError`. Keep the file under 80 lines (it is currently 12 lines, adding 3 lines keeps it well within the limit).

4. **Create `crates/anvilml-ipc/tests/transport_tests.rs`** — Integration tests that exercise the actual socket:
   - Test `bind()` returns a non-zero port.
   - Test `send()` successfully delivers a message to a connected DEALER socket.
   - Test `send()` to a non-existent worker returns an error (worker not found).

5. **Bump version** — Update `crates/anvilml-ipc/Cargo.toml` version from `0.1.1` to `0.1.2` per the crate version bump convention (§12 of ENVIRONMENT.md).

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| `struct` | `anvilml_ipc::RouterTransport` | `pub struct RouterTransport { socket: Arc<tokio::sync::Mutex<zeromq::RouterSocket>>, pub port: u16 }` |
| `fn` | `anvilml_ipc::RouterTransport::bind` | `pub async fn bind() -> Result<Self, TransportError>` |
| `fn` | `anvilml_ipc::RouterTransport::send` | `pub async fn send(&self, worker_id: &str, msg: &WorkerMessage) -> Result<(), TransportError>` |
| `enum` | `anvilml_ipc::TransportError` | `pub enum TransportError { Bind(String), Zmq(ZmqError), Encode(String) }` |

All items will have `///` doc comments per §12.1 of FORGE_AGENT_RULES.md.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-ipc/src/transport.rs` | RouterTransport struct, bind(), send() |
| CREATE | `crates/anvilml-ipc/src/error.rs` | TransportError enum |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | Add `pub mod transport;`, re-export new types |
| CREATE | `crates/anvilml-ipc/tests/transport_tests.rs` | Unit/integration tests for bind and send |
| MODIFY | `crates/anvilml-ipc/Cargo.toml` | Bump version 0.1.1 → 0.1.2 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-ipc/tests/transport_tests.rs` | `bind_returns_nonzero_port` | `RouterTransport::bind()` binds successfully and returns a non-zero port | None | N/A | `RouterTransport { port, .. }` where `port > 0` | `cargo test -p anvilml-ipc -- transport` exits 0 |
| `crates/anvilml-ipc/tests/transport_tests.rs` | `send_delivers_message_to_dealer` | `RouterTransport::send()` delivers a msgpack-encoded message to a connected DEALER socket | A DEALER socket is connected to the ROUTER's bound address | `worker_id = "test-worker"`, `msg = WorkerMessage::Ping { seq: 1 }` | DEALER socket receives a multipart message with identity frame + encoded payload | `cargo test -p anvilml-ipc -- transport` exits 0 |
| `crates/anvilml-ipc/tests/transport_tests.rs` | `send_to_unknown_worker_returns_error` | `RouterTransport::send()` returns an error when the worker identity is not connected | ROUTER socket bound but no DEALER connected with matching identity | `worker_id = "unknown"`, any `msg` | `Err(TransportError::Zmq(_))` with "Destination client not found" | `cargo test -p anvilml-ipc -- transport` exits 0 |

## CI Impact

No CI changes required. The new test file `tests/transport_tests.rs` is picked up automatically by `cargo test -p anvilml-ipc` which runs in all CI jobs (rust-linux, rust-windows). No new CI gates or jobs are introduced.

## Platform Considerations

None identified. The `tcp://127.0.0.1:0` bind address is platform-neutral. ZeroMQ's TCP transport works identically on Linux and Windows. The `tokio::sync::Mutex` and `Arc` are cross-platform. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The zeromq 0.6.0 `RouterSocket::send()` API requires exactly 2 frames (identity + message) and returns `ZmqError::Socket("ROUTER send requires at least 2 frames")` if fewer frames are provided. Constructing the `ZmqMessage` incorrectly (e.g. single frame) will panic or return an error. | Low | Medium | Verify the frame count before calling `send()`. The implementation constructs exactly 2 frames: identity + payload. Add the `send_to_unknown_worker_returns_error` test to catch this early. |
| The `Endpoint::Tcp(host, port)` pattern matching on the bind result may fail if zeromq returns a non-TCP endpoint (e.g. IPC). | Very Low | High | The bind address is hardcoded to `"tcp://127.0.0.1:0"` which guarantees a TCP endpoint. Pattern match with a `None` fallback that returns an error with a descriptive message. |
| `Arc<Mutex<RouterSocket>>` may cause contention if multiple tasks call `send()` concurrently. | Low | Low | Contention is negligible at the IPC message rate expected (one message per job dispatch, not per-packet). The design doc §8.7 confirms this is the intended pattern. If contention becomes a problem, `RouterSocket::split()` can be used in a future task. |
| The `bytes` crate is needed to construct `Bytes::from()` for `ZmqMessage` frames, but it is not a direct dependency of `anvilml-ipc`. | Low | Medium | `bytes` is a transitive dependency of `zeromq` (zeromq 0.6.0 depends on `bytes` for `ZmqMessage` frame storage). It will be available at compile time. If it is not, add it as a direct dependency with a minimal version pin. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-ipc -- transport` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `grep "^pub mod transport;" crates/anvilml-ipc/src/lib.rs` finds the module declaration
- [ ] `grep "^pub use.*RouterTransport" crates/anvilml-ipc/src/lib.rs` finds the re-export
- [ ] `grep "^pub use.*TransportError" crates/anvilml-ipc/src/lib.rs` finds the re-export
- [ ] `grep 'version = "0.1.2"' crates/anvilml-ipc/Cargo.toml` confirms version bump
