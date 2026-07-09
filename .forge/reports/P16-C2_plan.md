# Plan Report: P16-C2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P16-C2                                      |
| Phase       | 16 — Live Events                              |
| Description | anvilml-server: ws_handler forward loop + Lagged disconnect |
| Depends on  | P16-C1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-09T18:35:00Z                        |
| Attempt     | 1                                           |

## Objective

Extend `handle_socket()` in `crates/anvilml-server/src/ws/handler.rs` from the P16-C1 skeleton (subscribe → send one SystemStats frame → return) into a full WebSocket forward loop that continuously receives `WsEvent`s from the broadcaster's broadcast channel, serialises each as JSON text, and sends it to the connected client. On `tokio::sync::broadcast::RecvError::Lagged` (consumer fell >1024 events behind), the connection is closed with a Close frame rather than attempting to catch up — per `ANVILML_DESIGN.md §13.6`, the client must reconnect. This completes Group C of Phase 16's WebSocket handler.

## Scope

### In Scope
- Modify `handle_socket()` in `crates/anvilml-server/src/ws/handler.rs` to implement the ongoing forward loop:
  - Subscribe to `state.broadcaster` (already done in P16-C1).
  - Send the initial `SystemStats` frame (already done in P16-C1).
  - Loop: receive from `Receiver<WsEvent>`, serialize with `serde_json::to_string()`, send as `Message::text()`.
  - On `RecvError::Lagged(n)`: log at WARN with the lag count, send `Message::Close`, then break the loop.
  - On `RecvError::Closed`: log at ERROR, send `Message::Close`, break.
  - On `socket.send()` failure: log at WARN, break the loop (graceful disconnect).
- Add `#[tracing::instrument]` to `handle_socket()`.
- Add INFO log for connection established, WARN log for lagged disconnect, ERROR log for closed channel.
- Add >=4 new integration tests in `handler_tests.rs` covering event forwarding, lagged disconnect, and concurrent clients.
- Bump `anvilml-server` crate version from 0.1.13 → 0.1.14.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. The P16-C1 deferred forward loop is being completed here. No stubs, no deferred functionality.

## Existing Codebase Assessment

**What already exists (P16-C1):** `handle_socket()` in `handler.rs` already creates a subscription via `state.broadcaster.subscribe()`, sends an initial placeholder `SystemStats` frame as JSON, and then returns — dropping the subscription. The connection skeleton is structurally correct; only the ongoing forward loop is missing. The `ws_handler()` upgrade function, `AppState` (with `broadcaster: Arc<EventBroadcaster>`), and the `/v1/events` route registration in `build_router()` are all already wired.

**Established patterns:**
- **Error handling:** The existing code uses `let Ok(x) = expr else { return; }` for non-critical failures and `let _ = socket.send(...).await` for best-effort sends. The forward loop will extend this pattern: send failures break the loop (graceful disconnect) rather than propagating errors.
- **Logging:** The existing codebase uses `tracing` with structured fields. The handler module already imports `tracing` transitively via other crates.
- **Test style:** `handler_tests.rs` uses real TCP listeners on ephemeral ports, `axum::serve()` in a background task, and `tokio_tungstenite::connect_async()` for the client side. Tests use `futures_util::StreamExt::next()` to read messages.
- **Serialization:** `WsEvent` uses `#[serde(tag = "type", rename_all = "snake_case")]`, so `serde_json::to_string()` produces JSON with a `"type"` field matching the variant name in snake_case.

**Gap between design doc and source:** None significant. The design doc (§13.6) specifies the exact connect sequence (subscribe → send SystemStats → forward) and the lag-disconnect rule, which the existing `handler.rs` module doc comment already documents as the target for P16-C2. The `EventBroadcaster` already uses a 1024-event buffer as specified.

## Resolved Dependencies

| Type   | Name          | Version verified | MCP source     | Feature flags confirmed |
|--------|--------------|-----------------|----------------|------------------------|
| crate  | tokio        | 1.52.3          | rust-docs MCP  | full (dev-dep in Cargo.toml) |
| crate  | serde_json   | 1.0 (workspace) | Cargo.toml     | none needed             |

No new dependencies are introduced. The existing `serde_json` dependency already covers `to_string()` on `WsEvent`. The `tokio::sync::broadcast::Receiver` and `tokio::sync::broadcast::error::RecvError` types are part of the `tokio` crate, already available via the `full` feature in dev-dependencies.

Confirmed via MCP: `tokio::sync::broadcast::error::RecvError` has two variants — `Closed` and `Lagged(u64)` — matching the task's requirements exactly. The `Lagged(n)` variant carries the number of messages skipped.

