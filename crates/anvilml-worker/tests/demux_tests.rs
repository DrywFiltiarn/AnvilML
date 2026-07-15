//! Integration tests for `Demux` — verifies register, deregister, and route
//! operations on the worker-event demultiplexing map.
//!
//! All tests use `tokio::sync::mpsc::channel()` to create sender/receiver pairs
//! for testing the route delivery path. No env vars are mutated, so no `#[serial]`
//! is needed.

use anvilml_core::AnvilError;
use anvilml_ipc::WorkerEvent;
use anvilml_worker::Demux;

/// Register a worker, route an event, verify the receiver gets it.
///
/// Creates a fresh channel, registers the sender with the demux, then routes
/// a `Ready` event through it. Verifies the receiver can `recv()` the exact
/// same event that was sent.
#[tokio::test]
async fn test_register_and_route_delivers() {
    let demux = Demux::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);

    demux.register("worker-0".to_string(), tx);

    let event = WorkerEvent::Ready {
        worker_id: "worker-0".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 8192,
        vram_free_mib: 8192,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };

    demux
        .route("worker-0", event.clone())
        .await
        .expect("route should succeed");

    let received = rx.recv().await.expect("receiver should get the event");
    assert_eq!(received, event);
}

/// Route to an unregistered worker returns `AnvilError::WorkerNotFound`.
///
/// Calls `route()` without first calling `register()`. Verifies the error
/// variant matches `AnvilError::WorkerNotFound` with the correct worker ID.
#[tokio::test]
async fn test_route_worker_not_found() {
    let demux = Demux::new();

    let event = WorkerEvent::Ready {
        worker_id: "worker-99".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 8192,
        vram_free_mib: 8192,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };

    let result = demux.route("worker-99", event).await;
    match result {
        Err(AnvilError::WorkerNotFound(id)) => {
            assert_eq!(id, "worker-99");
        }
        other => panic!("expected WorkerNotFound, got {:?}", other),
    }
}

