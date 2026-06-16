# Plan Report: P8-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P8-A3                                       |
| Phase       | 008 — ZeroMQ IPC Transport                  |
| Description | anvilml-ipc: RouterTransport recv with identity routing |
| Depends on  | P8-A1, P8-A2                                |
| Project     | anvilml                                     |
| Planned at  | 2026-06-16T12:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement `pub async fn recv(&self) -> Result<(String, WorkerEvent), AnvilError>` on `RouterTransport` in `crates/anvilml-ipc/src/transport.rs`. The method receives a multipart message from the ZeroMQ ROUTER socket, extracts the worker identity as a UTF-8 string, decodes the msgpack payload into a `WorkerEvent` using `decode_event()`, and returns `(worker_id, event)`. This enables the worker pool's event loop to receive heartbeats, job completions, and lifecycle events from Python workers. After implementation, `cargo test -p anvilml-ipc -- roundtrip` exits 0 with a new in-process roundtrip test that sends `WorkerEvent::Pong{seq:42}` via a DEALER socket and verifies `recv()` returns the correct `(worker_id, Pong{seq:42})` tuple.

## Scope

### In Scope
- **`crates/anvilml-ipc/src/transport.rs`**: Add `pub async fn recv(&self) -> Result<(String, WorkerEvent), AnvilError>` method to `RouterTransport`. The method acquires the mutex lock, calls `socket.recv().await` on the `RouterSocket`, extracts the identity frame as UTF-8 string, extracts the payload frame, and calls `decode_event()` on the payload bytes.
- **`crates/anvilml-ipc/tests/transport_tests.rs`**: Add `recv_roundtrip` async test that creates a `RouterTransport`, connects an in-process `DealerSocket` with a fixed identity `"test-worker-0"`, sends a `WorkerEvent::Pong{seq:42}` via the ROUTER socket, then calls `transport.recv()` and asserts the returned `(worker_id, event)` matches.
- **`crates/anvilml-ipc/Cargo.toml`**: Add `anvilml-core` dependency (already present as path dependency — verify it is available).
- **`crates/anvilml-ipc/Cargo.toml`**: Bump patch version from `0.1.2` to `0.1.3`.

### Out of Scope
- Adding `recv_timeout` or deadline-based receive (future task).
- Adding `TransportError::Decode` variant (using `AnvilError::Ipc` for decode failures per design doc).
- Modifying the existing `send()` method or its return type.
- Adding stress tests (handled by P8-C1).
- Modifying `messages.rs`, `error.rs`, or `lib.rs`.

## Existing Codebase Assessment

The `anvilml-ipc` crate already has `RouterTransport::bind()` and `RouterTransport::send()` implemented in `transport.rs` (191 lines). The `send()` method uses `TransportError` as its error type, while the design doc (§8.7) specifies `AnvilError` for `recv`. The `anvilml-core` crate is already a dependency of `anvilml-ipc` (via path), so `AnvilError` is accessible without adding new dependencies. The `zeromq` crate is at version 0.6.0 (confirmed via lockfile; MCP unavailable).

The existing `transport_tests.rs` already demonstrates the ROUTER socket recv pattern: `sock.recv().await` returns a `ZmqMessage`, and `msg.get(0)` returns `Option<&Bytes>` for frame access. The test shows ROUTER recv produces `[identity, payload]` (two frames) — not three frames as described in TASKS_PHASE008.md's gotcha section. This discrepancy between the task doc and actual zeromq 0.6.0 behavior is noted in Risks and Mitigations.

The `roundtrip_tests.rs` file contains only serialization roundtrip tests (msgpack encode/decode), not socket-level tests. All new socket-level tests go in `transport_tests.rs`.

The established patterns are: `#[tokio::test]` for async tests, `tokio::time::sleep` for connection establishment delays (50ms), `tokio::time::timeout` for receive timeouts (2s), and `tracing::debug!` / `tracing::info!` for structured logging.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | zeromq     | 0.6.0           | Cargo.lock     | tokio-runtime, tcp-transport |
| crate  | rmp-serde  | 1.3.1           | Cargo.lock     | (none)                  |

**Note:** MCP `rust-docs` was unavailable. Versions resolved from `Cargo.lock` per FORGE_AGENT_RULES §6.4 fallback. The zeromq 0.6.0 API shape (`RouterSocket::new()`, `.bind().await`, `.send().await`, `.recv().await` returning `ZmqMessage`) was confirmed via existing usage in `transport_tests.rs`.

## Approach

1. **Add `recv` method to `RouterTransport`** in `crates/anvilml-ipc/src/transport.rs`. The method signature is `pub async fn recv(&self) -> Result<(String, WorkerEvent), AnvilError>`. Implementation:
   - Acquire the mutex lock: `let mut socket = self.socket.lock().await;`
   - Receive the multipart message: `let msg = socket.recv().await?;` (zeromq error maps to `AnvilError::Ipc`)
   - Extract identity frame: use `msg.get(0)` which returns `Option<&Bytes>` per the existing test pattern. Convert to UTF-8 string via `String::from_utf8`. If identity frame is missing, return `AnvilError::Ipc("ROUTER recv returned no identity frame")`.
   - Extract payload frame: use `msg.get(1)` which returns `Option<&Bytes>`. If payload is missing, return `AnvilError::Ipc("ROUTER recv returned no payload frame")`.
   - Decode the payload: call `decode_event(payload_bytes)` and map `IpcError` to `AnvilError::Ipc` via `.map_err(|e| AnvilError::Ipc(e.to_string()))`.
   - Log at DEBUG level: `tracing::debug!(worker_id = %worker_id, event_type = ?event, "event received from worker")`.
   - Rationale for using `AnvilError::Ipc` for both zeromq and decode errors: the design doc (§8.7) specifies `AnvilError` as the return type, and `AnvilError::Ipc` covers "protocol mismatch" per its doc comment.

