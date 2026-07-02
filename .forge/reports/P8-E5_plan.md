# Plan Report: P8-E5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P8-E5                                         |
| Phase       | 008 — IPC Stress Gate & Worker Pool          |
| Description | anvilml-worker: wire KeepaliveWatchdog as second crash source |
| Depends on  | P8-E4, P8-C2                                  |
| Project     | anvilml                                       |
| Planned at  | 2026-07-02T22:15:00Z                          |
| Attempt     | 1                                             |

## Objective

Wire `KeepaliveWatchdog` (built in P8-C2) into `ManagedWorker::run()` so that a worker
that hangs or dies silently — producing no transport error — is detected by the keepalive
ping/pong timeout and triggers the same crash path as a transport recv error. This closes
the gap flagged by a `TODO(P8-E5)` comment in `keepalive.rs` that has been present since
P8-C2 shipped.

## Scope

### In Scope
- Remove `#[allow(dead_code)]` from `RouterTransportAdapter` in `keepalive.rs`.
- Add `watchdog_dead_rx: oneshot::Receiver<()>` and `pong_tx: mpsc::Sender<WorkerEvent>`
  fields to `ManagedWorker`.
- Update `ManagedWorker::new()` to accept `pong_tx: mpsc::Sender<WorkerEvent>` (the
  caller creates the channel pair and passes the sender).
- In `run()`'s single-generation loop (not the respawn loop — that is P8-E6), at the top
  of the loop: create a `oneshot::channel()` for `dead_rx`, construct a
  `KeepaliveWatchdog` wrapping `Arc::clone(&self.transport)` in `RouterTransportAdapter`,
  with production defaults `ping_interval = 30s`, `pong_timeout = 10s`, spawn
  `watchdog.run()`, and add a third `tokio::select!` branch on `dead_rx`.
- The `dead_rx` branch appends to `attempt_history`, calls `should_respawn()`, logs
  `crash_respawn_decision`, sets status to `Dead`, and breaks — identical to the existing
  transport-error branch.
- In `handle_event()`'s `WorkerEvent::Pong` arm, forward the event via
  `pong_tx.send(event).await` (best-effort `try_send` is acceptable; a closed/full
  channel is not itself an error — use `try_send` since `handle_event` is async but the
  send must not block the event loop).
- Update existing tests that construct `ManagedWorker` to pass the new `pong_tx` argument.
- Add ≥4 new tests in `managed_tests.rs`.
- Bump `anvilml-worker` patch version (0.1.14 → 0.1.15).

### Out of Scope

defers_to (from JSON): []

No scope is deferred — this task's `defers_to` field is empty. No Out of Scope bullets
may name a deferral target.

## Existing Codebase Assessment

**What exists:** `KeepaliveWatchdog<T: Transport>` is fully implemented in
`keepalive.rs` with a generic `Transport` trait, `MockTransport`, and
`RouterTransportAdapter(pub(crate))`. The adapter wraps `Arc<RouterTransport>` and
implements `Transport::send()`. A `TODO(P8-E5)` comment on the adapter's doc comment
explicitly states the wiring gap. The watchdog's `run()` method sends Pings at a
configurable interval and waits for matching Pongs through an `mpsc::Receiver`; if no
Pong arrives within the timeout (or the channel closes, or the send fails), it signals
death via a `oneshot::Sender`.

`ManagedWorker` currently has two `select!` branches in `run()`: `shutdown_rx` and
`transport.recv()`. The `handle_event()` method handles `WorkerEvent::Pong` with a no-op
arm that only logs at DEBUG level. The comment on the arm says "the keepalive watchdog
monitors these separately via the demux channel" — but no such wiring exists yet.

**Established patterns:** Error handling uses `tracing` with structured fields
(`worker_id = %self.worker_id`). Crash paths append to `attempt_history`, call
`should_respawn()`, and log at INFO. Tests use in-process ZeroMQ ROUTER/DEALER pairs
with `send_event()` (DEALER → ROUTER direction) and `send_malformed()` for crash
simulation. All tests use bounded waits (5s timeout via `tokio::select!`). The
`#[tracing::instrument]` attribute is used on `run()`.

**Gap between design and source:** The design doc (§9.2) states "Keepalive pings every
30 seconds; no pong within 10 seconds → declared dead." The keepalive watchdog implements
this, but it is never constructed or spawned. The `handle_event()` Pong arm's comment
references a demux-channel mechanism that doesn't exist — the actual mechanism is a
dedicated `mpsc` channel created per `ManagedWorker` and fed by forwarding Pongs from
`handle_event()`.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | tokio   | 1.52.3          | rust-docs MCP  | sync (includes mpsc, oneshot) |
| crate  | anvilml-ipc | (path dep)  | N/A            | N/A                      |

