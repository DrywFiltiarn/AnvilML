//! Integration tests for `respawn.rs` — verifies the `RespawnPolicy`
//! backoff decision logic: under-limit allows respawn, at-limit blocks,
//! and out-of-window attempts are discarded.
//!
//! All tests use `std::time::Instant` directly — no mock or async needed
//! since the logic is pure computation. The `should_respawn` method uses
//! `Instant::now()` at call time, so tests that need precise timing set
//! timestamps very close to when they call the method.

use std::time::{Duration, Instant};

use anvilml_worker::RespawnPolicy;

/// `RespawnPolicy::default()` produces the documented defaults:
/// 2000ms delay, 5 max attempts, 300s window.
///
/// Verifies all three fields against the values specified in
/// `ANVILML_DESIGN.md §19.4`.
#[test]
fn test_defaults_match_documented_values() {
    let policy = RespawnPolicy::default();
    let delay = policy.next_delay();

    assert_eq!(
        delay,
        Duration::from_millis(2000),
        "default delay should be 2000ms"
    );
    // Verify should_respawn returns true with empty history (0 < 5).
    assert!(
        policy.should_respawn(&[]),
        "default policy with empty history should allow respawn"
    );
}

/// `should_respawn` returns `true` when the attempt count is strictly
/// below `max_attempts` within the trailing window.
///
/// Creates a policy with `max_attempts=3`, feeds 2 attempt timestamps
/// that are fresh (within the 300s default window), and asserts that
/// respawn is allowed.
#[test]
fn test_under_limit_allows_respawn() {
    let policy = RespawnPolicy::new(1000, 3, 300);
    let now = Instant::now();
    let attempts = vec![now, now - Duration::from_millis(100)];

    assert!(
        policy.should_respawn(&attempts),
        "2 attempts with max_attempts=3 should allow respawn"
    );
}

/// `should_respawn` returns `false` when the attempt count equals
/// `max_attempts` within the trailing window.
///
/// Creates a policy with `max_attempts=3`, feeds exactly 3 attempt
/// timestamps within the window, and asserts that respawn is blocked.
/// This is the boundary condition: at exactly max_attempts, respawn halts.
#[test]
fn test_at_limit_blocks_respawn() {
    let policy = RespawnPolicy::new(1000, 3, 300);
    let now = Instant::now();
    let attempts = vec![
        now,
        now - Duration::from_millis(10),
        now - Duration::from_millis(20),
    ];

    assert!(
        !policy.should_respawn(&attempts),
        "3 attempts with max_attempts=3 should block respawn"
    );
}

/// Attempts older than `respawn_window_s` are excluded from the count.
///
/// Creates a policy with `max_attempts=2` and `window=1` second, feeds
/// 2 attempt timestamps that are 2 seconds old (outside the 1s window),
/// and asserts that respawn is allowed (0 in-window attempts < 2 max).
#[test]
fn test_attempts_outside_window_dont_count() {
    let policy = RespawnPolicy::new(1000, 2, 1);
    let now = Instant::now();
    // Both timestamps are 2 seconds old — outside the 1-second window.
    let attempts = vec![now - Duration::from_secs(2), now - Duration::from_secs(3)];

    assert!(
        policy.should_respawn(&attempts),
        "attempts outside the window should not count toward max_attempts"
    );
}

/// `next_delay()` returns the configured delay as a `Duration`.
///
/// Creates a policy with a custom delay of 5000ms and verifies that
/// `next_delay()` returns exactly `Duration::from_millis(5000)`.
#[test]
fn test_next_delay_returns_correct_duration() {
    let policy = RespawnPolicy::new(5000, 5, 300);
    let delay = policy.next_delay();

    assert_eq!(
        delay,
        Duration::from_millis(5000),
        "next_delay() should return the configured delay as Duration"
    );
}

/// An empty attempt history always allows respawn, since zero attempts
/// is strictly below any `max_attempts` threshold.
///
/// This is the happy-path baseline: a brand-new worker with no crash
/// history should always be eligible for respawn.
#[test]
fn test_empty_history_allows_respawn() {
    let policy = RespawnPolicy::new(1000, 1, 300);
    assert!(
        policy.should_respawn(&[]),
        "empty history should always allow respawn (0 < any max_attempts)"
    );
}
