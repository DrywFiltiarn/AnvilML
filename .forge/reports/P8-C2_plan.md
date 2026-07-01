# Plan Report: P8-C2

| Field | Value |
|-------|-------|
| Task ID | P8-C2 |
| Phase | 008 — IPC Stress Gate & Worker Pool |
| Description | anvilml-worker: keepalive.rs ping/pong heartbeat watchdog |
| Depends on | P8-C1 |
| Project | anvilml |
| Planned at | 2026-07-01T08:00:00Z |
| Attempt | 1 |

## Objective

Create `crates/anvilml-worker/src/keepalive.rs` — a `KeepaliveWatchdog` type that runs a tokio task sending `WorkerMessage::Ping{seq}` at a configurable interval and tracking incoming `WorkerEvent::Pong{seq}` responses. If no matching Pong arrives within the configured timeout after a Ping, the watchdog signals worker death via a `tokio::sync::oneshot::Sender<()>` channel. The interval and timeout are injected as constructor parameters (not hardcoded `Duration::from_secs` literals) so tests can use millisecond-scale durations and run fast. Declare `mod keepalive; pub use keepalive::KeepaliveWatchdog;` in `lib.rs`.

## Scope

### In Scope
- `crates/anvilml-worker/src/keepalive.rs` — `KeepaliveWatchdog` struct with constructor, `run()` async method, and the internal ping loop
- `crates/anvilml-worker/src/lib.rs` — add `mod keepalive;` and `pub use keepalive::KeepaliveWatchdog;`
- `crates/anvilml-worker/Cargo.toml` — add `"time"` feature to the tokio dependency (required for `tokio::time::interval`)
- `crates/anvilml-worker/tests/keepalive_tests.rs` — ≥4 tests using injected millisecond durations

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope with no deferrals. The task context mentions "confirm/verify at ACT time" phrases, which mean resolve-then-implement, not skip-and-stub.

## Existing Codebase Assessment

The `anvilml-worker` crate already contains five source modules (`demux.rs`, `env.rs`, `spawn.rs`, `job_object.rs`) and three test files (`demux_tests.rs`, `env_tests.rs`, `spawn_tests.rs`). The established patterns are:

- **Error handling**: All errors use `anvilml_core::AnvilError` (re-exported from the crate root). The demux tests show the pattern of matching `Err(AnvilError::WorkerNotFound(id))` for variant-specific assertions.
- **Testing style**: Tests are integration test crate files in `tests/` using `#[tokio::test]` for async tests. They import from the crate root (`use anvilml_worker::Demux;`). Test helpers are minimal — each test constructs its own fixtures inline. The `WorkerEvent::Ready` struct is used as a common fixture.
- **Module structure**: `lib.rs` contains only `//!` crate doc, `pub mod` declarations, and `pub use` re-exports. No implementation code in `lib.rs`.
- **Dependency pattern**: `anvilml-ipc` re-exports `WorkerMessage`, `WorkerEvent`, `RouterTransport`, and `IpcError` at the crate root. `anvilml-core` re-exports `AnvilError` at the crate root. Both are used directly by worker modules.
- **No prior source gap**: The design doc (§9.3) lists `keepalive.rs` as a planned module; it simply hasn't been created yet. No discrepancy between design doc and current source.

## Resolved Dependencies

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| crate | tokio | 1.52.3 | rust-docs MCP | time (new), process, sync (existing) |

No new external crates are introduced. The only change is adding the `time` feature to the existing tokio dependency, which is confirmed to exist in tokio 1.52.3 via MCP lookup.

## Approach

### Step 1 — Add `time` feature to Cargo.toml

Open `crates/anvilml-worker/Cargo.toml`. The existing tokio line is:

```toml
tokio = { version = "1.52.3", features = ["process", "sync"] }
```

Change to:

```toml
tokio = { version = "1.52.3", features = ["process", "sync", "time"] }
```

The `time` feature is confirmed to exist in tokio 1.52.3 (verified via rust-docs MCP). It provides `tokio::time::interval` and `tokio::time::sleep`, which are required for the keepalive loop.