No new external dependencies are introduced. All types used (`tokio::sync::mpsc`,
`tokio::sync::oneshot`, `tokio::time::Duration`) are available via the existing
`sync` feature flag declared in `Cargo.toml`.

## Approach

### Step 1 — Remove `#[allow(dead_code)]` from `RouterTransportAdapter`

In `keepalive.rs`, remove the `#[allow(dead_code)]` attribute from
`RouterTransportAdapter`. This struct is `pub(crate)` and will be constructed by
`ManagedWorker` in this task, making it no longer dead code.

### Step 2 — Add watchdog fields to `ManagedWorker`

Add two fields to the `ManagedWorker` struct:

```rust
/// Oneshot receiver for the watchdog's death signal.
///
/// When the watchdog detects a missing Pong (pong_timeout elapsed without a
/// matching response), it sends on `dead_tx` and this receiver becomes ready.
/// The `run()` loop polls this in a `select!` branch alongside shutdown and
/// transport recv — when ready, the worker is declared Dead.
watchdog_dead_rx: tokio::sync::oneshot::Receiver<()>,

/// Channel sender for forwarding Pong events to the watchdog.
///
/// Each `WorkerEvent::Pong` from `handle_event()` is sent here. The watchdog
/// filters for matching sequence numbers internally. A closed or full channel
/// is not an error — the watchdog will eventually timeout and declare the
/// worker dead, which is the correct failure mode.
pong_tx: tokio::sync::mpsc::Sender<anvilml_ipc::WorkerEvent>,
```

### Step 3 — Update `ManagedWorker::new()` constructor

Add a `pong_tx: mpsc::Sender<WorkerEvent>` parameter. Create the oneshot pair
inside `new()` (the sender is passed to the watchdog which is spawned in `run()`;
the receiver is stored in the struct):

```rust
pub fn new(
    worker_id: String,
    transport: Arc<RouterTransport>,
    demux: Arc<Demux>,
    status: Arc<RwLock<WorkerStatus>>,
    respawn_policy: RespawnPolicy,
    init_timeout: Duration,
    pong_tx: mpsc::Sender<WorkerEvent>,
) -> Self {
    let (watchdog_dead_tx, watchdog_dead_rx) = oneshot::channel();
    // Store watchdog_dead_tx in a field so it can be dropped when the
    // ManagedWorker is consumed — this closes the channel and stops the
    // watchdog when the worker exits.
    Self {
        worker_id,
        transport,
        demux,
        status,
        respawn_policy,
        attempt_history: Vec::new(),
        init_timeout,
        pong_tx,
        watchdog_dead_rx,
        // Note: watchdog_dead_tx is NOT stored — it is moved into the
        // watchdog when spawned in run(). This is intentional: dropping
        // the sender (when self is consumed) closes the channel, causing
        // the watchdog to exit its loop.
    }
}
```

**Rationale:** The oneshot sender is created in `new()` but not stored — it is
moved into the watchdog task when spawned in `run()`. This means when `run()`
consumes `self`, the sender is dropped, which closes the watchdog's `dead_rx`
receiver side from the sender's perspective... wait, actually the sender is
owned by the watchdog task (spawned via `tokio::spawn`), so when `run()` returns,
the watchdog task is still running with its own copy of the sender. The receiver
is stored in `self` and will be dropped when `self` is consumed. Dropping the
receiver doesn't affect the sender — the sender just stops being polled. This
is fine: the watchdog's `run()` loop will naturally exit when the pong channel
is closed (which happens when the test drops `pong_tx` after `run()` returns).

Actually, let me reconsider. The `watchdog_dead_tx` needs to be stored somewhere
so it can be explicitly dropped when the worker exits (to stop the watchdog
gracefully). But `run()` consumes `self`, so the watchdog is spawned inside
`run()`. The cleanest approach: store `watchdog_dead_tx` as `Option<oneshot::Sender<()>>`
in `ManagedWorker`, spawn the watchdog in `run()`, and on every exit path,
`take()` the sender and drop it. This closes the watchdog's `dead_tx` from the
watchdog's perspective... no, dropping the sender doesn't signal death — it
just stops the watchdog from being able to send. The watchdog won't notice.

The correct approach: the watchdog's `run()` loop exits when `pong_rx` is closed
(all senders dropped). The `pong_tx` is stored in `ManagedWorker` and is cloned
for each call to `handle_event()`. When `run()` consumes `self`, the `pong_tx`
is dropped, and all clones are also dropped. The watchdog's `pong_rx.recv()` then
returns `None`, and the watchdog exits.

