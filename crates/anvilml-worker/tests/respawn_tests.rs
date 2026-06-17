//! Integration tests for `RespawnPolicy`.
//!
//! Each test constructs a `RespawnPolicy` with known parameters, calls
//! `should_respawn` and `next_delay_ms`, and asserts the expected results.

use std::time::Duration;

use anvilml_worker::RespawnPolicy;

/// Verify that `should_respawn` returns `false` when `crash_count`
/// equals `max_attempts` (maximum attempts exceeded).
///
/// Preconditions: `max_attempts = 3`.
/// Inputs: `crash_count = 3` (mutable ref), `last_crash = Instant::now()`.
/// Expected output: `false` — the worker should not be respawned.
#[test]
fn test_should_respawn_max_attempts_exceeded() {
    let policy = RespawnPolicy {
        delay_ms: 1000,
        max_attempts: 3,
        window_s: 60,
    };

    // crash_count == max_attempts → no more respawns.
    let mut count = 3;
    assert!(!policy.should_respawn(&mut count, std::time::Instant::now()));
}

/// Verify that `should_respawn` returns `true` when `crash_count`
/// is below `max_attempts` and the crash window has not expired.
///
/// Preconditions: `max_attempts = 5`, `window_s = 60`.
/// Inputs: `crash_count = 2` (mutable ref), `last_crash = 30 seconds ago`.
/// Expected output: `true` — the worker should be respawned; `crash_count`
/// incremented to 3.
#[test]
fn test_should_respawn_within_window() {
    let policy = RespawnPolicy {
        delay_ms: 1000,
        max_attempts: 5,
        window_s: 60,
    };

    // 30 seconds have elapsed — still within the 60-second window.
    let last_crash = std::time::Instant::now() - Duration::from_secs(30);
    let mut count = 2;
    assert!(policy.should_respawn(&mut count, last_crash));
    assert_eq!(count, 3); // incremented by the allow step
}

/// Verify that `should_respawn` resets `crash_count` to 0 when the
/// window has expired, then increments it to 1 and returns `true`.
///
/// This test asserts both the boolean return value and the counter
/// mutation — the old buggy implementation returned `true` but never
/// mutated the count, so the old signature didn't even accept a mutable
/// reference. The assertion on `count == 1` is what distinguishes the
/// correct implementation from the broken one.
///
/// Preconditions: `max_attempts = 5`, `window_s = 10`.
/// Inputs: `crash_count = 4` (mutable ref), `last_crash = 15 seconds ago`.
/// Expected output: `true`; `crash_count` == 1 (reset to 0 by window expiry,
/// then incremented to 1 by the allow step).
#[test]
fn test_should_respawn_window_reset() {
    let policy = RespawnPolicy {
        delay_ms: 1000,
        max_attempts: 5,
        window_s: 10,
    };

    // 15 seconds have elapsed — exceeds the 10-second window.
    let last_crash = std::time::Instant::now() - Duration::from_secs(15);
    let mut count = 4;
    let result = policy.should_respawn(&mut count, last_crash);
    assert!(result);
    // Window expired → count reset to 0, then incremented to 1.
    assert_eq!(count, 1);
}

/// Verify that `next_delay_ms` computes exponential backoff correctly
/// and caps at 30,000 ms.
///
/// Preconditions: `delay_ms = 1000`.
/// Inputs: attempts 0 through 5 and attempt 10.
/// Expected output:
///   attempt 0 → 1000
///   attempt 1 → 2000
///   attempt 2 → 4000
///   attempt 3 → 8000
///   attempt 4 → 16000
///   attempt 5 → 30000 (capped)
///   attempt 10 → 30000 (capped)
#[test]
fn test_next_delay_ms_exponential_backoff_and_cap() {
    let policy = RespawnPolicy {
        delay_ms: 1000,
        max_attempts: 5,
        window_s: 60,
    };

    // Exponential growth for low attempts.
    assert_eq!(policy.next_delay_ms(0), 1000);
    assert_eq!(policy.next_delay_ms(1), 2000);
    assert_eq!(policy.next_delay_ms(2), 4000);
    assert_eq!(policy.next_delay_ms(3), 8000);
    assert_eq!(policy.next_delay_ms(4), 16000);

    // Capped at 30,000 ms starting from attempt 5 (2^5 * 1000 = 32000 > 30000).
    assert_eq!(policy.next_delay_ms(5), 30000);
    assert_eq!(policy.next_delay_ms(10), 30000);
}
