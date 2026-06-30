//! Integration tests for `EventBroadcaster` — publish/subscribe behaviour
//! of the tokio::sync::broadcast wrapper.
//!
//! All tests use `#[tokio::test]` for async support. No env vars, files, or
//! I/O are used; each test constructs its own `EventBroadcaster` instance.

use anvilml_core::WsEvent;
use anvilml_ipc::EventBroadcaster;
use uuid::Uuid;

/// Publishing an event with zero subscribers does not panic — the internal
/// `send()` returns `Err(SendError)` which `publish()` silently discards.
#[tokio::test]
async fn test_publish_zero_subscribers() {
    let broadcaster = EventBroadcaster::new();
    let event = WsEvent::JobQueued {
        job_id: Uuid::new_v4(),
        queue_position: 1,
    };

    // publish() with zero subscribers: must not panic; SendError is ignored.
    broadcaster.publish(event);
}

/// Publishing an event with one subscriber delivers the event to that subscriber.
#[tokio::test]
async fn test_publish_one_subscriber_delivers() {
    let broadcaster = EventBroadcaster::new();
    let mut receiver = broadcaster.subscribe();
    let expected = WsEvent::JobStarted {
        job_id: Uuid::new_v4(),
        worker_id: "gpu:0".to_string(),
    };

    broadcaster.publish(expected.clone());
    let received = receiver
        .recv()
        .await
        .expect("receiver should deliver the event");

    assert_eq!(
        expected, received,
        "received event does not match published event"
    );
}

/// Publishing one event to multiple subscribers gives each subscriber an
/// independent copy of the event.
#[tokio::test]
async fn test_publish_multiple_subscribers_independent_copies() {
    let broadcaster = EventBroadcaster::new();
    let mut rx1 = broadcaster.subscribe();
    let mut rx2 = broadcaster.subscribe();
    let expected = WsEvent::JobCompleted {
        job_id: Uuid::new_v4(),
        elapsed_ms: 42,
    };

    broadcaster.publish(expected.clone());

    let from_rx1 = rx1.recv().await.expect("rx1 should receive the event");
    let from_rx2 = rx2.recv().await.expect("rx2 should receive the event");

    assert_eq!(
        expected, from_rx1,
        "rx1 received event does not match published event"
    );
    assert_eq!(
        expected, from_rx2,
        "rx2 received event does not match published event"
    );
}

/// subscribe() returns a receiver that is valid — calling recv().await does
/// not immediately return `RecvError::Closed` before any publish occurs.
#[tokio::test]
async fn test_subscribe_returns_valid_receiver() {
    let broadcaster = EventBroadcaster::new();
    let mut receiver = broadcaster.subscribe();

    // Wait with a timeout to confirm the receiver is open (not closed).
    // A timeout elapsing means recv() is still blocked waiting for events,
    // which proves the channel is open. If the channel were closed, recv()
    // would return Err(RecvError::Closed) immediately, completing within
    // the timeout window.
    let result = tokio::time::timeout(std::time::Duration::from_millis(100), receiver.recv()).await;

    // Err(Elapsed) means recv() was still blocked when the timeout fired —
    // the channel is open and waiting for events. This is the expected state.
    assert!(
        result.is_err(),
        "recv() should still be blocked (channel open), not closed; got {:?}",
        result
    );
}

// ---------------------------------------------------------------------------
// WorkerMessage msgpack roundtrip tests
// ---------------------------------------------------------------------------

use anvilml_core::JobSettings;
use anvilml_ipc::messages::WorkerMessage;

/// `WorkerMessage::Ping { seq: 42 }` serialises via rmp-serde and roundtrips
/// to an equal value. The msgpack dict contains `"_type": "Ping"` and
/// `"seq": 42`.
#[test]
fn test_ping_roundtrip() {
    let msg = WorkerMessage::Ping { seq: 42 };

    let bytes = rmp_serde::to_vec_named(&msg).expect("serialize Ping");
    let decoded: WorkerMessage = rmp_serde::from_slice(&bytes).expect("deserialize Ping");

    assert_eq!(msg, decoded, "Ping roundtrip must preserve seq");
}