/// Register a worker, route successfully, deregister, then route again —
/// verify `AnvilError::WorkerNotFound`.
///
/// This is the mandatory deregistration test per `ANVILML_DESIGN.md §9.4`.
/// It proves that `deregister()` actually removes the entry from the routing
/// table and that `route()` correctly fails for deregistered workers.
#[tokio::test]
async fn test_deregister_removes_entry() {
    let demux = Demux::new();
    let (tx, _rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);

    demux.register("worker-0".to_string(), tx);

    // Route should succeed while registered.
    let event = WorkerEvent::Ready {
        worker_id: "worker-0".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 8192,
        vram_free_mib: 8192,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    demux
        .route("worker-0", event.clone())
        .await
        .expect("route should succeed before deregister");

    // Deregister the worker.
    let removed = demux.deregister("worker-0");
    assert!(
        removed,
        "deregister should return true for an existing entry"
    );

    // Route should now fail with WorkerNotFound.
    let result = demux.route("worker-0", event).await;
    match result {
        Err(AnvilError::WorkerNotFound(id)) => {
            assert_eq!(id, "worker-0");
        }
        other => panic!("expected WorkerNotFound after deregister, got {:?}", other),
    }
}

/// Deregister an existing entry, then deregister the same ID again —
/// verify the second call returns `false` and does not panic.
///
/// Tests that double-deregister is safe (idempotent) and returns `false`
/// on the second call, confirming the entry was actually removed on the
/// first call.
#[tokio::test]
async fn test_double_deregister_is_safe() {
    let demux = Demux::new();
    let (tx, _rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);

    demux.register("worker-0".to_string(), tx);

    // First deregister: should return true.
    let first = demux.deregister("worker-0");
    assert!(first, "first deregister should return true");

    // Second deregister on the same ID: should return false, no panic.
    let second = demux.deregister("worker-0");
    assert!(!second, "second deregister should return false");
}

/// Register a worker with sender A, then register the same worker ID with
/// sender B. Route an event and verify it arrives on B's receiver (not A's).
///
/// Tests the idempotent overwrite behavior: when a worker respawns and
/// re-registers, the new sender replaces the old one. Events delivered
/// after re-registration go to the new channel, not the stale one.
#[tokio::test]
async fn test_register_overwrites() {
    let demux = Demux::new();
    let (tx_a, mut rx_a) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    let (tx_b, mut rx_b) = tokio::sync::mpsc::channel::<WorkerEvent>(16);

    // Register with sender A first.
    demux.register("worker-0".to_string(), tx_a);

    // Register again with sender B — this overwrites A's sender.
    demux.register("worker-0".to_string(), tx_b);

    // Route an event. It should go to B's channel, not A's.
    let event = WorkerEvent::Ready {
        worker_id: "worker-0".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 8192,
        vram_free_mib: 8192,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    demux
        .route("worker-0", event.clone())
        .await
        .expect("route should succeed");

    // A's receiver should be empty (nothing was sent to it after overwrite).
    assert!(
        rx_a.try_recv().is_err(),
        "event should NOT be delivered to the old sender (A)"
    );

    // B's receiver should have the event.
    let received = rx_b.recv().await.expect("receiver B should get the event");
    assert_eq!(received, event);
}

/// `subscribe()` delivers a `(worker_id, event)` clone of every routed event
/// to the subscriber, tagged with the worker it came from.
///
/// This is the mandatory fan-out test per `ANVILML_DESIGN.md §9.8`
/// (`docs/ADDENDUM_DEMUX_FANOUT.md`) — a subscriber must observe events for
/// *any* worker_id, not just one it separately registered for.
#[tokio::test]
async fn test_subscribe_receives_fanned_out_event() {
    let demux = Demux::new();
    let (_id, mut sub_rx) = demux.subscribe();

    let event = WorkerEvent::Pong { seq: 7 };

    // Routing to an unregistered primary worker still fans out to the
    // subscriber — the two delivery paths are independent (§9.8).
    let route_result = demux.route("worker-0", event.clone()).await;
    assert!(
        matches!(route_result, Err(AnvilError::WorkerNotFound(_))),
        "primary delivery still fails with WorkerNotFound as before fan-out existed"
    );

    let (got_worker_id, got_event) = sub_rx
        .recv()
        .await
        .expect("subscriber should receive the fanned-out event");
    assert_eq!(got_worker_id, "worker-0");
    assert_eq!(got_event, event);
}

/// Multiple subscribers each independently receive their own clone of the
/// same routed event — one subscriber's receive does not consume it for
/// another, and does not interfere with primary delivery.
#[tokio::test]
async fn test_multiple_subscribers_each_receive_independently() {
    let demux = Demux::new();
    let (_id_a, mut sub_rx_a) = demux.subscribe();
    let (_id_b, mut sub_rx_b) = demux.subscribe();

    let (primary_tx, mut primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("worker-0".to_string(), primary_tx);

    let event = WorkerEvent::Cancelled {
        job_id: uuid::Uuid::new_v4(),
    };

    demux
        .route("worker-0", event.clone())
        .await
        .expect("route should succeed — worker-0 is registered");

    // The primary consumer still receives it, unaffected by fan-out.
    let primary_received = primary_rx
        .recv()
        .await
        .expect("primary consumer should still receive the event");
    assert_eq!(primary_received, event);

    // Both subscribers independently receive their own copy.
    let (a_worker_id, a_event) = sub_rx_a
        .recv()
        .await
        .expect("subscriber A should receive the event");
    let (b_worker_id, b_event) = sub_rx_b
        .recv()
        .await
        .expect("subscriber B should receive the event");
    assert_eq!(a_worker_id, "worker-0");
    assert_eq!(b_worker_id, "worker-0");
    assert_eq!(a_event, event);
    assert_eq!(b_event, event);
}

/// `unsubscribe()` stops further fan-out delivery to that subscription.
///
/// Subscribes, routes one event (received), unsubscribes, routes a second
/// event, and verifies the second event never arrives — the channel is
/// closed from the subscriber's perspective (`recv()` returns `None`) rather
/// than silently hanging.
#[tokio::test]
async fn test_unsubscribe_stops_fanout_delivery() {
    let demux = Demux::new();
    let (id, mut sub_rx) = demux.subscribe();

    let first = WorkerEvent::Pong { seq: 1 };
    let _ = demux.route("worker-0", first.clone()).await;
    let (_wid, got) = sub_rx.recv().await.expect("first event should arrive");
    assert_eq!(got, first);

    demux.unsubscribe(id);

    let second = WorkerEvent::Pong { seq: 2 };
    let _ = demux.route("worker-0", second).await;

    // The channel's sender was dropped by unsubscribe(), so recv() resolves
    // to None rather than yielding the second event or hanging forever.
    let result = sub_rx.recv().await;
    assert!(
        result.is_none(),
        "recv() should return None after unsubscribe(), got {:?}",
        result
    );
}

/// `unsubscribe()` with an id that was never issued (or already removed) is
/// a safe no-op — mirrors `deregister()`'s existing idempotency convention.
#[tokio::test]
async fn test_unsubscribe_unknown_id_is_safe() {
    let demux = Demux::new();
    // No panic, no error return value to check — unsubscribe() is `()`.
    demux.unsubscribe(9999);

    // A real subscription is unaffected by an unrelated unsubscribe() call.
    let (_id, mut sub_rx) = demux.subscribe();
    let event = WorkerEvent::Pong { seq: 42 };
    let _ = demux.route("worker-0", event.clone()).await;
    let (_wid, got) = sub_rx.recv().await.expect("subscription should still work");
    assert_eq!(got, event);
}

/// Fan-out to a full subscriber channel drops that one event for that one
/// subscriber (logged at WARN) without returning an error from `route()`
/// and without blocking delivery to any other consumer.
///
/// Fills the subscriber's channel to capacity by never draining it, then
/// routes one more event than the channel can hold. `route()` must still
/// return `Ok(())` (primary worker is registered) and the primary consumer
/// must still receive every event — a stalled subscriber must never be able
/// to stall the worker's own primary delivery.
#[tokio::test]
async fn test_full_subscriber_channel_does_not_block_route() {
    let demux = Demux::new();
    let (_id, _sub_rx_never_drained) = demux.subscribe();

    let (primary_tx, mut primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(4096);
    demux.register("worker-0".to_string(), primary_tx);

    // DEMUX_SUBSCRIBER_CAPACITY (256) + a few extra sends to guarantee the
    // subscriber's channel fills and try_send() starts failing, while every
    // one of these route() calls must still succeed and reach the primary
    // consumer.
    for i in 0..300u64 {
        let event = WorkerEvent::Pong { seq: i };
        demux
            .route("worker-0", event)
            .await
            .expect("route() must succeed regardless of subscriber channel state");
    }

    // Drain the primary consumer's channel and confirm it received all 300 —
    // fan-out backpressure on the (undrained) subscriber never affected it.
    let mut count = 0;
    while let Ok(_event) = primary_rx.try_recv() {
        count += 1;
    }
    assert_eq!(
        count, 300,
        "primary consumer must receive every routed event even though the \
         subscriber's channel filled up and started dropping events"
    );
}
