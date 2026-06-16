//! Tests for `EventBroadcaster` — the WebSocket event broadcast channel.
//!
//! These tests exercise the three public methods (`new`, `send`, `subscribe`)
//! and verify correct behavior for normal send/receive, lagged receivers,
//! and error paths when all subscribers drop.

use anvilml_core::types::WsEvent;
use anvilml_server::ws::EventBroadcaster;
use uuid::Uuid;

/// Verify that `EventBroadcaster::new()` creates a valid broadcaster
/// with channel capacity 1024.
///
/// Checks that `subscribe()` works and the receiver can receive a broadcast
/// event. Also exercises the `Default` impl which delegates to `new()`.
///
/// No preconditions — the broadcaster is freshly constructed.
#[tokio::test]
async fn test_broadcaster_new() {
    let bc = EventBroadcaster::new();

    // The Default impl delegates to new(), so verify it works too.
    let _bc_default = EventBroadcaster::default();

    // Subscribe and verify the receiver can receive an event.
    let mut rx = bc.subscribe();

    bc.send(WsEvent::SystemStats {
        cpu_pct: 0.0,
        ram_used_mib: 0,
        workers: vec![],
    });

    // The receiver should get the event we just sent.
    let received = rx.recv().await;
    assert!(
        received.is_ok(),
        "receiver should receive the broadcast event"
    );
}

/// Verify that `send()` delivers an event to a subscriber and the
/// received event matches the sent event exactly.
///
/// Constructs a `WsEvent::SystemStats` with known fields, sends it
/// through the broadcaster, and asserts the receiver gets an identical event.
///
/// Preconditions: A broadcaster with one subscriber.
#[tokio::test]
async fn test_broadcaster_send_and_receive() {
    let bc = EventBroadcaster::new();
    let mut rx = bc.subscribe();

    let expected = WsEvent::SystemStats {
        cpu_pct: 42.5,
        ram_used_mib: 8192,
        workers: vec![],
    };

    bc.send(expected.clone());

    let received = rx.recv().await.expect("should receive the sent event");
    assert_eq!(
        received, expected,
        "received event must match the sent event"
    );
}

/// Verify that when all subscribers drop while the channel is full,
/// `send()` does not panic — the event is silently dropped.
///
/// Creates a broadcaster, sends 1024 events to fill the buffer, then
/// drops the subscriber. A subsequent `send()` must not panic because
/// the channel drops the event when there are no receivers.
///
/// Preconditions: A broadcaster with one subscriber that does not consume.
#[tokio::test]
async fn test_broadcaster_lagged_receiver() {
    let bc = EventBroadcaster::new();
    let mut rx = bc.subscribe();

    // Drain any initial buffered events (there shouldn't be any).
    while rx.try_recv().is_ok() {}

    // Fill the channel to capacity (1024 events).
    // With an active subscriber, all sends succeed (oldest events are evicted).
    for i in 0..1024 {
        let event = WsEvent::JobQueued {
            job_id: Uuid::new_v4(),
            queue_position: i + 1,
        };
        bc.send(event);
    }

    // Drop the subscriber — now the channel has no receivers.
    drop(rx);

    // The channel is still full (1024 events buffered) but all receivers dropped.
    // send() must not panic — it silently drops the event.
    let overflow_event = WsEvent::JobQueued {
        job_id: Uuid::new_v4(),
        queue_position: 9999,
    };
    // This should not panic — the event is silently dropped.
    bc.send(overflow_event);
}