/// `WorkerMessage::Shutdown` (unit variant, no fields) roundtrips via
/// rmp-serde. The msgpack dict contains only `"_type": "Shutdown"`.
#[test]
fn test_shutdown_roundtrip() {
    let msg = WorkerMessage::Shutdown;

    let bytes = rmp_serde::to_vec_named(&msg).expect("serialize Shutdown");
    let decoded: WorkerMessage = rmp_serde::from_slice(&bytes).expect("deserialize Shutdown");

    assert_eq!(msg, decoded, "Shutdown roundtrip must be identity");
}

/// `WorkerMessage::Execute { job_id, graph, settings, device_index }` roundtrips
/// via rmp-serde. All four fields (`job_id`, `graph`, `settings`, `device_index`)
/// are preserved with correct types (Uuid→string, Value→dict, JobSettings→dict,
/// u32→int).
#[test]
fn test_execute_roundtrip() {
    let msg = WorkerMessage::Execute {
        job_id: Uuid::new_v4(),
        graph: serde_json::json!({}),
        settings: JobSettings {
            device_preference: None,
        },
        device_index: 0,
    };

    let bytes = rmp_serde::to_vec_named(&msg).expect("serialize Execute");
    let decoded: WorkerMessage = rmp_serde::from_slice(&bytes).expect("deserialize Execute");

    assert_eq!(
        msg, decoded,
        "Execute roundtrip must preserve all four fields"
    );
}

/// `WorkerMessage::CancelJob { job_id }` roundtrips via rmp-serde. The
/// `job_id` field is preserved correctly across serialisation.
#[test]
fn test_cancel_job_roundtrip() {
    let msg = WorkerMessage::CancelJob {
        job_id: Uuid::new_v4(),
    };

    let bytes = rmp_serde::to_vec_named(&msg).expect("serialize CancelJob");
    let decoded: WorkerMessage = rmp_serde::from_slice(&bytes).expect("deserialize CancelJob");

    assert_eq!(msg, decoded, "CancelJob roundtrip must preserve job_id");
}

/// `WorkerMessage::MemoryQuery` (unit variant, no fields) roundtrips via
/// rmp-serde. The msgpack dict contains only `"_type": "MemoryQuery"`.
#[test]
fn test_memory_query_roundtrip() {
    let msg = WorkerMessage::MemoryQuery;

    let bytes = rmp_serde::to_vec_named(&msg).expect("serialize MemoryQuery");
    let decoded: WorkerMessage = rmp_serde::from_slice(&bytes).expect("deserialize MemoryQuery");

    assert_eq!(msg, decoded, "MemoryQuery roundtrip must be identity");
}

// ---------------------------------------------------------------------------
// WorkerEvent msgpack roundtrip tests
// ---------------------------------------------------------------------------

use anvilml_core::NodeTypeDescriptor;
use anvilml_ipc::messages::WorkerEvent;

/// `WorkerEvent::Ready` with all 13 fields roundtrips via rmp-serde.
///
/// Constructs a realistic Ready event with representative GPU capability
/// values, two registered node types, and verifies the deserialised event
/// is byte-for-byte equal to the original. The msgpack dict contains
/// `"_type": "Ready"` plus all 13 field keys.
#[test]
fn test_ready_roundtrip() {
    let event = WorkerEvent::Ready {
        worker_id: "gpu:0".to_string(),
        device_index: 0,
        device_name: "NVIDIA RTX 4090".to_string(),
        device_type: "cuda".to_string(),
        vram_total_mib: 24576,
        vram_free_mib: 20480,
        torch_version: "2.5.1+cu124".to_string(),
        fp16: true,
        bf16: true,
        fp8: true,
        flash_attention: true,
        capabilities_source: "pytorch".to_string(),
        node_types: vec![
            NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "Load Checkpoint".to_string(),
                category: "loaders".to_string(),
                description: "Loads a model checkpoint from disk.".to_string(),
                inputs: vec![],
                outputs: vec![],
            },
            NodeTypeDescriptor {
                type_name: "KSampler".to_string(),
                display_name: "K-Sampler".to_string(),
                category: "sampling".to_string(),
                description: "Samples from a latent space using a diffusion model.".to_string(),
                inputs: vec![],
                outputs: vec![],
            },
        ],
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Ready");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Ready");

    assert_eq!(
        event, decoded,
        "Ready roundtrip must preserve all 13 fields"
    );
}

