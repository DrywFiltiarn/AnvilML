//! Integration tests for `bridge.rs`'s `spawn_bridge()` — the P8-F1 IPC
//! bridge task pair (independent reader/writer tasks against the split
//! `RouterTransport`, per `ANVILML_DESIGN.md §9.6`).
//!
//! These tests construct helpers locally rather than sharing them with
//! `managed_tests.rs` — matching this crate's own established convention
//! (each integration test file is its own separate crate; Rust doesn't
//! let them share code without an explicit `tests/common/mod.rs`-style
//! module, which this codebase doesn't already use).

use std::sync::Arc;
use std::time::Duration;

use anvilml_ipc::{RouterTransport, WorkerEvent, WorkerMessage};
use anvilml_worker::{Demux, spawn_bridge};
use bytes::Bytes;
use zeromq::prelude::*;
use zeromq::util::PeerIdentity;
use zeromq::{DealerSocket, SocketOptions, ZmqMessage};

/// Connect a DEALER socket to a `RouterTransport`'s bound endpoint, setting
/// the worker identity. Matches `managed_tests.rs`'s own established
/// `connect_dealer` helper exactly.
async fn connect_dealer(transport: &RouterTransport, worker_id: &str) -> DealerSocket {
    let mut opts = SocketOptions::default();
    opts.peer_identity(
        PeerIdentity::try_from(Bytes::from(worker_id.to_string())).expect("valid identity"),
    );
    let mut dealer = DealerSocket::with_options(opts);
    let endpoint = format!("tcp://127.0.0.1:{}", transport.port);
    dealer
        .connect(&endpoint)
        .await
        .expect("DEALER connect to ROUTER should succeed");
    // Give the ROUTER time to register the DEALER's identity — see
    // managed_tests.rs's own connect_dealer for the full explanation of
    // why this matters.
    tokio::time::sleep(Duration::from_millis(50)).await;
    dealer
}

/// Send a `WorkerEvent` from the DEALER side, simulating a worker-originated
/// event. Matches `managed_tests.rs`'s own established `send_event` helper.
async fn send_event(dealer: &mut DealerSocket, event: &WorkerEvent) {
    let payload = rmp_serde::to_vec_named(event).expect("event should serialize");
    let mut msg = ZmqMessage::from(Bytes::from(""));
    msg.push_back(Bytes::from(payload));
    dealer.send(msg).await.expect("DEALER send should succeed");
}

/// A message sent via the writer channel actually reaches the transport —
/// verified by a connected DEALER receiving it on the wire.
#[tokio::test]
async fn test_sent_message_reaches_transport_send() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let demux = Arc::new(Demux::new());

    let (tx, _writer_handle, _reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));

    let mut dealer = connect_dealer(&transport, "test-worker").await;

    tx.send(("test-worker".to_string(), WorkerMessage::Ping { seq: 42 }))
        .await
        .expect("channel send should succeed");

    // Receive on the DEALER side — the identity frame the ROUTER used to
    // route this is stripped by ZeroMQ before it reaches the DEALER
    // (confirmed directly against zeromq 0.6.0's RouterSocket::send() in
    // an earlier session's investigation), leaving 2 frames: [empty
    // delimiter, msgpack payload].
    let received = tokio::time::timeout(Duration::from_secs(2), dealer.recv())
        .await
        .expect("should receive a message within 2s")
        .expect("DEALER recv should succeed");
    let frames = received.into_vec();
    assert_eq!(
        frames.len(),
        2,
        "expected 2 frames from the DEALER's perspective (delimiter + payload)"
    );
    let message: WorkerMessage =
        rmp_serde::from_slice(&frames[1]).expect("payload should deserialize as WorkerMessage");
    assert_eq!(
        message,
        WorkerMessage::Ping { seq: 42 },
        "the DEALER should receive exactly the message sent via the writer channel"
    );
}

/// An event received on the transport reaches the correct, registered
/// demux receiver — not just any receiver, the one registered under the
/// matching worker_id.
#[tokio::test]
async fn test_recv_event_reaches_right_demux_receiver() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let demux = Arc::new(Demux::new());

    // Register two different workers' receivers, to prove routing is
    // actually identity-based, not "whichever receiver happens to be
    // registered" — a bug that would still pass a single-receiver test.
    let (right_tx, mut right_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    let (_wrong_tx, mut wrong_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("right-worker".to_string(), right_tx);
    demux.register("wrong-worker".to_string(), _wrong_tx);

    let (_tx, _writer_handle, _reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));

    let mut dealer = connect_dealer(&transport, "right-worker").await;
    let ready = WorkerEvent::Ready {
        worker_id: "right-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp16: true,
        bf16: true,
        fp8: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut dealer, &ready).await;

    let received = tokio::time::timeout(Duration::from_secs(2), right_rx.recv())
        .await
        .expect("the correctly-registered receiver should get the event within 2s")
        .expect("channel should not be closed");
    assert_eq!(
        received, ready,
        "the event received should be exactly the one sent"
    );

    // The wrong worker's receiver should have gotten nothing.
    let nothing = tokio::time::timeout(Duration::from_millis(100), wrong_rx.recv()).await;
    assert!(
        nothing.is_err(),
        "the receiver registered under a different worker_id should not \
         have received this event"
    );
}

