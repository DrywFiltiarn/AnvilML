//! Integration tests for the IPC bridge writer task (`start` function).
//!
//! These tests exercise the actual ZeroMQ ROUTER + DEALER socket pair to
//! verify the bridge writer task functions correctly. Unit testing is not
//! feasible because the bridge operates on a real `RouterTransport` backed
//! by a ZeroMQ ROUTER socket — mocking would require the zeromq 0.6 API to
//! support socket injection, which it does not.
//!
//! There is no reader test here: `bridge::start` no longer reads from the
//! transport at all (see `crate::bridge`'s module docs for why a per-worker
//! reader was unsound). Reader/dispatch behavior is exercised in
//! `demux_tests.rs` instead, against `crate::demux::start`.

use anvilml_ipc::{RouterTransport, WorkerEvent, WorkerMessage};
use anvilml_worker::start;
use rmp_serde;
use std::sync::Arc;
use tokio::sync::mpsc;
use zeromq::{DealerSocket, Socket, SocketRecv, SocketSend, ZmqMessage};

/// Verify the bridge writer task receives messages from the mpsc channel and
/// forwards them to the ROUTER socket via `RouterTransport::send()`.
///
/// This test discovers the DEALER's auto-generated identity via a probe
/// message, then passes the raw identity bytes to the bridge writer.
/// The writer sends a message using that identity, and the DEALER receives it.
#[tokio::test]
async fn test_writer_sends_message() {
    // Bind a real ROUTER transport to get an OS-assigned port.
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

    // Discover the DEALER's auto-generated identity by receiving its probe
    // message through transport.recv_with_raw_identity() — RouterTransport
    // doesn't expose a raw socket field anymore (send and recv are now
    // independent locked halves; see transport.rs's module docs), so the
    // raw bytes this test needs to address the DEALER again come from the
    // dedicated test/diagnostic method instead of reaching into a field.
    // The probe must be a real encoded WorkerEvent, not an arbitrary string —
    // recv_with_raw_identity() decodes the payload the same way recv() does.
    dealer
        .send(ZmqMessage::from(
            rmp_serde::to_vec_named(&WorkerEvent::Pong { seq: 0 })
                .expect("encode probe"),
        ))
        .await
        .expect("probe send should succeed");

    let (identity_bytes, _worker_id, _probe_event) = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        transport.recv_with_raw_identity(),
    )
    .await
    .expect("identity discovery timed out")
    .expect("probe recv should succeed");

    // Use the raw identity bytes for the bridge worker_id. The ROUTER socket
    // routes based on raw bytes, so we must pass the exact identity bytes
    // that the ROUTER discovered from the DEALER socket.
    let worker_id = identity_bytes;

    // Create the mpsc channel for the bridge.
    let (msg_tx, msg_rx) = mpsc::channel(16);

    // Spawn the bridge writer task.
    let writer_handle = start(
        Arc::new(transport),
        worker_id,
        "test-worker".to_string(),
        msg_rx,
    );

    // Send a message through the mpsc channel.
    msg_tx
        .send(WorkerMessage::Ping { seq: 1 })
        .await
        .expect("send should succeed");

    // Drop the sender to signal the writer to exit.
    drop(msg_tx);

    // Verify the DEALER received the message.
    // The ROUTER returns a multipart message: [identity, encoded_payload].
    // The DEALER receives only the payload frames (without the identity).
    let received = tokio::time::timeout(std::time::Duration::from_secs(2), dealer.recv())
        .await
        .expect("recv should complete")
        .expect("received message should not be None");

    assert_eq!(
        received.len(),
        1,
        "expected 1 frame, got {}",
        received.len()
    );

    let payload_bytes: Vec<u8> = received.into_vecdeque().pop_front().unwrap().to_vec();
    let decoded: WorkerMessage =
        rmp_serde::from_slice(&payload_bytes).expect("payload should decode as WorkerMessage");
    assert!(
        matches!(decoded, WorkerMessage::Ping { seq: 1 }),
        "expected Ping {{ seq: 1 }}, got {decoded:?}"
    );

    // The writer should have exited cleanly after the channel was drained
    // and the sender was dropped.
    tokio::time::timeout(std::time::Duration::from_secs(2), writer_handle)
        .await
        .expect("writer should exit within timeout")
        .expect("writer should not panic");
}

/// Verify that dropping the bridge writer's handle does not panic.
///
/// This test creates a ROUTER socket, spawns the bridge writer with a
/// dummy mpsc channel, and immediately drops its handle. The task should
/// exit cleanly once its channel sender is dropped.
#[tokio::test]
async fn test_handle_drops_cleanly() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));

    // Create the mpsc channel for the bridge.
    let (msg_tx, msg_rx) = mpsc::channel(16);

    // Spawn the bridge writer task.
    let writer_handle = start(
        Arc::clone(&transport),
        b"test-worker".to_vec(),
        "test-worker".to_string(),
        msg_rx,
    );

    // Drop the sender to signal the writer to exit.
    drop(msg_tx);

    // Dropping the handle immediately should not panic — the writer task
    // exits on its own once the channel closes; dropping the handle here
    // only detaches the test from waiting on that exit, it doesn't force it.
    drop(writer_handle);

    drop(transport);
}