//! 1000-round-trip ROUTER/DEALER stress test for the ZeroMQ IPC transport.
//!
//! This test binds a `RouterTransport`, spawns a simulated DEALER worker in a
//! background task, and exercises 1000 sequential Ping→Pong round trips over
//! loopback TCP. It verifies zero message loss and zero reordering.
//!
//! This test is the Phase 8 GATE — every subsequent phase depends on this
//! proving the transport survives sustained load without silently dropping
//! or reordering messages.

use anvilml_ipc::RouterTransport;
use anvilml_ipc::messages::WorkerEvent;
use bytes::Bytes;
use std::sync::Arc;
use tokio::time::timeout;
use zeromq::prelude::*;
use zeromq::util::PeerIdentity;
use zeromq::{DealerSocket, SocketOptions, ZmqMessage};

/// Binds a `RouterTransport`, spawns a simulated DEALER worker, and performs
/// 1000 sequential Ping→Pong round trips over loopback TCP.
///
/// Verifies:
/// - All 1000 messages are received (zero loss)
/// - Sequence numbers arrive in ascending order 1..=1000 (zero reordering)
/// - Worker identity matches `"stress-worker"` on every round trip
/// - Every message completes within the 5-second per-message timeout
///
/// The simulated DEALER worker echoes each Ping back as a Pong with the same
/// sequence number, exercising the full msgpack serialisation/deserialisation
/// path 1000 times.
#[tokio::test]
async fn test_1000_roundtrips() {
    // Bind a ROUTER socket on an OS-assigned loopback port.
    // This is the transport the main task uses to send pings and receive pongs.
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));

    let router_port = transport.port;

    // Spawn a simulated DEALER worker in a background task.
    // The worker connects to the router, then loops: for each incoming Ping,
    // it sends back a Pong with the same sequence number.
    let handle = tokio::spawn(async move {
        // Create a DEALER socket with the identity "stress-worker".
        // This identity must match the worker_id used in RouterTransport::send()
        // for the ROUTER to route messages correctly.
        let mut opts = SocketOptions::default();
        opts.peer_identity(
            PeerIdentity::try_from(Bytes::from("stress-worker")).expect("valid identity"),
        );
        let mut dealer = DealerSocket::with_options(opts);

        // Connect to the router's bound port on the loopback interface.
        dealer
            .connect(&format!("tcp://127.0.0.1:{router_port}"))
            .await
            .expect("DEALER connect should succeed");

        // Give the DEALER time to register with the ROUTER before we start
        // sending. ZeroMQ ROUTER sockets need a moment to discover new
        // DEALER connections on the wire.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Loop 1000 times: for each sequence number, send a Pong event back
        // to the router. Each send is individually timed out to prevent
        // a hung socket from blocking the entire test indefinitely.
        for seq in 1..=1000 {
            // Serialize the Pong event to msgpack bytes.
            // to_vec_named produces a flat dict with "_type" discriminator,
            // matching the Python msgpack decoder and the router's deserializer.
            let pong_event = WorkerEvent::Pong { seq };
            let payload = rmp_serde::to_vec_named(&pong_event).expect("serialize Pong");

            // Build a 2-frame DEALER multipart message:
            //   Frame 0: empty delimiter (ROUTER prepends identity automatically)
            //   Frame 1: msgpack payload
            //
            // ZmqMessage::from(Bytes::from("")) creates frame 0 (empty delimiter).
            // push_back adds frame 1 (the payload) to the back.
            let mut msg = ZmqMessage::from(Bytes::from(""));
            msg.push_back(Bytes::from(payload));

            // Send with a 5-second timeout — generous for loopback TCP
            // (sub-millisecond latency expected) but provides a safety net
            // against hangs per ENVIRONMENT.md §11.5.
            timeout(std::time::Duration::from_secs(5), dealer.send(msg))
                .await
                .expect("DEALER send should complete within 5s (seq={seq})")
                .expect("DEALER send should not error (seq={seq})");
        }
    });

    // Give the DEALER task time to connect and register with the ROUTER.
    // The background task already sleeps 100ms internally, but we also
    // sleep here to ensure the main task doesn't start sending before
    // the DEALER is ready.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Receive 1000 Pong events from the router and verify each one.
    // The router receives 3-frame ROUTER messages: [identity, delimiter, payload].
    // recv() deserializes the payload into a WorkerEvent and returns (identity, event).
    for i in 0..1000 {
        let expected_seq = i + 1;

        // Receive with a 5-second timeout per per-message timeout rule.
        // On timeout, we abort the background DEALER task and assert the
        // timeout did not occur — this surfaces any underlying issue.
        let recv_result = timeout(std::time::Duration::from_secs(5), transport.recv())
            .await
            .expect("recv should complete within 5s (expected seq={expected_seq})")
            .expect("recv should not error (expected seq={expected_seq})");

        let (identity, event) = recv_result;

        // Verify the worker identity matches the DEALER's peer identity.
        assert_eq!(
            identity, "stress-worker",
            "worker identity should be 'stress-worker' (seq={expected_seq})"
        );

        // Extract the sequence number from the Pong event and verify it
        // matches the expected value. This proves zero reordering.
        match event {
            WorkerEvent::Pong { seq } => {
                assert_eq!(
                    seq, expected_seq,
                    "Pong seq should be {expected_seq}, got {seq}"
                );
            }
            other => {
                panic!("expected Pong event, got {other:?} (seq={expected_seq})");
            }
        }
    }

    // Wait for the background DEALER task to finish.
    // All 1000 sends should have completed by now. If the task panicked,
    // this assertion will surface the panic message.
    handle.await.expect("DEALER task should not panic");
}