### Step 2 — Create `crates/anvilml-worker/src/keepalive.rs`

Create the file with the following structure:

```rust
//! Keepalive watchdog: sends periodic Ping messages and detects dead workers.
//!
//! Runs a tokio task that sends `WorkerMessage::Ping{seq}` at a configurable
//! interval and waits for matching `WorkerEvent::Pong{seq}` responses. If no
//! Pong arrives within the configured timeout after a Ping, the watchdog signals
//! worker death via a oneshot channel.
//!
//! The interval and timeout are injected as constructor parameters so that tests
//! can use millisecond-scale durations without waiting real seconds.
//!
//! See `ANVILML_DESIGN.md §9.2` — keepalive pings every 30s; no pong within 10s → dead.

use anvilml_core::AnvilError;
use anvilml_ipc::{RouterTransport, WorkerEvent, WorkerMessage};
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::{interval, sleep, Duration, MissedTickBehavior};

/// A keepalive watchdog that sends periodic Ping messages and detects dead workers.
///
/// Runs an async loop that sends `WorkerMessage::Ping{seq}` at `ping_interval`
/// cadence and waits up to `pong_timeout` for a matching `WorkerEvent::Pong{seq}`.
/// If no Pong arrives within the timeout, the watchdog signals worker death by
/// sending on the `dead_tx` oneshot channel.
///
/// The watchdog is constructed with dependency-injected durations so tests can
/// use millisecond-scale values. Default production values are 30s interval
/// and 10s timeout per `ANVILML_DESIGN.md §9.2`.
///
/// # Example
///
/// ```ignore
/// let (dead_tx, dead_rx) = oneshot::channel();
/// let watchdog = KeepaliveWatchdog::new(
///     "worker-0".into(),
///     transport,
///     dead_tx,
///     Duration::from_secs(30),
///     Duration::from_secs(10),
/// );
/// tokio::spawn(watchdog.run());
/// ```
pub struct KeepaliveWatchdog {
    /// Stable worker identity (e.g. `"0"`). Used as the ROUTER socket address.
    worker_id: String,

    /// Shared reference to the ROUTER transport for sending Ping messages.
    transport: Arc<RouterTransport>,

    /// Oneshot sender used to signal that the watchdog has detected the worker
    /// as dead (no Pong within the timeout). The receiver side is awaited by
    /// the caller to learn about the death signal.
    dead_tx: oneshot::Sender<()>,

    /// Time between consecutive Ping messages.
    /// Default: 30 seconds per `ANVILML_DESIGN.md §9.2`.
    ping_interval: Duration,

    /// Maximum time to wait for a matching Pong after sending a Ping.
    /// Default: 10 seconds per `ANVILML_DESIGN.md §9.2`.
    pong_timeout: Duration,

    /// Monotonically increasing sequence number, incremented on each Ping send.
    seq: u64,
}

impl KeepaliveWatchdog {
    /// Create a new `KeepaliveWatchdog`.
    ///
    /// # Arguments
    /// * `worker_id` — Stable worker identity (e.g. `"0"`).
    /// * `transport` — Shared reference to the ROUTER transport.
    /// * `dead_tx` — Oneshot sender for signaling worker death.
    /// * `ping_interval` — Time between consecutive Ping messages.
    /// * `pong_timeout` — Maximum wait for a matching Pong after a Ping.
    pub fn new(
        worker_id: String,
        transport: Arc<RouterTransport>,
        dead_tx: oneshot::Sender<()>,
        ping_interval: Duration,
        pong_timeout: Duration,
    ) -> Self {
        Self {
            worker_id,
            transport,
            dead_tx,
            ping_interval,
            pong_timeout,
            seq: 0,
        }
    }

