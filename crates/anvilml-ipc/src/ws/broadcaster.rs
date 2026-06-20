//! EventBroadcaster — a thin wrapper around a Tokio broadcast channel.
//!
//! Provides `new()`, `send()`, and `subscribe()` methods for broadcasting
//! `WsEvent` messages to connected WebSocket clients. The channel has a
//! fixed capacity of 1024; when the buffer is full, the oldest subscriber
//! that has not yet consumed the message is dropped and a warning is logged.
//!
//! Defined in `anvilml-ipc` (not `anvilml-server`) to avoid a cyclic
//! dependency: `anvilml-worker` needs this type to construct `WorkerPool`,
//! but `anvilml-server` transitively depends on `anvilml-worker`.
//! Placing it in `anvilml-ipc` (which `anvilml-worker` already depends on)
//! breaks the cycle.

use anvilml_core::types::WsEvent;
use tokio::sync::broadcast;

/// A broadcast channel wrapper for `WsEvent` and `WorkerEvent` messages.
///
/// Holds two independent broadcast channels: one for `WsEvent` messages
/// (sent to WebSocket clients) and one for `WorkerEvent` messages
/// (internal IPC events from workers, consumed by the scheduler's
/// event loop). This separation keeps external WebSocket traffic
/// isolated from internal IPC event routing.
#[derive(Debug)]
pub struct EventBroadcaster {
    /// The WsEvent broadcast sender; cloned internally by `subscribe()`.
    tx: broadcast::Sender<WsEvent>,

    /// The WorkerEvent broadcast sender for internal IPC events.
    ///
    /// The scheduler subscribes to this channel to receive Completed/Failed
    /// events from workers, enabling it to update job status and release
    /// VRAM reservations.
    worker_event_tx: broadcast::Sender<crate::WorkerEvent>,
}

impl EventBroadcaster {
    /// Create a new `EventBroadcaster` with channel capacity of 1024.
    ///
    /// Creates two independent broadcast channels:
    /// 1. `WsEvent` channel for WebSocket client notifications (capacity 1024).
    /// 2. `WorkerEvent` channel for internal IPC events from workers
    ///    (capacity 1024). The scheduler subscribes to this channel.
    ///
    /// The channel capacity of 1024 is sufficient for the periodic
    /// system stats tick (one event every ~5 seconds) plus burst job events.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        // Worker event channel uses the same capacity — worker events
        // (Completed/Failed) are infrequent and the event loop consumes
        // them immediately in the same task, so backpressure is minimal.
        let (worker_event_tx, _worker_event_rx) = broadcast::channel(1024);
        Self {
            tx,
            worker_event_tx,
        }
    }

    /// Send a `WsEvent` to all current subscribers.
    ///
    /// If the broadcast buffer is full (all receivers are lagging behind),
    /// the event is dropped and a warning is logged. The caller should not
    /// retry — the event is transient and will be superseded by the next
    /// state-change event.
    ///
    /// # Arguments
    ///
    /// * `event` — The WebSocket event to broadcast. Passed by value because
    ///   the tokio broadcast `Sender::send` method consumes the value.
    pub fn send(&self, event: WsEvent) {
        // The broadcast channel drops the oldest unread message when full.
        // This is intentional — stale events (e.g. an old job progress update)
        // are less useful than keeping the channel responsive.
        // Clone before sending because `send()` consumes the value and we need
        // it for logging in the error path. The clone cost is acceptable since
        // WsEvent is small and cloning only happens on the error path.
        if self.tx.send(event.clone()).is_err() {
            // No receivers are subscribed; the message was dropped.
            // This is normal during startup and when no client is connected
            // to /v1/events — logged at DEBUG to avoid polluting WARN output
            // with routine channel-empty conditions.
            tracing::debug!(event_type = ?event, "broadcast receiver lagged, message dropped");
        }
    }

    /// Subscribe to the broadcast channel, returning a new `Receiver`.
    ///
    /// Each call to `subscribe()` produces an independent receiver that
    /// will receive all events sent after the subscription point. If the
    /// sender has buffered events at the time of subscription, the receiver
    /// will first drain those buffered events before receiving new ones.
    ///
    /// # Returns
    ///
    /// A `broadcast::Receiver<WsEvent>` that can be awaited in a loop to
    /// receive events. Callers must call `.recv().await` in a loop to
    /// consume all available events.
    pub fn subscribe(&self) -> broadcast::Receiver<WsEvent> {
        self.tx.subscribe()
    }

    /// Subscribe to the worker event channel, returning a new `Receiver`.
    ///
    /// The scheduler uses this to receive `WorkerEvent` messages (Completed,
    /// Failed, etc.) from workers. Events sent before this subscription point
    /// are not delivered — the event loop subscribes immediately after the
    /// scheduler is constructed, and no Completed/Failed events can occur
    /// before the scheduler is ready.
    ///
    /// # Returns
    ///
    /// A `broadcast::Receiver<WorkerEvent>` for consuming worker events.
    pub fn subscribe_worker_events(&self) -> broadcast::Receiver<crate::WorkerEvent> {
        self.worker_event_tx.subscribe()
    }

    /// Send a `WorkerEvent` to all subscribers (scheduler event loop).
    ///
    /// This is called by the worker pool when a worker emits a Completed,
    /// Failed, or other lifecycle event. The event is forwarded to the
    /// scheduler's event loop for status updates and VRAM release.
    ///
    /// If the broadcast buffer is full (no subscribers or all lagging),
    /// the event is dropped silently — the event loop will catch up on
    /// the next iteration.
    ///
    /// # Arguments
    ///
    /// * `event` — The worker event to broadcast.
    pub fn broadcast_worker_event(&self, event: crate::WorkerEvent) {
        // If no one is subscribed (event loop not started yet), silently
        // drop the event. This is acceptable because no Completed/Failed
        // events can occur before the event loop is started.
        if self.worker_event_tx.send(event).is_err() {
            tracing::debug!("worker event broadcast: no subscribers");
        }
    }
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}
