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
use tokio::time::{Duration, MissedTickBehavior, interval};

/// Transport abstraction for sending Ping messages to workers.
///
/// This trait is implemented by `Arc<RouterTransport>` for production use
/// and by `MockTransport` in tests. It exists to decouple the watchdog from
/// the concrete ZeroMQ transport type, enabling testability without requiring
/// a live socket for every test.
pub trait Transport: Send + Sync {
    /// Send a `WorkerMessage` to a worker identified by `worker_id`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the send operation fails (e.g. the worker is
    /// unreachable or the underlying socket is closed).
    fn send(
        &self,
        worker_id: &str,
        msg: &WorkerMessage,
    ) -> impl std::future::Future<Output = Result<(), anvilml_ipc::IpcError>> + Send;
}

/// A mock transport that either succeeds or fails on every send.
///
/// Used in tests to exercise the watchdog's transport-failure path
/// without requiring a live ZeroMQ socket.
#[derive(Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct MockTransport {
    /// If `Some`, all `send()` calls return this error.
    /// If `None`, all `send()` calls succeed.
    fail_with: Option<anvilml_ipc::IpcError>,
}

impl MockTransport {
    /// Create a new `MockTransport` that always succeeds.
    #[allow(dead_code)]
    pub fn new_ok() -> Self {
        Self { fail_with: None }
    }

    /// Create a new `MockTransport` that always fails with the given error.
    #[allow(dead_code)]
    pub fn new_err(err: anvilml_ipc::IpcError) -> Self {
        Self {
            fail_with: Some(err),
        }
    }
}

impl Transport for MockTransport {
    async fn send(
        &self,
        _worker_id: &str,
        _msg: &WorkerMessage,
    ) -> Result<(), anvilml_ipc::IpcError> {
        if let Some(ref err) = self.fail_with {
            Err(err.clone())
        } else {
            Ok(())
        }
    }
}

/// An adapter that implements `Transport` for `Arc<RouterTransport>`.
///
/// Bridges the concrete `RouterTransport::send()` method to the trait API.
///
/// TODO(P8-E3): construct this inside `ManagedWorker::run()` — wrap the worker's
/// `Arc<RouterTransport>` in `RouterTransportAdapter`, pass it to
/// `KeepaliveWatchdog::new(...)`, and spawn `watchdog.run()` in run()'s lifecycle.
/// Remove the `#[allow(dead_code)]` below once that wiring exists — this struct is
/// unused because no supervisor/ManagedWorker module exists yet to construct it.
#[allow(dead_code)]
struct RouterTransportAdapter(Arc<RouterTransport>);

impl Transport for RouterTransportAdapter {
    async fn send(
        &self,
        worker_id: &str,
        msg: &WorkerMessage,
    ) -> Result<(), anvilml_ipc::IpcError> {
        self.0.send(worker_id, msg).await
    }
}

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
///
/// # Example
///
/// ```ignore
/// let (dead_tx, dead_rx) = oneshot::channel();
/// let (pong_tx, pong_rx) = mpsc::channel(16);
/// let watchdog = KeepaliveWatchdog::new(
///     "worker-0".into(),
///     transport,
///     pong_rx,
///     dead_tx,
///     Duration::from_secs(30),
///     Duration::from_secs(10),
/// );
/// tokio::spawn(watchdog.run());
/// ```
pub struct KeepaliveWatchdog<T: Transport> {
    /// Stable worker identity (e.g. `"0"`). Used as the ROUTER socket address.
    worker_id: String,

    /// Transport abstraction for sending Ping messages.
    transport: T,

    /// Channel for receiving events from the bridge reader task.
    /// The watchdog filters for `WorkerEvent::Pong` variants matching the
    /// current sequence number.
    pong_rx: mpsc::Receiver<WorkerEvent>,

    /// Oneshot sender used to signal that the watchdog has detected the worker
    /// as dead (no Pong within the timeout).
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

impl<T: Transport> KeepaliveWatchdog<T> {
    /// Create a new `KeepaliveWatchdog` with the given transport.
    ///
    /// # Arguments
    /// * `worker_id` — Stable worker identity (e.g. `"0"`).
    /// * `transport` — Transport implementing the `Transport` trait.
    ///   For production use, wrap `Arc<RouterTransport>` in `RouterTransportAdapter`.
    /// * `pong_rx` — Channel for receiving events from the bridge reader task.
    /// * `dead_tx` — Oneshot sender for signaling worker death.
    /// * `ping_interval` — Time between consecutive Ping messages.
    /// * `pong_timeout` — Maximum wait for a matching Pong after a Ping.
    pub fn new(
        worker_id: String,
        transport: T,
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

            // Send the Ping message via the transport.
            if let Err(e) = self
                .transport
                .send(&self.worker_id, &WorkerMessage::Ping { seq })
                .await
            {
                // Transport send failed — the worker is unreachable.
                // Signal death and exit the loop.
                tracing::error!(
                    worker_id = %self.worker_id,
                    seq,
                    error = %e,
                    "failed to send ping"
                );
                let _ = self.dead_tx.send(());
                return;
            }

            // Wait for a Pong with matching seq within the timeout.
            // Use tokio::time::timeout to bound the wait.
            let pong_received =
                tokio::time::timeout(self.pong_timeout, self.wait_for_matching_pong(seq)).await;

            match pong_received {
                Ok(true) => {
                    // Pong received within timeout — continue the loop.
                    tracing::debug!(worker_id = %self.worker_id, seq, "received pong");
                }
                Ok(false) => {
                    // Pong arrived but with wrong seq (shouldn't happen
                    // in normal operation, but guard against it).
                    // Continue the loop to send the next Ping.
                    tracing::debug!(
                        worker_id = %self.worker_id,
                        expected_seq = seq,
                        "pong with wrong seq, continuing"
                    );
                }
                Err(_) => {
                    // Timeout expired — no Pong received within the timeout.
                    // The worker is declared dead.
                    tracing::info!(
                        worker_id = %self.worker_id,
                        seq,
                        "no pong received within timeout — worker declared dead"
                    );
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
        tracing::debug!(worker_id = %self.worker_id, expected_seq, "entering wait_for_matching_pong");
        loop {
            // Receive the next event from the channel.
            // If the channel is closed, the worker is gone.
            let event = match self.pong_rx.recv().await {
                Some(event) => {
                    tracing::debug!(
                        worker_id = %self.worker_id,
                        event_type = ?event,
                        "received event from pong_rx"
                    );
                    event
                }
                None => {
                    tracing::debug!(worker_id = %self.worker_id, "pong_rx channel closed");
                    return false;
                }
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