    /// Run the keepalive loop.
    ///
    /// Sends a Ping every `ping_interval` and waits for a matching Pong
    /// within `pong_timeout`. If the timeout expires without a Pong, the
    /// watchdog signals worker death via `dead_tx` and the loop terminates.
    ///
    /// This method is designed to be spawned as a tokio task:
    /// `tokio::spawn(watchdog.run())`.
    pub async fn run(mut self) {
        // Use MissedTickBehavior::Delay to ensure pings fire at least as
        // frequently as the interval, even if a previous iteration takes
        // longer than the interval. This prevents pings from accumulating
        // and flooding the worker.
        let mut ticker = interval(self.ping_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;

            // Increment sequence number before sending.
            let seq = self.seq;
            self.seq += 1;

            tracing::debug!(worker_id = %self.worker_id, seq, "sending ping");

            // Send the Ping message via the ROUTER transport.
            if let Err(e) = self.transport.send(&self.worker_id, &WorkerMessage::Ping { seq }).await {
                tracing::error!(worker_id = %self.worker_id, seq, error = %e, "failed to send ping");
                // Send a death signal — the transport failure means the worker
                // is unreachable. The loop then terminates.
                let _ = self.dead_tx.send(());
                return;
            }

            // Wait for a Pong within the timeout. We use select! to race
            // the timeout against receiving events from the transport.
            // The reader task (bridge.rs) delivers incoming events through
            // the demux; we need to check if a Pong arrived.
            //
            // Strategy: poll the transport's recv half with a timeout.
            // If we get a Pong, continue the loop. If timeout fires first,
            // signal death and exit.
            let pong_received = self.await_pong_or_timeout().await;

            if pong_received {
                tracing::debug!(worker_id = %self.worker_id, seq, "received pong");
            } else {
                // No Pong received within the timeout — the worker is dead.
                tracing::info!(worker_id = %self.worker_id, seq, "no pong received — worker declared dead");
                let _ = self.dead_tx.send(());
                return;
            }
        }
    }

    /// Await a Pong event from the transport, racing against the pong timeout.
    ///
    /// Receives messages from the ROUTER socket and checks if the first
    /// matching event for this worker is a Pong. If it is, returns `true`.
    /// If the timeout fires first, returns `false`.
    ///
    /// Note: This method receives directly from the transport's recv half.
    /// In production, the bridge reader task would normally consume events
    /// and route them via the demux. For the keepalive watchdog, we need
    /// to intercept Pong events specifically. However, since the bridge
    /// task is the sole consumer of `transport.recv()`, this creates a
    /// conflict.
    ///
    /// **Resolution**: The watchdog does NOT call `transport.recv()` directly.
    /// Instead, it uses a separate channel-based approach: the bridge task
    /// routes all events through the demux, and the watchdog subscribes to
    /// a per-worker channel. But since keepalive is built before the bridge,
    /// the simplest correct approach is: the watchdog sends a Ping, then
    /// waits on a separate `tokio::sync::mpsc::Receiver<WorkerEvent>` that
    /// the bridge task feeds Pongs into.
    ///
    /// **Revised approach**: The `KeepaliveWatchdog` constructor takes an
    /// `mpsc::Receiver<WorkerEvent>` (a channel fed by the bridge reader
    /// task). The `run()` loop sends a Ping, then awaits on the receiver
    /// with a timeout. If a Pong with matching seq arrives before the
    /// timeout, it continues. Otherwise, it signals death.
    ///
    /// Since this task is built in isolation (before bridge.rs), the
    /// constructor accepts an `mpsc::Receiver<WorkerEvent>` that the
    /// caller (bridge.rs, built later) will wire up.
    async fn await_pong_or_timeout(&self) -> bool {
        // This method signature is a placeholder — the actual implementation
        // in Step 2b below uses a receiver channel.
        unimplemented!("see Step 2b for actual implementation")
    }
}
```

Wait — I need to reconsider the architecture. The keepalive watchdog needs to receive Pong events. In the final architecture (bridge.rs), the bridge reader task calls `transport.recv()` and routes via `demux.route()`. The watchdog cannot call `transport.recv()` directly because the bridge task is the sole consumer.

The correct approach: the watchdog receives Pongs through a channel. The simplest design is to have the watchdog take a `tokio::sync::mpsc::Receiver<WorkerEvent>` in its constructor, which the bridge task (or a Pong-specific filter) will feed. However, since this task is built before bridge.rs, the receiver will be wired up later.

Let me refine the approach:

### Step 2 (revised) — Create `crates/anvilml-worker/src/keepalive.rs`

The `KeepaliveWatchdog` takes:
- `worker_id: String`
- `pong_rx: tokio::sync::mpsc::Receiver<WorkerEvent>` — a channel that the bridge reader task will feed Pong events into (or the caller can wire it up for testing)
- `dead_tx: oneshot::Sender<()>`
- `ping_interval: Duration`
- `pong_timeout: Duration`

The `run()` method:
1. Creates a `tokio::time::interval` with `ping_interval`
2. On each tick:
   a. Increment `seq`, send `WorkerMessage::Ping { seq }` via `transport`
   b. Await `pong_rx.recv()` with `tokio::time::timeout(pong_timeout, ...)`
   c. If the recv returns a Pong with matching `seq`, continue
   d. If the timeout fires, signal death via `dead_tx` and exit

```rust
//! Keepalive watchdog: sends periodic Ping messages and detects dead workers.
//!
//! Runs a tokio task that sends `WorkerMessage::Ping{seq}` at a configurable
//! interval and waits for matching `WorkerEvent::Pong{seq}` responses received
//! through a dedicated channel. If no Pong arrives within the configured timeout
//! after a Ping, the watchdog signals worker death via a oneshot channel.
//!
//! The interval and timeout are injected as constructor parameters so that tests
//! can use millisecond-scale durations without waiting real seconds.
//!
//! See `ANVILML_DESIGN.md §9.2` — keepalive pings every 30s; no pong within 10s → dead.

