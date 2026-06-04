//! Thin wrapper around `tokio::sync::broadcast::Sender<Arc<WsEvent>>` for
//! broadcasting WebSocket events to connected clients.

use std::sync::Arc;

use anvilml_core::types::events::WsEvent;
use tokio::sync::broadcast;

/// Broadcasts [`WsEvent`] instances to all subscribed receivers.
///
/// Internally uses a `tokio::sync::broadcast` channel. The `send` method
/// silently ignores send failures (e.g. when no subscribers are present).
pub struct EventBroadcaster {
    sender: broadcast::Sender<Arc<WsEvent>>,
}

impl EventBroadcaster {
    /// Create a new broadcaster with the given channel capacity.
    ///
    /// The capacity determines how many events can be buffered before older
    /// messages are dropped for slow consumers.
    pub fn new(capacity: usize) -> Self {
        let (sender, _receiver) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Send an event to all current subscribers.
    ///
    /// The event is wrapped in an `Arc` before sending. If there are no
    /// subscribers the error is silently ignored — this is by design so
    /// callers never need to check subscriber count.
    pub fn send(&self, event: WsEvent) {
        let _ = self.sender.send(Arc::<WsEvent>::from(Box::new(event)));
    }

    /// Subscribe to receive events broadcast by this broadcaster.
    ///
    /// The receiver will only see events sent **after** the subscription is
    /// created; it does not receive historical messages.
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<WsEvent>> {
        self.sender.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Subscribe → send a `WsEvent::SystemStats` → receive on subscriber;
    /// the received event must equal the sent event.
    #[test]
    fn subscribe_send_receive() {
        let broadcaster = EventBroadcaster::new(16);
        let mut rx = broadcaster.subscribe();

        let expected = WsEvent::SystemStats(anvilml_core::types::events::SystemStatsEvent {
            event: "system.stats".to_string(),
            timestamp: chrono::Utc::now(),
            gpus: vec![],
            ram_used_mib: 0,
            ram_total_mib: 0,
        });

        broadcaster.send(expected.clone());

        let received = rx.try_recv().expect("should receive the event");
        let expected_json = serde_json::to_string(&expected).unwrap();
        let received_json = serde_json::to_string(received.as_ref()).unwrap();
        assert_eq!(received_json, expected_json);
    }

    /// Send with zero subscribers must not panic or propagate an error.
    #[test]
    fn send_no_subscribers_no_error() {
        let broadcaster = EventBroadcaster::new(16);

        // This should not panic — SendError is silently ignored by design.
        broadcaster.send(WsEvent::SystemStats(
            anvilml_core::types::events::SystemStatsEvent {
                event: "system.stats".to_string(),
                timestamp: chrono::Utc::now(),
                gpus: vec![],
                ram_used_mib: 0,
                ram_total_mib: 0,
            },
        ));
    }
}
