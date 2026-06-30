# Plan Report: P7-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-B1                                       |
| Phase       | 007 — IPC Foundations                       |
| Description | anvilml-ipc: RouterTransport struct + bind() |
| Depends on  | P7-A4                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-30T19:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the `RouterTransport` struct and its `bind()` constructor in `crates/anvilml-ipc/src/transport.rs`, implementing the exact split-lock ownership shape specified in `ANVILML_DESIGN.md §8.3`. This establishes the ROUTER socket wrapper that Phase 8's worker pool will consume via `Arc<RouterTransport>`. The struct binds a ZeroMQ ROUTER socket on `tcp://127.0.0.1:0` (OS-assigned port), splits the socket into independent send/recv halves, and exposes the assigned port. `send()` and `recv()` methods are explicitly deferred to P7-B2.

## Scope

### In Scope
- Create `crates/anvilml-ipc/src/transport.rs` with the `RouterTransport` struct and `bind()` method.
- Add `zeromq` crate dependency to `crates/anvilml-ipc/Cargo.toml`.
- Add `mod transport;` and `pub use transport::RouterTransport;` to `crates/anvilml-ipc/src/lib.rs`.
- Add `>=3` tests in `crates/anvilml-ipc/tests/roundtrip_tests.rs` verifying: bind succeeds with nonzero port, two binds get different ports, the port is listening.

### Out of Scope
- `send()` and `recv()` methods on `RouterTransport` — deferred to P7-B2, which genuinely covers the send/recv split-lock implementation per its `description` and `context` fields in `tasks_phase007.json`.
- `EventBroadcaster` WebSocket fan-out — covered by P7-C1.
- `lib.rs` re-export pass and 80-line check — covered by P7-D1.

## Existing Codebase Assessment