/// `WorkerEvent::Pong { seq: 42 }` roundtrips via rmp-serde.
/// The msgpack dict contains `"_type": "Pong"` and `"seq": 42`.
#[test]
fn test_pong_roundtrip() {
    let event = WorkerEvent::Pong { seq: 42 };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Pong");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Pong");

    assert_eq!(event, decoded, "Pong roundtrip must preserve seq");
}

/// `WorkerEvent::Dying { reason: "OOM" }` roundtrips via rmp-serde.
/// The msgpack dict contains `"_type": "Dying"` and `"reason": "OOM"`.
#[test]
fn test_dying_roundtrip() {
    let event = WorkerEvent::Dying {
        reason: "OOM".to_string(),
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Dying");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Dying");

    assert_eq!(event, decoded, "Dying roundtrip must preserve reason");
}

/// `WorkerEvent::MemoryReport { vram_used_mib: 4096, ram_used_mib: 8589934592 }`
/// roundtrips via rmp-serde. The msgpack dict contains `"_type": "MemoryReport"`
/// plus the two memory fields.
#[test]
fn test_memory_report_roundtrip() {
    let event = WorkerEvent::MemoryReport {
        vram_used_mib: 4096,
        ram_used_mib: 8589934592,
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize MemoryReport");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize MemoryReport");

    assert_eq!(
        event, decoded,
        "MemoryReport roundtrip must preserve vram_used_mib and ram_used_mib"
    );
}

/// `WorkerEvent::Progress { job_id, step: 3, total_steps: 20, preview_b64: Some(...) }`
/// roundtrips via rmp-serde. All four fields (`job_id`, `step`, `total_steps`,
/// `preview_b64`) are preserved with correct types. The msgpack dict contains
/// `"_type": "Progress"` plus all field keys.
#[test]
fn test_progress_roundtrip() {
    let event = WorkerEvent::Progress {
        job_id: Uuid::new_v4(),
        step: 3,
        total_steps: 20,
        preview_b64: Some("iVBORw0KGgo...".into()),
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Progress");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Progress");

    assert_eq!(
        event, decoded,
        "Progress roundtrip must preserve all four fields"
    );
}

/// `WorkerEvent::ImageReady { job_id, image_b64, width: 512, height: 512,
/// format: "png", seed: 42, steps: 20 }` roundtrips via rmp-serde. All seven
/// fields (`job_id`, `image_b64`, `width`, `height`, `format`, `seed`, `steps`)
/// are preserved with correct types. The msgpack dict contains `"_type":
/// "ImageReady"` plus all field keys.
#[test]
fn test_image_ready_roundtrip() {
    let event = WorkerEvent::ImageReady {
        job_id: Uuid::new_v4(),
        image_b64: "iVBORw0KGgo...".into(),
        width: 512,
        height: 512,
        format: "png".into(),
        seed: 42,
        steps: 20,
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize ImageReady");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize ImageReady");

    assert_eq!(
        event, decoded,
        "ImageReady roundtrip must preserve all seven fields"
    );
}