The `watchdog_dead_tx` is owned by the watchdog task. When the watchdog exits
naturally (pong channel closed), it doesn't send on `dead_tx`. The `dead_rx`
in `run()` simply stays pending and is never ready. This is correct behavior:
the watchdog's exit is graceful, and the `dead_rx` branch only fires when the
watchdog *detects* a problem.

So the design is:
- `watchdog_dead_tx` is created in `new()`, moved into the watchdog task in `run()`.
- `watchdog_dead_rx` is stored in `ManagedWorker`.
- When `run()` exits, `self` is consumed, `watchdog_dead_rx` is dropped.
- The `pong_tx` field is also consumed, closing the channel.
- The watchdog's `pong_rx.recv()` returns `None`, and the watchdog exits.

This means `watchdog_dead_tx` is not stored — it's moved into the spawned task.
The task owns it for the duration of its lifetime. When the task exits, the sender
is dropped automatically.

### Step 4 — Add third `select!` branch in `run()`

Add a third branch to the existing `tokio::select!` in `run()`:

```rust
// Watchdog dead path — the keepalive watchdog detected a missing Pong.
// This is a second crash source, independent of transport.recv().
// Handled identically to the transport-error branch.
_ = &mut self.watchdog_dead_rx => {
    tracing::error!(worker_id = %self.worker_id, "watchdog timeout — worker declared dead");
    *self.status.write().await = WorkerStatus::Dead;
    self.attempt_history.push(Instant::now());
    let should = self.respawn_policy.should_respawn(&self.attempt_history);
    tracing::info!(worker_id = %self.worker_id, should_respawn = should, "crash_respawn_decision");
    break;
}
```

**Rationale:** The `dead_rx` is a oneshot receiver, so `&mut self.watchdog_dead_rx`
works the same way as `&mut shutdown_rx` in the existing code. When the watchdog
detects a missing Pong, it sends on `dead_tx`, making this branch ready.

### Step 5 — Forward Pongs in `handle_event()`

In the `WorkerEvent::Pong` arm of `handle_event()`, forward the event to the
watchdog's pong channel:

```rust
WorkerEvent::Pong { seq } => {
    // Forward the Pong to the keepalive watchdog's pong channel.
    // The watchdog filters for matching sequence numbers internally.
    // A failed send (closed/full channel) is best-effort — the watchdog
    // will timeout and declare the worker dead if it misses a Pong,
    // which is the correct failure mode.
    let _ = self.pong_tx.try_send(event);
    tracing::debug!(worker_id = %self.worker_id, seq = %seq, "pong_received");
    false
}
```

**Rationale:** `try_send` is used instead of `send().await` because:
1. `handle_event()` is called from within the `select!` loop's `transport.recv()`
   branch, and we don't want to block the event loop on a channel send.
2. A failed send (channel full or closed) means the watchdog will timeout and
   declare the worker dead on the next Ping cycle — this is the correct failure
   mode, not a spurious error.
3. Using `try_send` keeps the event processing non-blocking and consistent with
   the established pattern in the codebase (see `WorkerHandle::request_shutdown()`
   which ignores the result of `tx.send(())`).

### Step 6 — Update existing tests

Every existing test that calls `ManagedWorker::new()` must pass a `pong_tx` argument.
Create an `mpsc::channel::<WorkerEvent>(16)` in each test and pass the sender. The
receiver is unused in most tests (the watchdog is spawned but its death signal is
never expected to fire in normal-path tests). The test should await the watchdog's
join handle after `run()` completes to ensure clean shutdown.

### Step 7 — Add new tests (≥4)

See the `## Tests` section below for the full test catalogue.

### Step 8 — Bump crate version

Bump `anvilml-worker` from `0.1.14` to `0.1.15` in `Cargo.toml`.

## Public API Surface

| Item | Location | Description |
|------|----------|-------------|
| `ManagedWorker::new()` — added `pong_tx: mpsc::Sender<WorkerEvent>` param | `managed.rs` | Constructor signature change |
| `RouterTransportAdapter` — `#[allow(dead_code)]` removed | `keepalive.rs` | No longer dead code |

