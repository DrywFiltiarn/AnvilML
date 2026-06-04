# Plan Report: P7-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-A1                                             |
| Phase       | 007 — WebSocket Event Stream                      |
| Description | anvilml-server: EventBroadcaster                  |
| Depends on  | P6-A7                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-04T10:50:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create the `EventBroadcaster` struct in `crates/anvilml-server/src/ws/broadcaster.rs`, providing a thin wrapper around `tokio::sync::broadcast::Sender<Arc<WsEvent>>` with `new(capacity)`, `send(event)`, and `subscribe()` methods, plus unit tests verifying send/receive equality and no-error on zero-subscriber sends.

## Scope

### In Scope
- Create `crates/anvilml-server/src/ws/broadcaster.rs` with the `EventBroadcaster` struct
- Implement `new(capacity: usize) -> Self`
- Implement `send(&self, event: WsEvent)` — wraps in `Arc`, calls `sender.send()`, ignores `SendError`
- Implement `subscribe(&self) -> broadcast::Receiver<Arc<WsEvent>>`
- Add `ws` module declaration to `crates/anvilml-server/src/lib.rs` (`mod ws; pub mod broadcaster;`)
- Add `sync` feature to tokio in `crates/anvilml-server/Cargo.toml` (required for `tokio::sync::broadcast`)
- Inline tests in `broadcaster.rs`: subscribe + send + receive equal event; send with no subscribers does not error

### Out of Scope
- WebSocket handler (`src/ws/handler.rs`) — task P7-A2
- Keepalive ping logic — task P7-A3
- System stats tick — task P7-A4
- Startup wiring in `main.rs` — task P7-A5
- Any changes to `anvilml-core` WsEvent types

## Approach

1. **Add `sync` feature to tokio** in `crates/anvilml-server/Cargo.toml`: change the tokio line from `features = ["macros", "rt-multi-thread"]` to `features = ["macros", "rt-multi-thread", "sync"]`. This is required for `tokio::sync::broadcast`.

2. **Create `crates/anvilml-server/src/ws/` directory** — new module directory for WebSocket-related code in this phase.

3. **Create `crates/anvilml-server/src/ws/broadcaster.rs`**:
   - Import `tokio::sync::broadcast`, `anvilml_core::types::events::WsEvent`, and `std::sync::Arc`.
   - Define `pub struct EventBroadcaster { sender: broadcast::Sender<Arc<WsEvent>> }`.
   - Implement `pub fn new(capacity: usize) -> Self` — calls `broadcast::channel(capacity)` and stores the sender.
   - Implement `pub fn send(&self, event: WsEvent)` — wraps in `Arc::<WsEvent>::from(Box::new(event))`, calls `self.sender.send()`, discards the result (no subscribers is fine).
   - Implement `pub fn subscribe(&self) -> broadcast::Receiver<Arc<WsEvent>>` — calls `self.sender.subscribe()`.

4. **Update `crates/anvilml-server/src/lib.rs`**: add `pub mod ws;` and `pub use ws::broadcaster::EventBroadcaster;` (or `mod ws; pub mod broadcaster;`).

5. **Write unit tests** inline in `broadcaster.rs`:
   - Test 1 (`subscribe_send_receive`): create broadcaster, subscribe, send a `WsEvent::SystemStats`, receive on subscriber, assert the received event equals the sent event.
   - Test 2 (`send_no_subscribers_no_error`): create broadcaster with capacity, call `send()` without subscribing — asserts no panic (SendError is silently ignored by design).

6. **Run tests**: `cargo test -p anvilml-server -- broadcaster` to verify both tests pass.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-server/src/ws/broadcaster.rs` | EventBroadcaster struct + unit tests |
| Create | `crates/anvilml-server/src/ws/mod.rs` | Module re-export for ws (or use inline mod in lib.rs) |
| Edit | `crates/anvilml-server/Cargo.toml` | Add `sync` feature to tokio |
| Edit | `crates/anvilml-server/src/lib.rs` | Declare `ws` module, re-export `EventBroadcaster` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/src/ws/broadcaster.rs` | `subscribe_send_receive` | Subscribe → send a WsEvent → receive; received event equals sent event |
| `crates/anvilml-server/src/ws/broadcaster.rs` | `send_no_subscribers_no_error` | Send with zero subscribers does not panic or error |

## CI Impact

No CI workflow files are modified. The existing `cargo test --workspace --features mock-hardware` gate in the CI matrix will automatically include the new tests once they pass locally. No OpenAPI drift gate needed (no handler signatures changed).

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Missing `sync` feature on tokio causes compile error | Add `sync` to tokio features in Cargo.toml before writing broadcaster.rs |
| `WsEvent` derives (Serialize, Deserialize) not compatible with broadcast channel — broadcast requires `Clone`, WsEvent already derives Clone per anvilml-core events.rs | Verify WsEvent derives Clone; it does (confirmed in events.rs line 166). Arc<WsEvent> is used to avoid per-subscriber clone of the enum payload. |
| Test needs a valid WsEvent variant to send — all variants require fields (event string, timestamp, etc.) | Use `WsEvent::SystemStats` with minimal constructed fields in tests; `Utc::now()` for timestamp |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server -- broadcaster` exits 0 (both subscribe/send and no-subscribers tests pass)
- [ ] `EventBroadcaster::new(capacity)` creates a broadcast channel with the given capacity
- [ ] `EventBroadcaster::send(event)` wraps in Arc, sends on channel, ignores SendError
- [ ] `EventBroadcaster::subscribe()` returns a new receiver that can receive events sent after subscription
- [ ] Sending to a broadcaster with zero subscribers does not panic or propagate an error
- [ ] `cargo clippy --package anvilml-server --features mock-hardware -- -D warnings` passes (no lint errors from the new code)
