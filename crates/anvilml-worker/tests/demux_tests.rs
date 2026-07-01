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
        fp16: true,
        bf16: true,
        fp8: false,
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
        fp16: true,
        bf16: true,
        fp8: false,
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
        fp16: true,
        bf16: true,
        fp8: false,
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
        fp16: true,
        bf16: true,
        fp8: false,
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