## Approach

### Step 1: Modify `handle_socket()` — add the forward loop

Replace the existing `handle_socket()` body (which subscribes, sends one frame, and returns) with a loop that:

1. **Subscribe** — already present, keep as-is. Store the receiver in a named variable (not `_`) so it can be used in the loop.
2. **Send initial frame** — already present, keep as-is.
3. **Enter the forward loop** — add a `while let Ok(event) = receiver.recv().await` loop:
   - Inside the loop, serialize the event: `let Ok(json) = serde_json::to_string(&event)`.
   - On serialization failure, log at WARN and continue to the next event (one bad event should not drop the entire connection).
   - Send the JSON as a text frame: `socket.send(Message::text(json)).await`.
   - On send success, `continue` to the next iteration.
   - On send failure (any error), log at WARN with the error description and `break` the loop (graceful disconnect).
4. **Handle `RecvError`** — the `while let` pattern alone won't catch `Lagged`/`Closed`. Instead, use a `match` on `receiver.recv().await`:
   - `Ok(event)` → serialize and send.
   - `Err(RecvError::Lagged(n))` → log at WARN: `lagged_disconnect = %n`, send Close frame, break.
   - `Err(RecvError::Closed)` → log at ERROR, send Close frame, break.

The loop replaces the current "subscribe → send once → return" pattern. The subscription stays alive for the entire connection lifetime.

**Rationale for `while let`/`match` on `recv()`:** Unlike `StreamExt::next()`, `tokio::sync::broadcast::Receiver::recv()` returns `Result<T, RecvError>` directly as an async operation. The `Lagged` variant must be handled explicitly (disconnect the client), not silently skipped. A `match` is clearer than `while let` here because we need different actions for `Ok` vs `Err`.

### Step 2: Add logging

Add the following log calls inside `handle_socket()`:
- `tracing::info!(client_addr = %addr, "WebSocket client connected");` — at connection start. The client address can be derived from the socket's peer (or omitted if not available from the axum WebSocket type; in that case, log without the address).
- `tracing::warn!(lagged_by = %n, "WebSocket client lagged behind, closing connection");` — on `RecvError::Lagged(n)`.
- `tracing::error!("WebSocket broadcast channel closed");` — on `RecvError::Closed`.
- `tracing::warn!(error = %e, "WebSocket send failed, disconnecting client");` — on socket send failure.

**Note on client address:** `axum::extract::ws::WebSocket` does not expose the peer address directly. Log without `client_addr` or use a connection counter for identification.

### Step 3: Add `#[tracing::instrument]`

Add `#[tracing::instrument(skip(socket, state))]` to `handle_socket()`. This creates a span for the entire connection lifetime, which is a meaningful unit of work (one WebSocket connection).

### Step 4: Write integration tests in `handler_tests.rs`

Add 4 new tests following the existing test file's pattern (real TCP listener + `axum::serve()` + `tokio_tungstenite` client):

**Test 1 — `test_forwarded_event_is_json_text`:** After connecting and receiving the initial SystemStats frame, the test publishes a `JobQueued` event via `state.broadcaster.publish()`, then reads the next message from the client stream. It asserts the message is `ClientMessage::Text` and the JSON contains `"type":"job_queued"`.

**Test 2 — `test_lagged_error_closes_connection`:** The test publishes >1024 events rapidly (filling and overflowing the broadcast buffer) while the client is idle (not calling `recv()`). When the client finally calls `recv()`, it should get `RecvError::Lagged`, and the handler should send a Close frame and exit. The test asserts the stream yields a Close frame (or ends) rather than panicking.

**Test 3 — `test_concurrent_clients_independent_copies`:** Connect two clients. Publish one event. Both clients must receive their own copy of the event as a Text frame. This verifies independent subscriptions (the existing `test_multiple_clients_each_receive_independent_initial_frame` tests this for the initial frame; this extends it to forwarded events).

**Test 4 — `test_lagged_disconnect_no_panic`:** Similar to Test 2 but explicitly verifies that the server task does not panic or produce an error when the Lagged error occurs. The test publishes >1024 events, then verifies the server is still running (e.g., by connecting a new client and receiving its initial frame successfully).

### Step 5: Bump crate version

Increment `crates/anvilml-server/Cargo.toml` `[package] version` from `0.1.13` to `0.1.14`.

## Public API Surface

No new `pub` items are introduced. The only change is to the internal `handle_socket()` function, which is `async fn` (not `pub`). The public API surface of `anvilml-server` is unchanged.