use anvilml_ipc::{RouterTransport, WorkerEvent, WorkerMessage};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, Duration, MissedTickBehavior};

/// A keepalive watchdog that sends periodic Ping messages and detects dead workers.
///
/// Runs an async loop that sends `WorkerMessage::Ping{seq}` at `ping_interval`
/// cadence and waits up to `pong_timeout` for a matching `WorkerEvent::Pong{seq}`
/// received through `pong_rx`. If no Pong arrives within the timeout, the watchdog
/// signals worker death by sending on the `dead_tx` oneshot channel and the loop
/// terminates.
///
/// The watchdog is constructed with dependency-injected durations so tests can
/// use millisecond-scale values. Default production values are 30s interval
/// and 10s timeout per `ANVILML_DESIGN.md §9.2`.
///
/// The `pong_rx` receiver is fed by the IPC bridge reader task, which routes
/// incoming `WorkerEvent::Pong` messages to this channel. This design avoids
/// the bridge task having to know about keepalive semantics — the bridge simply
/// passes all events through, and the watchdog filters for Pongs.
pub struct KeepaliveWatchdog {
    /// Stable worker identity (e.g. `"0"`). Used as the ROUTER socket address.
    worker_id: String,

    /// Shared reference to the ROUTER transport for sending Ping messages.
    transport: Arc<RouterTransport>,

    /// Channel for receiving events from the bridge reader task.
    /// The watchdog filters for `WorkerEvent::Pong` variants matching the
    /// current sequence number.
    pong_rx: mpsc::Receiver<WorkerEvent>,

    /// Oneshot sender used to signal that the watchdog has detected the worker
    /// as dead (no Pong within the timeout).
    dead_tx: oneshot::Sender<()>,

    /// Time between consecutive Ping messages.
    ping_interval: Duration,

    /// Maximum time to wait for a matching Pong after sending a Ping.
    pong_timeout: Duration,

    /// Monotonically increasing sequence number, incremented on each Ping send.
    seq: u64,
}

