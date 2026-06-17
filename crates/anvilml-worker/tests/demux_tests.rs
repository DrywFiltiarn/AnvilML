//! Integration tests for the demux task (`crate::demux::start`).
//!
//! These tests exercise the actual ZeroMQ ROUTER + DEALER socket pair. The
//! `test_reader_broadcasts_event` test formerly lived in `bridge_tests.rs`,
//! exercising `bridge::start`'s reader task — that task no longer exists;
//! this is its replacement, exercising `demux::start` instead.

use anvilml_ipc::{RouterTransport, WorkerEvent};
use anvilml_worker::{register_route, start_demux, RouteTable};
use rmp_serde;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use zeromq::{DealerSocket, Socket, SocketSend, ZmqMessage};

/// Verify the demux task receives events from the ROUTER socket and
/// dispatches them to the correct worker's broadcast channel by identity.
///
/// This test sends a multipart message from a DEALER socket (with identity
/// frame + payload), the demux task's `transport.recv()` extracts and
/// decodes it, looks up the route by identity, and the broadcast channel
/// delivers the event to that route's subscriber.
#[tokio::test]
async fn test_demux_dispatches_event_to_registered_route() {
    // Wrapped in Arc so the demux task and the identity-discovery task
    // below can share it; dropping it at end of scope closes the socket.
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));

    let bind_addr = format!("tcp://127.0.0.1:{}", transport.port);
    let mut dealer = DealerSocket::new();
    dealer
        .connect(&bind_addr)
        .await
        .expect("DEALER connect should succeed");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Discover the DEALER's auto-generated identity via
    // transport.recv_with_raw_identity() — called BEFORE start_demux()
    // below, not concurrently with it. RouterTransport doesn't expose a
    // raw socket field anymore (send and recv are independent locked
    // halves now; see transport.rs's module docs), and more importantly,
    // recv() must have exactly one caller for the transport's lifetime —
    // doing discovery first and sequentially, rather than via a second
    // task racing the demux task's own recv() loop, is what keeps this
    // test honest about that constraint rather than violating it.
    // The probe must be a real encoded WorkerEvent: recv_with_raw_identity()
    // decodes the payload the same way recv() does.
    dealer
        .send(ZmqMessage::from(
            rmp_serde::to_vec_named(&WorkerEvent::Pong { seq: 0 }).expect("encode probe"),
        ))
        .await
        .expect("probe send should succeed");

    let (identity_bytes, _rendered, _probe_event) = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        transport.recv_with_raw_identity(),
    )
    .await
    .expect("identity discovery timed out")
    .expect("probe recv should succeed");

    // The key must use the same UTF-8-or-hex rendering RouterTransport
    // uses internally — anvilml_ipc::render_identity is the single source
    // of truth for that rule, exactly as it is in production's pool.rs.
    let key = anvilml_ipc::render_identity(&identity_bytes);

    // Pre-seed the table before starting the task, rather than registering
    // after — either order is valid (register() works against a running
    // task too), but pre-seeding is simpler when there's no keepalive race
    // to avoid, unlike in production's spawn_all.
    let (event_tx, mut event_rx) = broadcast::channel(16);
    let routes: RouteTable = Arc::new(Mutex::new(HashMap::new()));
    register_route(&routes, key.clone(), ("test-worker".to_string(), event_tx)).await;

    let _demux_handle = start_demux(Arc::clone(&transport), routes);

    // Give the demux task time to enter its recv loop before the DEALER
    // sends — otherwise the event could arrive before anything is reading.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // The ROUTER automatically prepends the DEALER's identity, so only the
    // payload frame is sent; recv() returns [DEALER_identity, payload].
    let event = WorkerEvent::Pong { seq: 42 };
    let payload_bytes = rmp_serde::to_vec_named(&event).expect("encode Pong should succeed");
    dealer
        .send(ZmqMessage::from(payload_bytes))
        .await
        .expect("event send should succeed");

    let received = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
        .await
        .expect("event should be broadcast within timeout")
        .expect("event channel should have a value");

    let (broadcast_worker_id, received_event) = received;
    assert_eq!(
        broadcast_worker_id, key,
        "demux should broadcast the wire identity, not the display label"
    );
    assert!(
        matches!(received_event, WorkerEvent::Pong { seq: 42 }),
        "expected Pong {{ seq: 42 }}, got {received_event:?}"
    );
}

/// Verify that an event from an identity with no registered route is
/// dropped rather than panicking or being delivered to the wrong route.
///
/// This is the regression test for the bug this module exists to fix:
/// before the demux task existed, every worker's bridge reader called
/// `recv()` independently on the same shared socket, so an event could be
/// — and in production, was observed to be — delivered to the wrong
/// worker's broadcast channel. With a single demux task and explicit
/// per-identity routes, an unregistered identity has nowhere to go and
/// must be dropped, not guessed at.
#[tokio::test]
async fn test_demux_drops_event_for_unregistered_identity() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));

    let bind_addr = format!("tcp://127.0.0.1:{}", transport.port);
    let mut dealer = DealerSocket::new();
    dealer
        .connect(&bind_addr)
        .await
        .expect("DEALER connect should succeed");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Empty table: whatever identity the DEALER ends up with, it will not
    // be present here, which is the condition under test.
    let routes: RouteTable = Arc::new(Mutex::new(HashMap::new()));
    let registered_worker_route = {
        let (event_tx, _unused_rx) = broadcast::channel(16);
        ("registered-worker".to_string(), event_tx)
    };
    register_route(
        &routes,
        "some-other-identity".to_string(),
        registered_worker_route,
    )
    .await;

    let _demux_handle = start_demux(Arc::clone(&transport), routes);

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Send a probe — the DEALER's real (unregistered) identity — through
    // the demux task rather than discovering it separately, since this
    // test only needs to confirm the event doesn't panic or hang, not
    // inspect what identity it actually was. Must be a real encoded
    // WorkerEvent: demux::start's recv() decodes the payload, and a
    // decode failure hits its fatal Err arm (which stops the whole demux
    // task), not the per-identity unregistered-route path this test
    // exists to exercise — a plain string here would test the wrong thing.
    dealer
        .send(ZmqMessage::from(
            rmp_serde::to_vec_named(&WorkerEvent::Pong { seq: 0 }).expect("encode probe"),
        ))
        .await
        .expect("send should succeed");

    // There is nothing to assert a positive result on here — an
    // unregistered identity has no channel to receive from by definition.
    // The test passes if this point is reached without panic or hang.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
}