2. **Add `anvilml_core` import** to `transport.rs` if not already present. The file currently imports from `crate::` for local types. Add `use anvilml_core::AnvilError;` at the top of the file.

3. **Add `recv_roundtrip` test** to `crates/anvilml-ipc/tests/transport_tests.rs`. The test:
   - Creates a `RouterTransport` via `bind().await`.
   - Creates a `DealerSocket` and sets its identity to `"test-worker-0"` using `dealer.set_identity(b"test-worker-0")` or the equivalent zeromq 0.6.0 API.
   - Connects the DEALER to `tcp://127.0.0.1:{transport.port}`.
   - Sleeps 50ms for connection establishment (matching existing test pattern).
   - Sends `WorkerEvent::Pong{seq:42}`: encodes with `encode_message` is not applicable for events; instead, constructs the multipart message manually: `[identity, payload_bytes]` where identity is `b"test-worker-0"` and payload is the msgpack-encoded `Pong{seq:42}`. Sends via the ROUTER socket through the existing `transport.socket` Arc.
   - Calls `transport.recv().await` and asserts `worker_id == "test-worker-0"` and `event == WorkerEvent::Pong{seq:42}`.
   - Uses `tokio::time::timeout(2s, ...)` for the recv call.

4. **Bump `anvilml-ipc` Cargo.toml version** from `0.1.2` to `0.1.3` (patch bump per §12 of ENVIRONMENT.md).

5. **Add logging**: Per ENVIRONMENT.md §9 mandatory DEBUG log points, IPC "Event received from a worker" requires `worker_id=` and `event_type=` structured fields. The recv method will log at DEBUG level after successful decode.

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `recv` | fn | `anvilml_ipc::RouterTransport` | `pub async fn recv(&self) -> Result<(String, WorkerEvent), AnvilError>` |

No new public types or traits. Only one new `pub` method on an existing struct.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-ipc/src/transport.rs` | Add `recv()` async method to `RouterTransport` |
| Modify | `crates/anvilml-ipc/tests/transport_tests.rs` | Add `recv_roundtrip` test |
| Modify | `crates/anvilml-ipc/Cargo.toml` | Bump version 0.1.2 → 0.1.3; verify `anvilml-core` dependency present |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-ipc/tests/transport_tests.rs` | `recv_roundtrip` | In-process DEALER socket sends `WorkerEvent::Pong{seq:42}`; `RouterTransport.recv()` returns correct `(worker_id, event)` pair | `RouterTransport::bind()` succeeds; DEALER connects to ROUTER; zeromq 0.6.0 available | ROUTER bound to OS-assigned port; DEALER identity `"test-worker-0"`; payload `Pong{seq:42}` | `recv()` returns `("test-worker-0", WorkerEvent::Pong{seq:42})` | `cargo test -p anvilml-ipc -- roundtrip` exits 0 |

## CI Impact

No CI changes required. The new test is in the existing `tests/transport_tests.rs` file, which is already picked up by `cargo test -p anvilml-ipc` (the rust-linux and rust-windows CI jobs). No new file types, modules, or CI gates are introduced.

## Platform Considerations

None identified. The ROUTER socket recv API is platform-neutral in zeromq 0.6.0. The identity frame is raw bytes — `String::from_utf8` works identically on Linux and Windows. The TCP loopback address `127.0.0.1` is the same on both platforms. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The zeromq 0.6.0 ROUTER socket recv API may differ from what the existing test pattern assumes — specifically, `ZmqMessage::get(0)` might return `Option<&Vec<u8>>` or `Option<Bytes>` rather than `Option<&Bytes>`, or the message might include a delimiter frame. | Medium | High | Verify the actual `ZmqMessage` API shape at session start via `cargo doc --open` or by inspecting the zeromq 0.6.0 source in `target/debug/build/`. The existing `transport_tests.rs` usage (`msg.get(0)` returning `Option<&Bytes>`) is the ground truth. |
| `DealerSocket::set_identity()` API may differ between zeromq 0.4.x and 0.6.0 — the task context says identity is set before connect, but the exact method name (e.g., `setsockopt`, `set_identity`, `identity`) varies. | Medium | High | Check the zeromq 0.6.0 `DealerSocket` API at session start. If `set_identity` does not exist, use the equivalent (likely `dealer.set_identity(...)` or socket option via `setsockopt`). The MCP tool was unavailable; the Cargo.lock fallback is 0.6.0. |
| `AnvilError` is not currently used in `anvilml-ipc` — adding it as a return type may require adding `anvilml-core` to the dependency list (already present as path dep, so no new dep needed). Existing `send()` returns `TransportError`. | Low | Medium | Verify `anvilml-core` is already in `Cargo.toml` (confirmed: yes, line 7). Use `AnvilError::Ipc` variant for both zeromq and decode errors. |
| The recv method acquires the mutex lock, which blocks all concurrent send() calls. At the expected message rate (keepalive every 30s + sporadic job events), contention is negligible. | Low | Low | No action needed. If contention becomes an issue, a separate reader task can be introduced (out of scope). |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-ipc -- roundtrip` exits 0
- [ ] `cargo test -p anvilml-ipc -- transport` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `grep -n "pub async fn recv" crates/anvilml-ipc/src/transport.rs` returns a line containing `Result<(String, WorkerEvent), AnvilError>`
- [ ] `grep -n "recv_roundtrip" crates/anvilml-ipc/tests/transport_tests.rs` returns a line containing `async fn recv_roundtrip`