impl KeepaliveWatchdog {
    /// Create a new `KeepaliveWatchdog`.
    ///
    /// # Arguments
    /// * `worker_id` — Stable worker identity (e.g. `"0"`).
    /// * `transport` — Shared reference to the ROUTER transport.
    /// * `pong_rx` — Channel for receiving events from the bridge reader task.
    /// * `dead_tx` — Oneshot sender for signaling worker death.
    /// * `ping_interval` — Time between consecutive Ping messages.
    /// * `pong_timeout` — Maximum wait for a matching Pong after a Ping.
    pub fn new(
        worker_id: String,
        transport: Arc<RouterTransport>,
        pong_rx: mpsc::Receiver<WorkerEvent>,
        dead_tx: oneshot::Sender<()>,
        ping_interval: Duration,
        pong_timeout: Duration,
    ) -> Self {
        Self {
            worker_id,
            transport,
            pong_rx,
            dead_tx,
            ping_interval,
            pong_timeout,
            seq: 0,
        }
    }

    /// Run the keepalive loop.
    ///
    /// Sends a Ping every `ping_interval` and waits for a matching Pong
    /// within `pong_timeout`. If the timeout expires without a Pong, the
    /// watchdog signals worker death via `dead_tx` and the loop terminates.
    ///
    /// This method is designed to be spawned as a tokio task:
    /// `tokio::spawn(watchdog.run())`.
    pub async fn run(mut self) {
        // Use MissedTickBehavior::Delay to ensure pings fire at least as
        // frequently as the interval, even if a previous iteration takes
        // longer than the interval. This prevents pings from accumulating
        // and flooding the worker.
        let mut ticker = interval(self.ping_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;

            // Increment sequence number before sending.
            let seq = self.seq;
            self.seq += 1;

            tracing::debug!(worker_id = %self.worker_id, seq, "sending ping");

            // Send the Ping message via the ROUTER transport.
            if let Err(e) = self.transport.send(&self.worker_id, &WorkerMessage::Ping { seq }).await {
                // Transport send failed — the worker is unreachable.
                // Signal death and exit the loop.
                tracing::error!(worker_id = %self.worker_id, seq, error = %e, "failed to send ping");
                let _ = self.dead_tx.send(());
                return;
            }

            // Wait for a Pong with matching seq within the timeout.
            // Use tokio::time::timeout to bound the wait.
            let pong_received = tokio::time::timeout(
                self.pong_timeout,
                self.wait_for_matching_pong(seq),
            )
            .await;

            match pong_received {
                Ok(true) => {
                    // Pong received within timeout — continue the loop.
                    tracing::debug!(worker_id = %self.worker_id, seq, "received pong");
                }
                Ok(false) => {
                    // Pong arrived but with wrong seq (shouldn't happen
                    // in normal operation, but guard against it).
                    // Continue the loop to send the next Ping.
                    tracing::debug!(worker_id = %self.worker_id, expected_seq = seq, "pong with wrong seq, continuing");
                }
                Err(_) => {
                    // Timeout expired — no Pong received within the timeout.
                    // The worker is declared dead.
                    tracing::info!(worker_id = %self.worker_id, seq, "no pong received within timeout — worker declared dead");
                    let _ = self.dead_tx.send(());
                    return;
                }
            }
        }
    }