No new `pub` items are introduced. The only API surface change is the additional
constructor parameter on `ManagedWorker::new()`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/keepalive.rs` | Remove `#[allow(dead_code)]` from `RouterTransportAdapter` |
| Modify | `crates/anvilml-worker/src/managed.rs` | Add watchdog fields, update constructor, add select! branch, forward Pongs |
| Modify | `crates/anvilml-worker/tests/managed_tests.rs` | Update existing tests for new constructor param; add ≥4 new tests |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.14 → 0.1.15 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `managed_tests.rs` | `test_watchdog_missing_pong_triggers_crash_path` | With short ping_interval (50ms) and pong_timeout (200ms), withholding all Pongs causes the watchdog to declare the worker dead via `dead_rx`, which triggers the same crash path as a transport error (status → Dead, attempt_history appended, should_respawn called, loop breaks). | `cargo test -p anvilml-worker --test managed_tests -- test_watchdog_missing_pong_triggers_crash_path` exits 0 |
| `managed_tests.rs` | `test_watchdog_live_pongs_no_false_trigger` | With short intervals (50ms/200ms), sending Pongs at the correct sequence number keeps the watchdog alive for the duration of the test. The worker processes Ready events normally, status transitions to Idle, and `dead_rx` never fires. Verifies live pongs don't false-trigger the crash path. | `cargo test -p anvilml-worker --test managed_tests -- test_watchdog_live_pongs_no_false_trigger` exits 0 |
| `managed_tests.rs` | `test_pong_forwarding_does_not_disturb_idle_busy` | Pong forwarding to the watchdog channel does not interfere with normal event processing. Sends Ready (→ Idle), manually sets Busy, sends Completed (→ Idle), sends Failed (→ Idle). The watchdog receives Pongs on its channel but they are filtered by sequence number. Status transitions are correct. | `cargo test -p anvilml-worker --test managed_tests -- test_pong_forwarding_does_not_disturb_idle_busy` exits 0 |
| `managed_tests.rs` | `test_router_transport_adapter_constructible` | Constructs `RouterTransportAdapter(Arc::clone(&transport))` directly, proving the type is no longer `#[allow(dead_code)]` and is usable. A compile-time test: if the attribute is still present, this test wouldn't compile. | `cargo test -p anvilml-worker --test managed_tests -- test_router_transport_adapter_constructible` exits 0 |
| `managed_tests.rs` | `test_watchdog_channel_cleans_up_on_exit` | After `run()` completes, the `pong_tx` is dropped (consumed by `self`), closing the watchdog's `pong_rx`. The watchdog exits its loop without sending on `dead_tx` (graceful exit). The test verifies the join handle completes within 500ms. | `cargo test -p anvilml-worker --test managed_tests -- test_watchdog_channel_cleans_up_on_exit` exits 0 |

## CI Impact

No CI changes required. The test module (`managed_tests.rs`) already exists and is
picked up by `cargo test --workspace --features mock-hardware`. The new tests are
in the same file, so no new test targets or CI jobs are needed.

## Platform Considerations

None identified. The changes are platform-neutral — no `#[cfg(unix)]` or `#[cfg(windows)]`
guards are required. The `tokio::sync::mpsc` and `tokio::sync::oneshot` channels are
cross-platform. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `handle_event()` uses `try_send` which silently drops Pongs when the channel is full. If the channel capacity is too small, the watchdog may miss Pongs and false-trigger a crash. | Medium | High | Use channel capacity of 16 (matching the existing doctest in `keepalive.rs`). The `try_send` is best-effort; if it fails, the watchdog times out on the next Ping cycle (at most 30s in production, 200ms in tests). This is the correct failure mode — the worker is actually dead if it can't keep up with Pongs. |
| The `watchdog_dead_rx` oneshot receiver in the `select!` loop may fire spuriously if the channel is closed by the watchdog task exiting naturally (graceful exit via `pong_rx` closure). When the watchdog exits gracefully, it does NOT send on `dead_tx`, so the receiver should NOT become ready. | Low | Medium | Verified by reading `tokio::sync::oneshot` semantics: a oneshot receiver only becomes ready when the sender sends a value. Dropping the sender (when the task exits) does not make the receiver ready. The test `test_watchdog_channel_cleans_up_on_exit` verifies this. |
| Existing tests that construct `ManagedWorker` will fail to compile due to the new `pong_tx` parameter. | High | Low | All existing tests are in the same file (`managed_tests.rs`) and are updated in this task. The fix is mechanical: add `let (pong_tx, _pong_rx) = mpsc::channel(16);` before each `ManagedWorker::new()` call. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --test managed_tests` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no dead_code warning on `RouterTransportAdapter`)
- [ ] `grep -c "dead_code" crates/anvilml-worker/src/keepalive.rs` returns 0 (no `allow(dead_code)` on `RouterTransportAdapter`)
- [ ] `cargo test -p anvilml-worker --test managed_tests -- test_watchdog_missing_pong_triggers_crash_path` exits 0
- [ ] `cargo test -p anvilml-worker --test managed_tests -- test_watchdog_live_pongs_no_false_trigger` exits 0
- [ ] `cargo test -p anvilml-worker --test managed_tests -- test_pong_forwarding_does_not_disturb_idle_busy` exits 0
- [ ] `cargo test -p anvilml-worker --test managed_tests -- test_router_transport_adapter_constructible` exits 0
- [ ] `cargo test -p anvilml-worker --test managed_tests -- test_watchdog_channel_cleans_up_on_exit` exits 0