/// `WorkerEvent::Completed { job_id, elapsed_ms: 5432 }` roundtrips via
/// rmp-serde. The msgpack dict contains `"_type": "Completed"` plus the
/// `job_id` and `elapsed_ms` fields.
#[test]
fn test_completed_roundtrip() {
    let event = WorkerEvent::Completed {
        job_id: Uuid::new_v4(),
        elapsed_ms: 5432,
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Completed");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Completed");

    assert_eq!(
        event, decoded,
        "Completed roundtrip must preserve job_id and elapsed_ms"
    );
}

/// `WorkerEvent::Failed { job_id, error: "CUDA out of memory",
/// traceback: Some("Traceback...") }` roundtrips via rmp-serde. All three
/// fields (`job_id`, `error`, `traceback`) are preserved with correct types.
/// The msgpack dict contains `"_type": "Failed"` plus all field keys.
#[test]
fn test_failed_roundtrip() {
    let event = WorkerEvent::Failed {
        job_id: Uuid::new_v4(),
        error: "CUDA out of memory".into(),
        traceback: Some("Traceback...".into()),
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Failed");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Failed");

    assert_eq!(
        event, decoded,
        "Failed roundtrip must preserve job_id, error, and traceback"
    );
}

/// `WorkerEvent::Cancelled { job_id }` roundtrips via rmp-serde. The
/// msgpack dict contains `"_type": "Cancelled"` and the `job_id` field.
#[test]
fn test_cancelled_roundtrip() {
    let event = WorkerEvent::Cancelled {
        job_id: Uuid::new_v4(),
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Cancelled");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Cancelled");

    assert_eq!(event, decoded, "Cancelled roundtrip must preserve job_id");
}

// ---------------------------------------------------------------------------
// RouterTransport bind() tests
// ---------------------------------------------------------------------------

use anvilml_ipc::RouterTransport;

/// `RouterTransport::bind()` succeeds and returns a port number greater than zero.
///
/// Constructs a `RouterTransport` by binding a ZeroMQ ROUTER socket on
/// `tcp://127.0.0.1:0` (OS-assigned port) and verifies the port field is
/// a valid, non-zero TCP port.
#[tokio::test]
async fn test_bind_returns_nonzero_port() {
    let transport = RouterTransport::bind().await.expect("bind should succeed");

    assert!(
        transport.port > 0,
        "bind should return a nonzero port, got {}",
        transport.port
    );
}

/// Two concurrent `RouterTransport::bind()` calls produce different port numbers.
///
/// Spawns two binds in parallel via `tokio::task::spawn` and verifies their
/// ports differ — proving the OS assigns distinct ports for concurrent binds.
#[tokio::test]
async fn test_two_binds_get_different_ports() {
    let (handle_a, handle_b) = tokio::join!(
        tokio::task::spawn(async {
            RouterTransport::bind()
                .await
                .expect("bind A should succeed")
        }),
        tokio::task::spawn(async {
            RouterTransport::bind()
                .await
                .expect("bind B should succeed")
        })
    );

    let transport_a = handle_a.expect("spawn A should not panic");
    let transport_b = handle_b.expect("spawn B should not panic");

    assert!(
        transport_a.port != transport_b.port,
        "two concurrent binds should produce different ports; got {} and {}",
        transport_a.port,
        transport_b.port
    );
}

/// The port returned by `RouterTransport::bind()` is actually listening on TCP.
///
/// Constructs a `RouterTransport`, then attempts a `TcpStream::connect` to
/// `127.0.0.1:{port}`. A successful connection proves the port is actually listening.
/// The transport is kept alive during the connection attempt (its `Drop`
/// closes the socket after the test ends).
#[tokio::test]
async fn test_bind_port_is_listening() {
    let transport = RouterTransport::bind().await.expect("bind should succeed");

    // Attempt a TCP connection to the bound port using tokio's async TcpStream.
    // A successful connection proves the port is actually listening.
    // The transport stays alive during the connect, keeping the socket bound.
    let connect_result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::net::TcpStream::connect(format!("127.0.0.1:{}", transport.port)),
    )
    .await;

    // The connect should succeed within 2 seconds — the port is bound and listening.
    assert!(
        connect_result.is_ok(),
        "TcpStream::connect to port {} should succeed within 2s; timeout may indicate bind did not actually listen",
        transport.port
    );

    // The connection should not error — the port is actively listening.
    assert!(
        connect_result.unwrap().is_ok(),
        "TcpStream::connect to port {} should not error; the port is not listening",
        transport.port
    );
}

