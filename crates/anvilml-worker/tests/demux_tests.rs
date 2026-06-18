//! Integration tests for the demux task (`crate::demux::start`).
//!
//! These tests exercise the actual ZeroMQ ROUTER + DEALER socket pair. The
//! `test_reader_broadcasts_event` test formerly lived in `bridge_tests.rs`,
//! exercising `bridge::start`'s reader task — that task no longer exists;
//! this is its replacement, exercising `demux::start` instead.

use anvilml_ipc::{RouterTransport, WorkerEvent};
use anvilml_worker::{deregister_route, register_route, start_demux, RouteTable};
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
    // inspect what identity it actually was. Sent as a real encoded
    // WorkerEvent so this test exercises exactly one thing — the
    // unregistered-identity path — without also exercising the separate
    // decode-failure path (see test_demux_survives_undecodable_payload for
    // that one). A plain string would have hit demux::start()'s old fatal
    // Err arm before that arm was split by RecvError variant (see
    // RecvError's doc comment); now that the arm correctly distinguishes
    // a decode failure from an unregistered identity, sending garbage here
    // would still pass — but for the wrong reason, since it would no
    // longer reach the registration lookup this test claims to verify.
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

/// Verify that an undecodable payload from one peer does not stop the
/// demux task from continuing to serve every other worker.
///
/// This is the regression test for the bug described in `RecvError`'s doc
/// comment: `demux::start()`'s loop used to treat every `recv()` failure
/// as fatal to the transport as a whole (breaking the loop and killing
/// event delivery for the entire pool), when in fact a malformed payload
/// from a single peer is a per-message problem — the ROUTER socket itself
/// is still alive, and every other worker's events are unaffected. This
/// sends a payload that isn't valid msgpack at all (plain ASCII bytes)
/// from one DEALER, then proves the demux task is still running
/// afterward by sending a real, registered event from a second DEALER and
/// confirming it's still delivered.
#[tokio::test]
async fn test_demux_survives_undecodable_payload() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let bind_addr = format!("tcp://127.0.0.1:{}", transport.port);

    // First DEALER: sends the undecodable payload. Its identity is
    // deliberately never registered — this test isn't exercising the
    // unregistered-identity path (see test_demux_drops_event_for_unregistered_identity
    // for that), it just doesn't matter for this probe, since the decode
    // failure happens before a route lookup would ever occur.
    let mut bad_dealer = DealerSocket::new();
    bad_dealer
        .connect(&bind_addr)
        .await
        .expect("bad DEALER connect should succeed");

    // Second DEALER: sends a real, registered event afterward, to prove
    // the demux task is still alive and dispatching correctly.
    let mut good_dealer = DealerSocket::new();
    good_dealer
        .connect(&bind_addr)
        .await
        .expect("good DEALER connect should succeed");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Discover the good DEALER's identity the same way
    // test_demux_dispatches_event_to_registered_route does — sequentially,
    // before start_demux() runs, since recv() must have exactly one
    // caller for the transport's lifetime.
    good_dealer
        .send(ZmqMessage::from(
            rmp_serde::to_vec_named(&WorkerEvent::Pong { seq: 0 }).expect("encode probe"),
        ))
        .await
        .expect("good DEALER probe send should succeed");

    let (good_identity_bytes, _rendered, _probe_event) = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        transport.recv_with_raw_identity(),
    )
    .await
    .expect("identity discovery timed out")
    .expect("probe recv should succeed");

    let good_key = anvilml_ipc::render_identity(&good_identity_bytes);

    let routes: RouteTable = Arc::new(Mutex::new(HashMap::new()));
    let (event_tx, mut event_rx) = broadcast::channel(16);
    register_route(
        &routes,
        good_key.clone(),
        ("good-worker".to_string(), event_tx),
    )
    .await;

    let demux_handle = start_demux(Arc::clone(&transport), routes);

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Send plain ASCII bytes — not valid msgpack at all — from the bad
    // DEALER. decode_event() rejects this as IpcError::Deserialize, which
    // RecvError wraps as DecodeFailed — the per-message variant this test
    // exists to confirm is non-fatal.
    bad_dealer
        .send(ZmqMessage::from(b"not valid msgpack".to_vec()))
        .await
        .expect("bad DEALER send should succeed");

    // Give the demux task time to process (and, if the bug were still
    // present, break out of) its loop before sending the real event.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // The demux task's JoinHandle should not have finished — if the old
    // fatal-on-any-error behaviour were still present, the task would
    // have already broken out of its loop and finished by now.
    assert!(
        !demux_handle.is_finished(),
        "demux task should still be running after one undecodable payload, not stopped"
    );

    // Send a real, registered event from the good DEALER and confirm it's
    // still delivered — direct proof the recv loop is still iterating and
    // dispatching, not just that the task hasn't panicked.
    let event = WorkerEvent::Pong { seq: 99 };
    let payload_bytes = rmp_serde::to_vec_named(&event).expect("encode Pong should succeed");
    good_dealer
        .send(ZmqMessage::from(payload_bytes))
        .await
        .expect("good DEALER event send should succeed");

    let received = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
        .await
        .expect("event should be broadcast within timeout — demux task must still be alive")
        .expect("event channel should have a value");

    let (broadcast_worker_id, received_event) = received;
    assert_eq!(
        broadcast_worker_id, good_key,
        "demux should still correctly dispatch by identity after surviving the bad payload"
    );
    assert!(
        matches!(received_event, WorkerEvent::Pong { seq: 99 }),
        "expected Pong {{ seq: 99 }}, got {received_event:?}"
    );
}

/// Verify that `deregister()` removes a previously `register()`-ed route,
/// and that deregistering an already-absent key is a no-op rather than a
/// panic.
///
/// This is the regression test for the memory-leak concern that motivated
/// adding `deregister()` in the first place: before it existed, the
/// routing table only ever grew (`register`, never removed), so a crashed
/// or shut-down worker's entry — and the broadcast channel it holds open —
/// would persist for the lifetime of the process across every respawn.
/// Unlike the two tests above, this doesn't need a real transport or
/// DEALER socket — `register`/`deregister` only touch the in-memory table,
/// independent of anything `start()`'s task does with it.
#[tokio::test]
async fn test_deregister_removes_route() {
    let routes: RouteTable = Arc::new(Mutex::new(HashMap::new()));
    let key = "0".to_string();
    let (event_tx, _event_rx) = broadcast::channel(16);

    register_route(&routes, key.clone(), ("worker-0".to_string(), event_tx)).await;

    assert!(
        routes.lock().await.contains_key(&key),
        "route should be present after register()"
    );

    deregister_route(&routes, &key).await;

    assert!(
        !routes.lock().await.contains_key(&key),
        "route should be absent after deregister()"
    );

    // Deregistering an already-absent key must not panic — covers the
    // case of a worker crashing before its own spawn() call ever reaches
    // registration.
    deregister_route(&routes, &key).await;
}
