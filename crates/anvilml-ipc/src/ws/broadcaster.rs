use anvilml_core::WsEvent;
use tokio::sync::broadcast::Sender;

/// A WebSocket event broadcaster wrapping `tokio::sync::broadcast::Sender<WsEvent>`.
///
/// Buffer capacity is 1024 events per ANVILML_DESIGN.md §13.6.
/// `publish()` ignores `SendError` — publishing with zero subscribers
/// is expected, normal behaviour and not an error condition.
#[derive(Debug)]
pub struct EventBroadcaster(Sender<WsEvent>);

impl EventBroadcaster {
    /// Create a new `EventBroadcaster` with a 1024-event broadcast buffer.
    ///
    /// The 1024 capacity is specified in ANVILML_DESIGN.md §13.6.
    pub fn new() -> Self {
        Self(Sender::new(1024))
    }

    /// Publish an event to all current subscribers.
    ///
    /// If there are zero subscribers, `send()` returns `Err(SendError)` which
    /// is ignored — this is expected and not an error condition.
    pub fn publish(&self, event: WsEvent) {
        // Ignore SendError: zero subscribers is a normal state, not an error.
        let _ = self.0.send(event);
    }

    /// Subscribe to receive future events.
    ///
    /// Returns a new `Receiver<WsEvent>` that can be awaited independently.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<WsEvent> {
        self.0.subscribe()
    }
}

impl Default for EventBroadcaster {
    // Clippy requires Default when a `new()` method exists without it.
    fn default() -> Self {
        Self::new()
    }
}
