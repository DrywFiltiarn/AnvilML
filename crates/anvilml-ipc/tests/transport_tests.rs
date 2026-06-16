//! Integration tests for `RouterTransport` bind and send operations.
//!
//! These tests exercise the actual ZeroMQ ROUTER socket — not mocks — to
//! verify that binding, port assignment, and message delivery work end-to-end
//! with a connected DEALER socket.

use anvilml_ipc::{RouterTransport, WorkerMessage};
use bytes::Bytes;
use std::sync::Arc;
use zeromq::{DealerSocket, Socket, SocketRecv, SocketSend, ZmqMessage};

/// Verify that `RouterTransport::bind()` binds successfully and returns
/// a non-zero port assigned by the OS.
#[tokio::test]
async fn bind_returns_nonzero_port() {
    let transport = RouterTransport::bind().await.expect("bind should succeed");
    assert!(
        transport.port > 0,
        "OS-assigned port must be > 0, got {}",
        transport.port
    );
}

/// Verify that `RouterTransport::send()` delivers a msgpack-encoded message
/// to a DEALER socket connected to the ROUTER's bound address.
///
/// This test discovers the DEALER's auto-generated ZeroMQ identity by having
/// the ROUTER receive a probe message first, then uses that identity to send
/// a real message via `RouterTransport::send()`.
#[tokio::test]
async fn send_delivers_message_to_dealer() {
    let transport = RouterTransport::bind().await.expect("bind should succeed");

    // Create a DEALER socket and connect it to the ROUTER's bound address.
    let bind_addr = format!("tcp://127.0.0.1:{}", transport.port);
    let mut dealer = DealerSocket::new();
    dealer
        .connect(&bind_addr)
        .await
        .expect("DEALER connect should succeed");

    // Give the connection time to establish. ZeroMQ's async connect
    // completes in the background; a brief yield lets the tokio runtime
    // process the connection handshake.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Discover the DEALER's auto-generated identity by receiving from
    // the ROUTER socket. The ROUTER returns a multipart message:
    // [identity_frame, message_frames]. We extract the identity (first frame).
    //
    // Since the transport wraps the socket in Arc<Mutex<>>, we can share
    // the Arc with a spawned recv task to discover the identity.
    let (identity_tx, identity_rx) = tokio::sync::oneshot::channel::<Bytes>();
    let socket_for_recv = Arc::clone(&transport.socket);

    // Spawn a task that receives from the ROUTER to discover the DEALER's
    // identity. The recv blocks until the DEALER sends a probe message.
    tokio::spawn(async move {
        let mut sock = socket_for_recv.lock().await;
        // The DEALER's probe message arrives as a multipart message:
        // [identity, probe_payload]. We extract the first frame as identity.
        if let Ok(msg) = sock.recv().await {
            if let Some(frame) = msg.get(0) {
                let _ = identity_tx.send(frame.clone());
            }
        }
    });

    // Send a probe from the DEALER so the ROUTER can discover its identity.
    // We use the same DEALER socket that will receive the real message.
    dealer
        .send(ZmqMessage::from("probe"))
        .await
        .expect("probe send should succeed");

    // Wait for the identity to be discovered.
    let identity_bytes = tokio::time::timeout(std::time::Duration::from_secs(2), identity_rx)
        .await
        .expect("identity discovery timed out")
        .expect("identity channel should have a value");

    // Now send the actual message using the discovered identity.
    // The identity is the raw byte sequence from the ROUTER's recv.
    transport
        .send(identity_bytes.as_ref(), &WorkerMessage::Ping { seq: 1 })
        .await
        .expect("send to known worker should succeed");

    // Verify the DEALER received the message.
    // The ROUTER returns a multipart message: [identity, encoded_payload].
    // The DEALER receives only the payload frames (without the identity).
    let received = tokio::time::timeout(std::time::Duration::from_secs(2), dealer.recv())
        .await
        .expect("recv should complete")
        .expect("received message should not be None");

    // The received message should have exactly one frame (the encoded payload).
    assert_eq!(
        received.len(),
        1,
        "expected 1 frame, got {}",
        received.len()
    );

    // Decode the payload and verify it matches the original message.
    let payload_bytes: Vec<u8> = received.into_vecdeque().pop_front().unwrap().to_vec();
    let decoded: WorkerMessage =
        rmp_serde::from_slice(&payload_bytes).expect("payload should decode as WorkerMessage");
    assert!(
        matches!(decoded, WorkerMessage::Ping { seq: 1 }),
        "expected Ping {{ seq: 1 }}, got {decoded:?}"
    );
}

/// Verify that `RouterTransport::send()` returns an error when the worker
/// identity is not connected to the ROUTER socket.
#[tokio::test]
async fn send_to_unknown_worker_returns_error() {
    let transport = RouterTransport::bind().await.expect("bind should succeed");

    // Send to a worker identity that has no connected DEALER socket.
    // The ROUTER socket will return ZmqError::Other("Destination client
    // not found by identity").
    let result = transport
        .send(b"nonexistent-worker", &WorkerMessage::Ping { seq: 1 })
        .await;

    assert!(
        result.is_err(),
        "send to unknown worker should return an error"
    );

    let err = result.unwrap_err();
    let err_string = format!("{err}");
    assert!(
        err_string.contains("Destination client not found"),
        "error should mention destination not found, got: {err_string}"
    );
}
