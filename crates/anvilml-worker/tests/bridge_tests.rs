//! Integration tests for the IPC bridge (`start` function).
//!
//! These tests exercise the actual ZeroMQ ROUTER + DEALER socket pair to verify
//! the bridge writer and reader tasks function correctly. Unit testing is not
//! feasible because the bridge operates on a real `RouterTransport` backed by
//! a ZeroMQ ROUTER socket — mocking would require the zeromq 0.6 API to support
//! socket injection, which it does not.

use anvilml_ipc::{RouterTransport, WorkerEvent, WorkerMessage};
use anvilml_worker::start;
use bytes::Bytes;
use rmp_serde;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot};
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

    // Discover the DEALER's auto-generated identity by receiving from
    // the ROUTER socket. The ROUTER returns a multipart message:
    // [identity_frame, message_frames]. We extract the identity (first frame).
    let (identity_tx, identity_rx) = oneshot::channel::<Bytes>();
    let socket_for_recv = Arc::clone(&transport.socket);

    tokio::spawn(async move {
        let mut sock = socket_for_recv.lock().await;
        if let Ok(msg) = sock.recv().await {
            if let Some(frame) = msg.get(0) {
                let _ = identity_tx.send(frame.clone());
            }
        }
    });

    // Send a probe from the DEALER so the ROUTER can discover its identity.
    dealer
        .send(ZmqMessage::from("probe"))
        .await
        .expect("probe send should succeed");

    let identity_bytes = tokio::time::timeout(std::time::Duration::from_secs(2), identity_rx)
        .await
        .expect("identity discovery timed out")
        .expect("identity channel should have a value");

    // Use the raw identity bytes for the bridge worker_id. The ROUTER socket
    // routes based on raw bytes, so we must pass the exact identity bytes
    // that the ROUTER discovered from the DEALER socket.
    let worker_id = identity_bytes.to_vec();

    // Create the mpsc channel and broadcast channel for the bridge.
    let (msg_tx, msg_rx) = mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);

    // Spawn the bridge writer task.
    let (writer_handle, _reader_handle) = start(Arc::new(transport), worker_id, msg_rx, event_tx);

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

/// Verify the bridge reader task receives events from the ROUTER socket and
/// broadcasts them via the `broadcast::Sender`.
///
/// This test sends a multipart message from a DEALER socket (with identity
/// frame + payload), the bridge reader's transport.recv() extracts and decodes
/// it, and the broadcast channel delivers the event.
#[tokio::test]
async fn test_reader_broadcasts_event() {
    // Wrap the transport in Arc so we can share it between the test
    // setup and the bridge tasks, and drop it to trigger reader shutdown.
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));

    // Create a DEALER socket and connect it to the ROUTER's bound address.
    let bind_addr = format!("tcp://127.0.0.1:{}", transport.port);
    let mut dealer = DealerSocket::new();
    dealer
        .connect(&bind_addr)
        .await
        .expect("DEALER connect should succeed");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Discover the DEALER's auto-generated identity.
    let (identity_tx, identity_rx) = oneshot::channel::<Bytes>();
    let socket_for_recv = Arc::clone(&transport.socket);

    tokio::spawn(async move {
        let mut sock = socket_for_recv.lock().await;
        if let Ok(msg) = sock.recv().await {
            if let Some(frame) = msg.get(0) {
                let _ = identity_tx.send(frame.clone());
            }
        }
    });

    dealer
        .send(ZmqMessage::from("probe"))
        .await
        .expect("probe send should succeed");

    let identity_bytes = tokio::time::timeout(std::time::Duration::from_secs(2), identity_rx)
        .await
        .expect("identity discovery timed out")
        .expect("identity channel should have a value");

    let worker_id = identity_bytes.to_vec();

    // Create the mpsc and broadcast channels for the bridge.
    let (msg_tx, msg_rx) = mpsc::channel(16);
    let (event_tx, mut event_rx) = broadcast::channel(16);

    // Spawn the bridge reader task. The writer is also spawned but its
    // channel sender is dropped immediately so the writer exits.
    let (writer_handle, _reader_handle) =
        start(Arc::clone(&transport), worker_id.clone(), msg_rx, event_tx);

    // Drop the writer's sender so the writer exits cleanly.
    drop(msg_tx);

    // Give the reader task time to start its recv loop. This ensures
    // the reader is waiting on the socket when we send the message.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Encode a WorkerEvent and send it via the DEALER socket.
    // The ROUTER automatically prepends the DEALER's identity, so we
    // only send the payload frame. The ROUTER's recv() will return
    // [DEALER_identity, payload], which is the format the reader expects.
    let event = WorkerEvent::Pong { seq: 42 };
    let payload_bytes = rmp_serde::to_vec_named(&event).expect("encode Pong should succeed");

    // Send only the payload — the ROUTER will prepend the DEALER's
    // auto-generated identity, giving [auto_id, payload].
    dealer
        .send(ZmqMessage::from(payload_bytes))
        .await
        .expect("event send should succeed");

    // Give the reader task time to receive and broadcast the event.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Read the event from the broadcast channel.
    let received = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
        .await
        .expect("event should be broadcast within timeout")
        .expect("event channel should have a value");

    let (broadcast_worker_id, received_event) = received;
    // The broadcast worker_id is the identity as returned by
    // RouterTransport::recv() — UTF-8 if valid, hex encoding otherwise.
    // The raw identity bytes from the probe may not be valid UTF-8
    // (they're typically a UUID), so we compare against the hex
    // representation.
    let expected_hex: String = worker_id.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        broadcast_worker_id, expected_hex,
        "worker_id should match the hex-encoded discovered identity"
    );
    assert!(
        matches!(received_event, WorkerEvent::Pong { seq: 42 }),
        "expected Pong {{ seq: 42 }}, got {received_event:?}"
    );

    // The reader is still running (its Arc clone keeps the socket alive).
    // We don't wait for it to exit — the important assertion is that the
    // event was received and broadcast, which we verified above.
    // The reader will eventually exit when the test's Arc is dropped
    // (at end of function) and the socket is closed.

    // The writer should have already exited (sender was dropped).
    tokio::time::timeout(std::time::Duration::from_secs(2), writer_handle)
        .await
        .expect("writer should exit within timeout")
        .expect("writer should not panic");
}

/// Verify that dropping both bridge task handles does not panic.
///
/// This test creates a ROUTER socket, spawns both bridge tasks with a
/// dummy mpsc channel, and immediately drops both handles. The tasks
/// should exit cleanly when their respective channels/transport close.
#[tokio::test]
async fn test_handles_drop_cleanly() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));

    // Create the mpsc and broadcast channels for the bridge.
    let (msg_tx, msg_rx) = mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);

    // Spawn both bridge tasks.
    let (writer_handle, reader_handle) = start(
        Arc::clone(&transport),
        b"test-worker".to_vec(),
        msg_rx,
        event_tx,
    );

    // Drop the sender to signal the writer to exit.
    drop(msg_tx);

    // Drop both handles immediately — this should not panic.
    // The writer will exit because the channel is closed.
    // The reader will eventually exit because the transport is dropped
    // below (the socket closes, causing recv to fail).
    drop(writer_handle);
    drop(reader_handle);

    // The transport is also dropped here, which closes the socket.
    // This ensures the reader's recv() call fails and the task exits.
    // (If the reader were still running, it would be waiting on recv().)
    drop(transport);
}