`anvilml-ipc` exists as a partially-built stub crate (Phase 1's P1-B4). Three of its four source modules are already implemented: `error.rs` defines `IpcError` with six variants and the `From<IpcError> for AnvilError` conversion; `messages.rs` defines both `WorkerMessage` (five variants) and `WorkerEvent` (nine variants including all job-lifecycle variants) with `#[serde(tag = "_type")]` for msgpack serialisation; `ws/broadcaster.rs` implements the `EventBroadcaster` wrapping `tokio::sync::broadcast::Sender<WsEvent>` with 1024-event buffer capacity.

The existing patterns are: `thiserror::Error` derives with `#[error("...")]` display macros; `#[serde(tag = "_type")]` enum serialisation; `#[tokio::test]` for async tests; `rmp-serde::to_vec_named` / `rmp_serde::from_slice` for msgpack roundtrip verification in integration tests. The `lib.rs` follows the re-exports-only pattern (9 lines).

No `transport.rs` exists yet — this task creates it from scratch. The design doc's §8.3 provides the exact struct shape (two `Arc<Mutex<...>>` fields, never a single shared lock) which is the primary risk to implement correctly.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | zeromq  | 0.6.0           | rust-docs MCP  | tokio-runtime, all-transport |

The zeromq 0.6.0 crate (released 2026-05-04, MSRV 1.85.0) provides:
- `zeromq::router::RouterSocket` — the ROUTER socket type
- `zeromq::router::RouterSocket::new()` — creates a new socket
- `zeromq::router::RouterSocket::split(self)` — returns `(RouterSendHalf, RouterRecvHalf)`, confirmed via `rust-docs_search_docs` and `rust-docs_get_doc_item`
- `zeromq::Socket` trait — provides `bind(&mut self, &str) -> ZmqResult<Endpoint>` and `new()`
- `zeromq::router::RouterSendHalf` — implements `SocketSend::send(&mut self, ZmqMessage)`
- `zeromq::router::RouterRecvHalf` — implements `SocketRecv::recv(&mut self) -> ZmqResult<ZmqMessage>`
- `zeromq::endpoint::Endpoint` — enum with `Tcp(Host, u16)` variant; pattern-match extracts the port

Default features (`tokio-runtime`, `all-transport`) provide `tokio` runtime support and TCP transport — both required for this task. The `tokio-runtime` feature is needed because `RouterTransport` wraps the halves in `tokio::sync::Mutex`.

## Approach

**Step 1: Add zeromq dependency to Cargo.toml.**
Add `zeromq = { version = "0.6.0", default-features = false, features = ["tokio-runtime", "all-transport"] }` to `[dependencies]` in `crates/anvilml-ipc/Cargo.toml`. Default features are disabled explicitly to avoid pulling in `async-std-runtime` (which would conflict with the project's tokio-only runtime), while `tokio-runtime` and `all-transport` (which enables TCP transport) are the two features actually needed.

**Step 2: Create `crates/anvilml-ipc/src/transport.rs`.**

Define the `RouterTransport` struct exactly as specified in `ANVILML_DESIGN.md §8.3`:

```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use zeromq::prelude::*;
use zeromq::router::{RouterSendHalf, RouterRecvHalf, RouterSocket};
use zeromq::endpoint::Endpoint;

use crate::IpcError;

/// The Rust-side ZeroMQ ROUTER socket wrapper.
///
/// Binds on construction. Ownership rule: constructed exactly once by `WorkerPool`
/// and shared via `Arc<RouterTransport>`. No other code holds the socket directly.
///
/// The send and receive halves are split into independent `tokio::sync::Mutex` guards
/// at construction time — this is the fix for a v3 shutdown deadlock where a blocked
/// `recv()` held the same lock a concurrent `send()` needed.
pub struct RouterTransport {
    sender: Arc<Mutex<RouterSendHalf>>,
    receiver: Arc<Mutex<RouterRecvHalf>>,
    pub port: u16,
}
```

Implement `bind()`:

```rust
impl RouterTransport {
    /// Bind a ROUTER socket on `tcp://127.0.0.1:0` (OS-assigned port),
    /// split into independent send/recv halves, and return the transport.
    ///
    /// The socket is bound on the loopback interface only — workers connect
    /// via `tcp://127.0.0.1:{port}` using their worker_id as the ZeroMQ identity.
    pub async fn bind() -> Result<Self, IpcError> {
        // Create a new ROUTER socket. RouterSocket::new() is a synchronous
        // constructor that produces an unbound socket ready for bind().
        let mut socket = RouterSocket::new();

        // Bind to tcp://127.0.0.1:0 — the OS assigns an available port.
        // The bind() method is provided by the zeromq::Socket trait, which
        // RouterSocket implements. It returns ZmqResult<Endpoint>.
        let endpoint = socket
            .bind("tcp://127.0.0.1:0")
            .await
            .map_err(|e| IpcError::BindFailed(e.to_string()))?;

        // Extract the port number from the returned Endpoint.
        // The endpoint is Tcp(Host, u16) — pattern match to get the port.
        let port = match endpoint {
            Endpoint::Tcp(_, p) => p,
            _ => return Err(IpcError::BindFailed(format!("unexpected endpoint type: {endpoint:?}"))),
        };

        // Split the socket into independent send/recv halves.
        // split(self: Self) consumes the original socket and returns
        // (RouterSendHalf, RouterRecvHalf). This is the structural fix
        // for the v3 shutdown deadlock — each half is wrapped in its own
        // Arc<Mutex<>> so concurrent send and recv never contend on the same lock.
        let (send_half, recv_half) = socket.split();

        Ok(RouterTransport {
            sender: Arc::new(Mutex::new(send_half)),
            receiver: Arc::new(Mutex::new(recv_half)),
            port,
        })
    }
}
```

**Step 3: Update `crates/anvilml-ipc/src/lib.rs`.**
Add `pub mod transport;` and `pub use transport::RouterTransport;` after the existing `ws` module declaration. The file should remain well under the 80-line hard cap (currently 9 lines, adding 2 more).

**Step 4: Add tests to `crates/anvilml-ipc/tests/roundtrip_tests.rs`.**
Append three tests to the existing test file (which already contains EventBroadcaster and msgpack roundtrip tests):

1. `test_bind_returns_nonzero_port` — calls `RouterTransport::bind()`, asserts `port > 0`.
2. `test_two_binds_get_different_ports` — creates two `RouterTransport` instances concurrently, asserts their ports differ.
3. `test_bind_port_is_listening` — calls `bind()`, uses a TCP connection attempt (or `std::net::TcpStream::connect`) to confirm the port is actually listening.

These tests use `#[tokio::test]` for async support. The concurrent bind test uses `tokio::task::JoinHandle` to spawn two binds in parallel.

**Step 5: Bump `anvilml-ipc` crate version** from `0.1.5` to `0.1.6` in `Cargo.toml` (per §4.8 of ANVILML_DESIGN.md and §12 of ENVIRONMENT.md).

## Public API Surface

| Item | Location | Signature / Shape |
|------|----------|-------------------|
| `RouterTransport` struct | `crates/anvilml-ipc/src/transport.rs` | `pub struct RouterTransport { sender: Arc<Mutex<RouterSendHalf>>, receiver: Arc<Mutex<RouterRecvHalf>>, pub port: u16 }` |
| `RouterTransport::bind()` | `crates/anvilml-ipc/src/transport.rs` | `pub async fn bind() -> Result<Self, IpcError>` |

No other public items are introduced. `IpcError` is already defined in `error.rs` and re-exported from `lib.rs`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-ipc/src/transport.rs` | `RouterTransport` struct + `bind()` method |
| MODIFY | `crates/anvilml-ipc/Cargo.toml` | Add `zeromq` dependency; bump patch version 0.1.5 → 0.1.6 |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | Add `pub mod transport;` and `pub use transport::RouterTransport;` |
| MODIFY | `crates/anvilml-ipc/tests/roundtrip_tests.rs` | Add 3 new tests for `bind()` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_bind_returns_nonzero_port` | `bind()` succeeds and returns a port > 0 | None | None | `RouterTransport` with `port > 0` | `cargo test -p anvilml-ipc --test roundtrip_tests test_bind_returns_nonzero_port` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_two_binds_get_different_ports` | Two concurrent binds produce different port numbers | None | None | Two `RouterTransport` instances with different `port` values | `cargo test -p anvilml-ipc --test roundtrip_tests test_two_binds_get_different_ports` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_bind_port_is_listening` | The bound port is actually listening on TCP | None | None | `TcpStream::connect` to the port succeeds | `cargo test -p anvilml-ipc --test roundtrip_tests test_bind_port_is_listening` exits 0 |

## CI Impact

No CI changes required. The task adds a new source file (`transport.rs`) and new tests in an existing test file (`roundtrip_tests.rs`). The existing `rust-linux` and `rust-windows` CI jobs already run `cargo test --workspace --features mock-hardware` which will pick up the new crate module and tests automatically. The `zeromq` dependency is added to `anvilml-ipc` which is already part of the workspace.

## Platform Considerations

None identified. The `tcp://127.0.0.1:0` bind address is platform-neutral — loopback TCP is available on both Linux and Windows. The zeromq crate's `all-transport` feature (which includes `tcp-transport`) works identically on both platforms. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed for `bind()`.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `RouterSocket::split(self)` consumes the socket by value, but `bind()` takes `&mut self` — the order of operations (bind first, then split) must be correct. Calling `split()` before `bind()` would leave the socket unbound. | Low | High | Follow the exact sequence from §8.3: create → bind → split. The borrow checker prevents calling `bind()` after `split()` because `split` consumes `self`, so the compiler enforces correct ordering. |
| The `zeromq` crate's `Endpoint::Tcp(Host, u16)` variant may not be public in version 0.6.0, preventing extraction of the port number from the `bind()` return value. | Low | Medium | If `Endpoint` is not publicly constructible/pattern-matchable, fall back to `socket.getsockopt(zmq::LocalAddr)` or query the bound address via the socket's backend. Verify this at ACT time. |
| `tokio::sync::Mutex` on `RouterSendHalf`/`RouterRecvHalf` may have different pinning requirements than `std::sync::Mutex` — the halves implement `SocketSend`/`SocketRecv` which take `&mut self`, but `Arc<Mutex<T>>` requires `.lock().await` which returns a pinned guard. The ACT agent must use `.lock().await` (not `.lock().unwrap()`) for any future send/recv methods. | Low | Medium | This task only creates the struct and `bind()` — no `.lock()` calls are needed in `bind()`. Document this in `lib.rs` and `transport.rs` comments for P7-B2's benefit. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-ipc --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-ipc --test roundtrip_tests test_bind_returns_nonzero_port` exits 0
- [ ] `cargo test -p anvilml-ipc --test roundtrip_tests test_two_binds_get_different_ports` exits 0
- [ ] `cargo test -p anvilml-ipc --test roundtrip_tests test_bind_port_is_listening` exits 0
- [ ] `cargo test -p anvilml-ipc --test roundtrip_tests` exits 0 (all tests including pre-existing ones)