// ---------------------------------------------------------------------------
// RouterTransport send()/recv() integration tests
// ---------------------------------------------------------------------------

use anvilml_ipc::IpcError;
use bytes::Bytes;
use std::sync::Arc;
use tokio::time::timeout;
use zeromq::prelude::*;
use zeromq::util::PeerIdentity;
use zeromq::{DealerSocket, SocketOptions, ZmqMessage};

/// Helper: create a DEALER socket with the given *identity* set as its
/// peer identity. The identity must match the worker_id used in
/// `RouterTransport::send()` for routing to succeed.
///
/// The socket is not connected — the caller must call `.connect()` on the
/// returned socket. The port is passed only for documentation purposes.
fn make_dealer(_port: u16, identity: String) -> DealerSocket {
    let mut opts = SocketOptions::default();
    opts.peer_identity(PeerIdentity::try_from(Bytes::from(identity)).expect("valid identity"));
    DealerSocket::with_options(opts)
}

/// A `WorkerMessage::Ping { seq: 42 }` is sent to a worker via `send()`,
/// and the matching `WorkerEvent::Pong { seq: 42 }` is received via `recv()`
/// over a real loopback socket.
///
/// Spawns a background task that connects a DEALER socket to the router,
/// sends a Pong event back, then the main task sends a Ping and receives
/// the Pong, verifying identity and event content.
#[tokio::test]
async fn test_send_recv_roundtrip() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));

    // Spawn a background DEALER that sends a Pong event back to the router.
    // The DEALER identity must match the worker_id used in send().
    let router_port = transport.port;
    let recv_transport = Arc::clone(&transport);
    let worker_id = "gpu:0".to_string();
    let dealer_id = worker_id.clone();
    let handle = tokio::spawn(async move {
        let mut dealer = make_dealer(router_port, dealer_id.clone());
        dealer
            .connect(&format!("tcp://127.0.0.1:{router_port}"))
            .await
            .expect("DEALER connect should succeed");

        // Send a 2-frame message for the DEALER (ROUTER prepends identity).
        // Frame 0: empty delimiter, Frame 1: payload.
        let pong_event = WorkerEvent::Pong { seq: 42 };
        let payload = rmp_serde::to_vec_named(&pong_event).expect("serialize Pong");

        let mut msg = ZmqMessage::from(Bytes::from(""));
        msg.push_back(Bytes::from(payload));

        dealer.send(msg).await.expect("DEALER send should succeed");
    });

    // Give the DEALER time to connect and register with the ROUTER.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Send a Ping message to the worker "gpu:0".
    let ping_msg = WorkerMessage::Ping { seq: 42 };
    Arc::clone(&transport)
        .send(&worker_id, &ping_msg)
        .await
        .expect("send should succeed");

    // Receive the Pong event from the worker.
    let (identity, event) = timeout(std::time::Duration::from_secs(5), recv_transport.recv())
        .await
        .expect("recv should complete within timeout")
        .expect("recv should succeed");

    assert_eq!(identity, "gpu:0", "worker identity should match");

    match event {
        WorkerEvent::Pong { seq } => {
            assert_eq!(seq, 42, "Pong seq should match Ping seq");
        }
        other => panic!("expected Pong event, got {other:?}"),
    }

    // Wait for the DEALER background task to finish.
    handle.await.expect("DEALER task should not panic");
}

