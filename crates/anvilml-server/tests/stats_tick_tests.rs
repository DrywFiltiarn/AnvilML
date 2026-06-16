//! Tests for the system stats tick task.
//!
//! These tests verify that the `stats_tick::start()` function correctly
//! broadcasts `WsEvent::SystemStats` events via the `EventBroadcaster`.
//! All tests use a shared in-process broadcaster — no server or network
//! is involved.

use anvilml_core::types::WsEvent;
use anvilml_server::ws::broadcaster::EventBroadcaster;
use anvilml_server::ws::stats_tick;
use std::sync::Arc;
use std::time::Duration;

/// Verify that the stats tick task broadcasts a `SystemStats` event
/// within 6 seconds of starting.
///
/// This test creates an `EventBroadcaster`, subscribes to it, calls
/// `start()`, then waits up to 6 seconds for a `SystemStats` event
/// to arrive on the broadcast channel. The event must have the correct
/// variant and field types.
#[tokio::test]
async fn test_stats_tick_broadcasts_system_stats() {
    // Create a broadcaster and subscribe to it.
    // The broadcast channel capacity of 1024 is sufficient for this test
    // which only receives a handful of events.
    let broadcaster = Arc::new(EventBroadcaster::new());
    let mut rx = broadcaster.subscribe();

    // Start the tick task. This spawns a detached tokio task that will
    // broadcast SystemStats events every 5 seconds.
    stats_tick::start(broadcaster);

    // Wait up to 6 seconds for the first SystemStats event.
    // The tick sleeps for 5 seconds before the first broadcast,
    // so 6 seconds gives a 1-second margin for scheduling variance.
    let mut found = false;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(6) {
        // Use a non-blocking recv with a short timeout.
        // tokio::sync::mpsc does not support try_recv on broadcast,
        // so we use a select with a timeout.
        tokio::select! {
            biased; // Prefer checking for events over the timeout.
            result = tokio::time::timeout(Duration::from_millis(200), rx.recv()) => {
                match result {
                    Ok(Ok(event)) => {
                        // Verify this is a SystemStats event.
                        if matches!(event, WsEvent::SystemStats { .. }) {
                            found = true;
                            break;
                        }
                    }
                    Ok(Err(_)) => {
                        // Channel closed — should not happen in this test.
                        break;
                    }
                    Err(_) => {
                        // Timeout — no event yet, continue waiting.
                        continue;
                    }
                }
            }
        }
    }

    // Assert that a SystemStats event was received.
    // If this fails, the tick task is not broadcasting correctly.
    assert!(found, "expected a SystemStats event within 6 seconds");
}

/// Verify that the CPU percentage value in a `SystemStats` event is a
/// finite `f32` (not NaN or infinity).
///
/// This test uses a dedicated broadcast channel with a receiver so we
/// can inspect the actual event fields. It waits for one event and
/// asserts that `cpu_pct.is_finite()` is true.
#[tokio::test]
async fn test_stats_tick_cpu_pct_is_finite() {
    let broadcaster = Arc::new(EventBroadcaster::new());
    let mut rx = broadcaster.subscribe();

    stats_tick::start(broadcaster);

    // Wait for one SystemStats event.
    let mut got_event = false;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(6) {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(event)) => {
                if let WsEvent::SystemStats { cpu_pct, .. } = event {
                    // CPU percentage must be a finite f32.
                    // A NaN or infinity would indicate a bug in the
                    // sysinfo API usage or the cast from f64 to f32.
                    assert!(
                        cpu_pct.is_finite(),
                        "cpu_pct must be finite, got {}",
                        cpu_pct
                    );
                    got_event = true;
                    break;
                }
            }
            Ok(Err(_)) => break,
            Err(_) => continue,
        }
    }

    assert!(got_event, "expected a SystemStats event within 6 seconds");
}

/// Verify that the RAM usage value in a `SystemStats` event is always
/// non-negative (u64, which is inherently non-negative).
///
/// This test waits for one event and asserts that `ram_used_mib` is
/// a valid non-negative value. Since the field type is `u64`, it is
/// inherently non-negative, but this test documents that invariant.
#[tokio::test]
async fn test_stats_tick_ram_used_mib_is_non_negative() {
    let broadcaster = Arc::new(EventBroadcaster::new());
    let mut rx = broadcaster.subscribe();

    stats_tick::start(broadcaster);

    // Wait for one SystemStats event.
    let mut got_event = false;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(6) {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(event)) => {
                if let WsEvent::SystemStats { ram_used_mib, .. } = event {
                    // RAM usage in mebibytes must be a reasonable positive
                    // value. Since ram_used_mib is u64, it is inherently
                    // non-negative; we assert it is positive (system RAM
                    // should always be in use).
                    assert!(
                        ram_used_mib > 0,
                        "ram_used_mib must be positive, got {}",
                        ram_used_mib
                    );
                    got_event = true;
                    break;
                }
            }
            Ok(Err(_)) => break,
            Err(_) => continue,
        }
    }

    assert!(got_event, "expected a SystemStats event within 6 seconds");
}
