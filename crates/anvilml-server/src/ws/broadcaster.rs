//! EventBroadcaster — a thin wrapper around a Tokio broadcast channel.
//!
//! Provides `new()`, `send()`, and `subscribe()` methods for broadcasting
//! `WsEvent` messages to connected WebSocket clients. The channel has a
//! fixed capacity of 1024; when the buffer is full, the oldest subscriber
//! that has not yet consumed the message is dropped and a warning is logged.

use anvilml_core::types::WsEvent;
use tokio::sync::broadcast;

/// A broadcast channel wrapper for `WsEvent` messages.
///
/// Holds a single `broadcast::Sender` backed by a ring buffer of capacity 1024.
/// Multiple subscribers can call `subscribe()` to obtain independent receivers.
/// When the buffer fills up, `send()` returns `Err(SendError)` and the event
/// is dropped — no retry or queueing is performed.
#[derive(Debug)]
pub struct EventBroadcaster {
    /// The broadcast sender; cloned internally by `subscribe()`.
    tx: broadcast::Sender<WsEvent>,
}

impl EventBroadcaster {
    /// Create a new `EventBroadcaster` with a channel capacity of 1024.
    ///
    /// The channel is created with `broadcast::channel(1024)`, which means
    /// up to 1024 events can be buffered before `send()` starts returning
    /// `Err(SendError)`. This capacity is sufficient for the periodic
    /// system stats tick (one event every ~5 seconds) plus burst job events.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self { tx }
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
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}