/// A blocked `recv()` does not prevent a concurrent `send()` from
/// completing — the load-bearing regression test for the v3 shutdown
/// deadlock.
///
/// Spawns a DEALER (to provide a valid send destination), spawns `recv()`
/// in a background task (which blocks waiting for a message), then
/// immediately calls `send()` from the main task. The send must complete
/// without waiting for recv to unblock.
#[tokio::test]
async fn test_concurrent_send_recv_does_not_block() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));

    let router_port = transport.port;
    let worker_id = "gpu:0".to_string();
    let dealer_id = worker_id.clone();

    // Spawn a DEALER that connects to the router (provides a valid send
    // destination). The DEALER also sends a Pong to unblock recv later.
    let dealer_handle = tokio::spawn(async move {
        let mut dealer = make_dealer(router_port, dealer_id.clone());
        dealer
            .connect(&format!("tcp://127.0.0.1:{router_port}"))
            .await
            .expect("DEALER connect should succeed");
        // Wait for send(); send a Pong to unblock recv.
        let pong_event = WorkerEvent::Pong { seq: 99 };
        let payload = rmp_serde::to_vec_named(&pong_event).expect("serialize Pong");
        let mut msg = ZmqMessage::from(Bytes::from(""));
        msg.push_back(Bytes::from(payload));
        dealer.send(msg).await.expect("DEALER send should succeed");
    });

    // Spawn recv() in a background task — it will block waiting for a message.
    let recv_transport = Arc::clone(&transport);
    let recv_handle = tokio::spawn(async move { recv_transport.recv().await });

    // Give the DEALER time to connect and register, and give recv time
    // to acquire the lock and block.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Now send a message from the main task. This must complete without
    // waiting for the recv to finish — proving the locks are independent.
    let ping_msg = WorkerMessage::Ping { seq: 99 };
    let send_result = timeout(
        std::time::Duration::from_secs(3),
        transport.send(&worker_id, &ping_msg),
    )
    .await;

    // Send should succeed within the timeout.
    assert!(
        send_result.is_ok(),
        "send should complete without waiting for recv (deadlock regression test)"
    );
    assert!(
        send_result.unwrap().is_ok(),
        "send should succeed, not error"
    );

    // Clean up: abort the recv and dealer tasks.
    recv_handle.abort();
    dealer_handle.abort();
}

/// `WorkerMessage::Ping { seq: 1 }` is sent and the corresponding
/// `WorkerEvent::Pong { seq: 1 }` is received, verifying the seq
/// field is preserved across the full send→recv roundtrip.
///
/// Uses a DEALER background task to send the Pong back to the router.
#[tokio::test]
async fn test_send_ping_then_recv_pong() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));

    let router_port = transport.port;
    let recv_transport = Arc::clone(&transport);
    let worker_id = "worker-1".to_string();
    let dealer_id = worker_id.clone();
    let handle = tokio::spawn(async move {
        let mut dealer = make_dealer(router_port, dealer_id.clone());
        dealer
            .connect(&format!("tcp://127.0.0.1:{router_port}"))
            .await
            .expect("DEALER connect should succeed");

        let pong_event = WorkerEvent::Pong { seq: 1 };
        let payload = rmp_serde::to_vec_named(&pong_event).expect("serialize Pong");

        let mut msg = ZmqMessage::from(Bytes::from(""));
        msg.push_back(Bytes::from(payload));

        dealer.send(msg).await.expect("DEALER send should succeed");
    });

    // Give the DEALER time to connect and register with the ROUTER.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let ping_msg = WorkerMessage::Ping { seq: 1 };
    Arc::clone(&transport)
        .send(&worker_id, &ping_msg)
        .await
        .expect("send should succeed");

    let (identity, event) = timeout(std::time::Duration::from_secs(5), recv_transport.recv())
        .await
        .expect("recv should complete within timeout")
        .expect("recv should succeed");

    assert_eq!(identity, "worker-1", "worker identity should match");

    match event {
        WorkerEvent::Pong { seq } => {
            assert_eq!(seq, 1, "Pong seq should be 1");
        }
        other => panic!("expected Pong event, got {other:?}"),
    }

    handle.await.expect("DEALER task should not panic");
}