    /// Await a Pong event from the channel matching the given sequence number.
    ///
    /// Reads events from `pong_rx` until either:
    /// - A `WorkerEvent::Pong { seq: expected_seq }` is received (returns `true`)
    /// - The channel is closed (returns `false` — worker is gone)
    /// - A Pong with a different seq is received (skips it, continues waiting)
    ///
    /// This method does NOT have its own timeout — the timeout is applied by
    /// the caller via `tokio::time::timeout`. This separation allows the
    /// timeout to cover the entire recv operation including any spurious
    /// events that may arrive before the matching Pong.
    async fn wait_for_matching_pong(&mut self, expected_seq: u64) -> bool {
        loop {
            // Receive the next event from the channel.
            // If the channel is closed, the worker is gone.
            let event = match self.pong_rx.recv().await {
                Some(event) => event,
                None => return false,
            };

            // Check if this event is a Pong with the expected sequence number.
            if let WorkerEvent::Pong { seq } = &event {
                if *seq == expected_seq {
                    return true;
                }
                // Wrong seq — skip and continue waiting.
                // This can happen if a previous Ping's Pong arrives late.
                tracing::debug!(
                    worker_id = %self.worker_id,
                    expected_seq,
                    received_seq = seq,
                    "skipping pong with wrong seq"
                );
            }
            // Non-Pong events are also skipped — the watchdog only cares
            // about Pong responses to its own Ping.
        }
    }
}
```

**Key design decisions:**

1. **Pong reception via channel, not direct transport recv**: The bridge reader task (built later) is the sole consumer of `transport.recv()`. The watchdog cannot call `recv()` directly. Instead, the bridge task will feed all events into the watchdog's `pong_rx` channel. The watchdog filters for Pongs with matching seq.

2. **`MissedTickBehavior::Delay`**: Prevents ping accumulation if a previous iteration takes longer than the interval. Each missed tick becomes one delayed send rather than multiple queued sends.

3. **Spurious Pong handling**: A Pong with a wrong `seq` is skipped rather than triggering death. This handles the edge case where a previous Ping's Pong arrives after the timeout has already started for the next Ping.

4. **Channel close = death**: If `pong_rx` is closed (bridge task died), `wait_for_matching_pong` returns `false`, which is treated as "no pong received" and triggers the death signal.

5. **Logging**: `tracing::debug!` for routine ping/pong activity; `tracing::info!` for the death declaration; `tracing::error!` for transport send failures. No mandatory INFO log points from `ANVILML_DESIGN.md §16.2` apply to the keepalive module (those cover worker spawn, job dispatch, model scan, etc.).

### Step 3 — Update `crates/anvilml-worker/src/lib.rs`

Add the keepalive module declaration and re-export:

```rust
mod keepalive;
pub use keepalive::KeepaliveWatchdog;
```

Add these lines after the existing `mod job_object;` / `pub use job_object::JobObjectGuard;` block (before the closing of the file).

### Step 4 — Create `crates/anvilml-worker/tests/keepalive_tests.rs`

Create ≥4 tests using injected millisecond durations:

**Test 1 — `test_pong_within_timeout_keeps_alive`**: Construct a watchdog with 50ms ping interval and 100ms pong timeout. Feed a matching Pong through `pong_rx` before the timeout. Verify `dead_rx` is not ready (no death signal sent).

**Test 2 — `test_missing_pong_triggers_dead_signal`**: Construct a watchdog with 50ms ping interval and 100ms pong timeout. Do NOT feed any Pong. Wait for `dead_rx` to receive. Verify the death signal is sent.

**Test 3 — `test_repeated_successful_pings_no_false_trigger`**: Construct a watchdog with 50ms ping interval and 100ms pong timeout. Feed matching Pongs for 3 consecutive pings. Wait for the loop to complete (or timeout the test at 500ms). Verify `dead_rx` is not ready.

**Test 4 — `test_transport_send_failure_triggers_dead_signal`**: Construct a watchdog with a mock transport that fails on send. Send a Ping and verify the transport send fails, triggering the death signal.

(See the detailed Tests section below for full test specifications.)

### Step 5 — Verify compilation

Run `cargo check -p anvilml-worker --features mock-hardware` to confirm the module compiles before writing tests.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| struct | `anvilml_worker::KeepaliveWatchdog` | `pub struct KeepaliveWatchdog { ... }` |
| fn | `KeepaliveWatchdog::new` | `pub fn new(worker_id: String, transport: Arc<RouterTransport>, pong_rx: mpsc::Receiver<WorkerEvent>, dead_tx: oneshot::Sender<()>, ping_interval: Duration, pong_timeout: Duration) -> Self` |
| fn | `KeepaliveWatchdog::run` | `pub async fn run(mut self)` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/keepalive.rs` | Keepalive watchdog implementation |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Add `mod keepalive;` and `pub use keepalive::KeepaliveWatchdog;` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Add `"time"` feature to tokio; bump patch version 0.1.3 → 0.1.4 |
| CREATE | `crates/anvilml-worker/tests/keepalive_tests.rs` | ≥4 tests for the keepalive watchdog |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `tests/keepalive_tests.rs` | `test_pong_within_timeout_keeps_alive` | A Pong received within the configured timeout does NOT trigger the death signal. Uses 50ms interval, 100ms timeout, feeds matching Pong via channel. | `cargo test -p anvilml-worker --test keepalive_tests test_pong_within_timeout_keeps_alive` exits 0 |
| `tests/keepalive_tests.rs` | `test_missing_pong_triggers_dead_signal` | No Pong arriving within the timeout triggers the death signal. Uses 50ms interval, 100ms timeout, no Pong sent. Awaits `dead_rx` to receive. | `cargo test -p anvilml-worker --test keepalive_tests test_missing_pong_triggers_dead_signal` exits 0 |
| `tests/keepalive_tests.rs` | `test_repeated_successful_pings_no_false_trigger` | Repeated successful Pongs do not false-trigger the death signal. Uses 50ms interval, 100ms timeout, feeds 3 matching Pongs. Verifies `dead_rx` is not ready after 500ms. | `cargo test -p anvilml-worker --test keepalive_tests test_repeated_successful_pings_no_false_trigger` exits 0 |
| `tests/keepalive_tests.rs` | `test_transport_send_failure_triggers_dead_signal` | A transport send failure (worker unreachable) triggers the death signal. Uses a mock transport that returns an error on `send()`. Verifies `dead_rx` receives. | `cargo test -p anvilml-worker --test keepalive_tests test_transport_send_failure_triggers_dead_signal` exits 0 |