| Item | Path | Change |
|------|------|--------|
| `handle_socket()` | `anvilml-server::ws::handler::handle_socket` | Modified: gains a forward loop body; signature unchanged |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-server/src/ws/handler.rs` | Implement forward loop in `handle_socket()`, add logging, add `#[tracing::instrument]` |
| MODIFY | `crates/anvilml-server/tests/handler_tests.rs` | Add >=4 new integration tests |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.13 → 0.1.14 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/handler_tests.rs` | `test_forwarded_event_is_json_text` | A published WsEvent after connect is forwarded as JSON text frame | Server running, client connected | Publish `JobQueued` via `broadcaster.publish()` | Client receives `ClientMessage::Text` with `"type":"job_queued"` | `cargo test -p anvilml-server --test handler_tests test_forwarded_event_is_json_text` exits 0 |
| `crates/anvilml-server/tests/handler_tests.rs` | `test_lagged_error_closes_connection` | A Lagged error causes the handler to send Close and exit, not panic | Server running, client connected but idle | Publish >1024 events rapidly | Client stream yields Close frame or ends; no server panic | `cargo test -p anvilml-server --test handler_tests test_lagged_error_closes_connection` exits 0 |
| `crates/anvilml-server/tests/handler_tests.rs` | `test_concurrent_clients_independent_copies` | Multiple concurrent clients each receive their own independent copy of forwarded events | Server running, two clients connected | Publish one event | Both clients receive Text frame with same event JSON | `cargo test -p anvilml-server --test handler_tests test_concurrent_clients_independent_copies` exits 0 |
| `crates/anvilml-server/tests/handler_tests.rs` | `test_lagged_disconnect_no_panic` | After a Lagged disconnect, the server remains operational for new connections | Server running | Publish >1024 events, then connect a new client | New client receives initial SystemStats frame successfully | `cargo test -p anvilml-server --test handler_tests test_lagged_disconnect_no_panic` exits 0 |

Acceptance command for total test count:
```bash
cargo test -p anvilml-server --test handler_tests
# -> >=7 tests total, exits 0
```

## CI Impact

No CI changes required. The task modifies existing test files and source files within the `anvilml-server` crate. The existing CI job `rust-linux` (which runs `cargo test --workspace --features mock-hardware`) already compiles and tests all crates including `anvilml-server`. No new CI jobs, gates, or configuration files are introduced.

## Platform Considerations

None identified. The forward loop uses only `tokio::sync::broadcast::Receiver::recv()`, `serde_json::to_string()`, and `axum::extract::ws::WebSocket::send()` — all platform-neutral async operations. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tokio_tungstenite` client stream does not surface the server's Close frame cleanly — the test may see `None` (stream end) instead of `Close`. This is a known behaviour difference between tungstenite and other WebSocket libraries. | Medium | Medium | Write the test to accept both `None` (stream ended) and `Ok(Close(_))` as valid outcomes for the lagged-disconnect test, matching the pattern already used in `test_handler_sends_exactly_one_frame_then_returns`. |
| Publishing >1024 events in the lag test may take non-deterministic time depending on system load, causing test timeouts. | Low | Medium | Use a bounded publish loop with a short `tokio::time::sleep` between publishes to ensure events are queued rather than dropped before the client can observe them. Set a reasonable timeout on the client's recv. |
| The broadcast channel's 1024-buffer overflow may silently discard events before the client's `recv()` call — the `Lagged(n)` value may be smaller than expected if events are published faster than the buffer fills. | Low | Low | The test only needs to verify that *some* Lagged error occurs (not a specific count). The `Err(RecvError::Lagged(_))` match arm handles any lag count uniformly. |
| `serde_json::to_string()` on `WsEvent` could fail for events with unrepresentable data (e.g., very long strings). | Very Low | Low | On serialization failure, log at WARN and continue to the next event rather than breaking the connection. This is consistent with the "best-effort" design philosophy of the broadcast channel. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test handler_tests` exits 0 with >=7 tests total
- [ ] `cargo test -p anvilml-server --test handler_tests test_forwarded_event_is_json_text` exits 0
- [ ] `cargo test -p anvilml-server --test handler_tests test_lagged_error_closes_connection` exits 0
- [ ] `cargo test -p anvilml-server --test handler_tests test_concurrent_clients_independent_copies` exits 0
- [ ] `cargo test -p anvilml-server --test handler_tests test_lagged_disconnect_no_panic` exits 0
- [ ] `cargo clippy -p anvilml-server --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