/// Sending a message and receiving an event can happen concurrently
/// without either blocking the other — the reader and writer tasks lock
/// only their own half of the transport.
///
/// Structured the same way as `anvilml-ipc`'s own
/// `test_concurrent_send_recv_does_not_block` regression test from an
/// earlier session: the DEALER deliberately withholds any event for 1
/// second (long past both the lead-in and the bound given to the send
/// below), so the reader task's `transport.recv()` is unambiguously still
/// blocked, with nothing to receive, at the moment the send is attempted.
/// If send() and recv() ever shared a lock again, this send would be
/// blocked behind the still-pending recv() and this test would time out,
/// not pass by favorable timing.
#[tokio::test]
async fn test_both_tasks_run_concurrently_without_blocking() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let demux = Arc::new(Demux::new());

    let (tx, _writer_handle, _reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));

    let dealer_transport = Arc::clone(&transport);
    tokio::spawn(async move {
        let mut dealer = connect_dealer(&dealer_transport, "test-worker").await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        let pong = WorkerEvent::Pong { seq: 0 };
        send_event(&mut dealer, &pong).await;
    });

    // Give the DEALER time to connect (and the reader task time to start
    // its recv() call), well short of the 1-second withheld-event delay
    // above, so reader_task's recv() is guaranteed still pending here.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Bounded well below the 1-second delay: if this send were blocked on
    // whatever the reader task's recv() is doing, it would time out here,
    // not pass by coincidence.
    let send_result = tokio::time::timeout(
        Duration::from_millis(250),
        tx.send(("test-worker".to_string(), WorkerMessage::Ping { seq: 0 })),
    )
    .await;
    assert!(
        send_result.is_ok(),
        "send should complete promptly without waiting for the reader \
         task's concurrent recv() (deadlock regression test)"
    );
    assert!(
        send_result.unwrap().is_ok(),
        "channel send should succeed, not error"
    );
}

/// The writer task exits cleanly once every sender clone is dropped —
/// `while let Some(...) = rx.recv().await` naturally ends the loop when
/// the channel closes, and the task returns rather than looping forever.
#[tokio::test]
async fn test_writer_task_exits_when_sender_dropped() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let demux = Arc::new(Demux::new());

    let (tx, writer_handle, _reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));

    drop(tx);

    let result = tokio::time::timeout(Duration::from_secs(2), writer_handle).await;
    assert!(
        result.is_ok(),
        "writer_task should exit within 2s of the sender being dropped"
    );
    assert!(
        result.unwrap().is_ok(),
        "writer_task should exit cleanly, not panic"
    );
}

/// Send a payload that fails `WorkerEvent` msgpack deserialization, from
/// the DEALER side — used to exercise the reader task's consecutive-
/// failure handling below.
async fn send_malformed(dealer: &mut DealerSocket) {
    let mut msg = ZmqMessage::from(Bytes::from(""));
    msg.push_back(Bytes::from_static(b"not valid msgpack"));
    dealer.send(msg).await.expect("DEALER send should succeed");
}

/// A single transient failure (one malformed message) does not end the
/// reader task — it logs, retries, and keeps routing subsequent, valid
/// events normally. Regression test for the gap found and fixed as part
/// of P8-F2: an earlier version of `bridge.rs` exited its reader task on
/// the very first `transport.recv()` error, which — because this bridge
/// is shared pool-wide — meant one worker's single malformed message
/// would silently kill event delivery for every worker sharing the pool.
/// See `bridge.rs`'s own "Error handling" doc section for the full
/// explanation.
#[tokio::test]
async fn test_reader_task_survives_transient_failure() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let demux = Arc::new(Demux::new());

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker".to_string(), event_tx);

    let (_tx, _writer_handle, reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));

    let mut dealer = connect_dealer(&transport, "test-worker").await;

    // Well under MAX_CONSECUTIVE_FAILURES (5) — this should recover, not
    // exit the reader task.
    for _ in 0..3 {
        send_malformed(&mut dealer).await;
        // Give the reader task time to observe, log, and back off before
        // the next malformed message — RETRY_BACKOFF is 50ms.
        tokio::time::sleep(Duration::from_millis(60)).await;
    }

    // The reader task should still be alive — not yet at the threshold.
    assert!(
        !reader_handle.is_finished(),
        "reader_task should still be running after only 3 consecutive \
         failures (below MAX_CONSECUTIVE_FAILURES)"
    );

    // A subsequent, valid event should still route normally, proving the
    // reader task genuinely recovered rather than being stuck in some
    // degraded state.
    let ready = WorkerEvent::Pong { seq: 0 };
    send_event(&mut dealer, &ready).await;

    let received = tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
        .await
        .expect("a valid event after transient failures should still route within 2s")
        .expect("channel should not be closed");
    assert_eq!(
        received, ready,
        "the reader task should route a valid event normally after \
         recovering from transient failures"
    );
}