## CI Impact

No CI changes required. The new test file `tests/keepalive_tests.rs` is picked up automatically by `cargo test --workspace --features mock-hardware` (step 6 of ENVIRONMENT.md §6), which runs all test crates in the workspace. No new CI jobs, gates, or file patterns are introduced.

## Platform Considerations

None identified. The keepalive module uses only `tokio::time::interval`, `tokio::time::timeout`, and standard library types — all platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `pong_rx` channel design creates a coupling with bridge.rs: the bridge reader task must feed events into the watchdog's channel, but the bridge task is built in a later task (P8-F1). If the bridge task doesn't wire this channel correctly, the watchdog will never receive Pongs and will always declare workers dead. | Medium | High | The `KeepaliveWatchdog::new` constructor takes a plain `mpsc::Receiver<WorkerEvent>` — the bridge task can be wired up later. The tests exercise the watchdog in isolation using a locally-created channel with manually-fed events. When P8-F1 builds the bridge, the wiring is a one-line addition: create a channel, pass the receiver to `KeepaliveWatchdog::new`, and forward all received events through the channel. |
| `tokio::time::interval` with `MissedTickBehavior::Delay` may not fire exactly every `ping_interval` if the tokio runtime is busy. The actual ping cadence could drift. | Low | Low | This is inherent to tokio's cooperative scheduling. The design doc specifies "every 30 seconds" as a cadence, not a hard real-time guarantee. The watchdog's correctness (detecting dead workers) does not depend on exact timing — it depends on the pong timeout being shorter than the worker's actual failure detection window. |
| Spurious Pongs with wrong `seq` could accumulate and fill the channel buffer, causing `recv()` to block and the timeout to fire prematurely. | Low | Medium | The channel capacity is set by the caller (typically 16, matching the demux pattern). The watchdog consumes one event per Ping. As long as the Pong rate doesn't exceed the Ping rate, the buffer won't fill. The timeout covers the entire recv loop, so even with spurious events, a matching Pong will eventually arrive or the timeout will fire. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-worker --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-worker --test keepalive_tests` exits 0 (all ≥4 tests pass)
- [ ] `cargo test -p anvilml-worker --test keepalive_tests test_pong_within_timeout_keeps_alive` exits 0
- [ ] `cargo test -p anvilml-worker --test keepalive_tests test_missing_pong_triggers_dead_signal` exits 0
- [ ] `cargo test -p anvilml-worker --test keepalive_tests test_repeated_successful_pings_no_false_trigger` exits 0
- [ ] `cargo test -p anvilml-worker --test keepalive_tests test_transport_send_failure_triggers_dead_signal` exits 0
- [ ] `wc -l crates/anvilml-worker/src/lib.rs` reports ≤80 lines