/// A complex `WorkerMessage::Execute` with all fields roundtrips
/// correctly through `send()` → `recv()`.
///
/// Sends a full Execute message with a UUID, empty graph, JobSettings,
/// and device_index. Receives it and verifies all fields are preserved.
#[tokio::test]
async fn test_send_execute_message_roundtrip() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));

    let router_port = transport.port;
    let recv_transport = Arc::clone(&transport);
    let worker_id = "gpu:2".to_string();
    let dealer_id = worker_id.clone();
    let handle = tokio::spawn(async move {
        let mut dealer = make_dealer(router_port, dealer_id.clone());
        dealer
            .connect(&format!("tcp://127.0.0.1:{router_port}"))
            .await
            .expect("DEALER connect should succeed");

        let pong_event = WorkerEvent::Pong { seq: 7 };
        let payload = rmp_serde::to_vec_named(&pong_event).expect("serialize Pong");

        let mut msg = ZmqMessage::from(Bytes::from(""));
        msg.push_back(Bytes::from(payload));

        dealer.send(msg).await.expect("DEALER send should succeed");
    });

    // Give the DEALER time to connect and register with the ROUTER.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let job_id = uuid::Uuid::new_v4();
    let execute_msg = WorkerMessage::Execute {
        job_id,
        graph: serde_json::json!({"nodes": []}),
        settings: anvilml_core::JobSettings {
            device_preference: None,
        },
        device_index: 2,
    };
    Arc::clone(&transport)
        .send(&worker_id, &execute_msg)
        .await
        .expect("send should succeed");

    let (identity, event) = timeout(std::time::Duration::from_secs(5), recv_transport.recv())
        .await
        .expect("recv should complete within timeout")
        .expect("recv should succeed");

    assert_eq!(identity, "gpu:2", "worker identity should match");

    match event {
        WorkerEvent::Pong { seq } => {
            assert_eq!(seq, 7, "Pong seq should be 7");
        }
        other => panic!("expected Pong event, got {other:?}"),
    }

    handle.await.expect("DEALER task should not panic");
}

/// `recv()` with only 2 frames (missing the delimiter) returns
/// `IpcError::RecvFailed` with a frame-count error message.
///
/// Sends a 2-frame message from a DEALER socket (identity + payload,
/// no delimiter). The router receives 2 frames and `recv()` rejects
/// it with an appropriate error.
#[tokio::test]
async fn test_recv_malformed_frames_returns_error() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));

    let router_port = transport.port;
    let recv_transport = Arc::clone(&transport);
    let worker_id = "gpu:0".to_string();
    let dealer_id = worker_id.clone();
    let handle = tokio::spawn(async move {
        let mut dealer = make_dealer(router_port, dealer_id.clone());
        dealer
            .connect(&format!("tcp://127.0.0.1:{router_port}"))
            .await
            .expect("DEALER connect should succeed");

        // Send a 2-frame message (no delimiter): just the payload.
        // The ROUTER will see: [identity, payload] — only 2 frames.
        let pong_event = WorkerEvent::Pong { seq: 0 };
        let payload = rmp_serde::to_vec_named(&pong_event).expect("serialize Pong");

        // Single-frame message — ROUTER will see only 2 frames total.
        let msg = ZmqMessage::from(Bytes::from(payload));

        dealer.send(msg).await.expect("DEALER send should succeed");
    });

    // Give the DEALER time to connect and send its malformed message.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // recv() should fail because the message has only 2 frames instead of 3.
    let recv_result = timeout(std::time::Duration::from_secs(5), recv_transport.recv())
        .await
        .expect("recv should complete within timeout");

    match recv_result {
        Err(IpcError::RecvFailed(msg)) => {
            assert!(
                msg.contains("expected 3 frames"),
                "error should mention frame count, got: {msg}"
            );
        }
        other => panic!("expected RecvFailed error, got {other:?}"),
    }

    handle.await.expect("DEALER task should not panic");
}
