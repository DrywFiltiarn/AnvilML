//! Respawn policy for managed worker subprocesses.
//!
//! Defines `RespawnPolicy` — a configuration struct that controls how and when
//! a dead worker subprocess should be respawned. The policy tracks the maximum
//! number of respawn attempts, the delay between attempts, and the time window
//! within which attempts are counted.
//!
//! **Current state:** Stub implementation. `should_respawn` always returns `true`
//! and `next_delay_ms` returns a constant delay. Full exponential backoff logic
//! is deferred to P10-A1.

use std::time::Instant;

/// Respawn policy for a managed worker subprocess.
///
/// Controls how and when a dead worker subprocess should be respawned. The policy
/// tracks the maximum number of respawn attempts, the delay between attempts,
/// and the time window within which attempts are counted toward the maximum.
///
/// # Default values
///
/// | Field        | Default | Description                                  |
/// |--------------|---------|----------------------------------------------|
/// | `delay_ms`   | 2000    | Base delay in milliseconds between attempts. |
/// | `max_attempts` | 5     | Maximum number of respawn attempts.          |
/// | `window_s`   | 300     | Time window in seconds for attempt counting. |
///
/// **Note:** This is a stub implementation. Full backoff logic is deferred to P10-A1.
#[allow(dead_code)] // max_attempts and window_s are for future backoff logic (P10-A1)
#[derive(Debug, Clone)]
pub struct RespawnPolicy {
    /// Base delay in milliseconds between respawn attempts.
    delay_ms: u64,

    /// Maximum number of respawn attempts before giving up.
    max_attempts: u32,

    /// Time window in seconds within which attempts are counted toward the maximum.
    window_s: u32,
}

impl Default for RespawnPolicy {
    /// Return the default respawn policy.
    ///
    /// Defaults: `delay_ms = 2000`, `max_attempts = 5`, `window_s = 300`.
    fn default() -> Self {
        Self {
            delay_ms: 2000,
            max_attempts: 5,
            window_s: 300,
        }
    }
}

impl RespawnPolicy {
    /// Determine whether the worker should be respawned.
    ///
    /// # Arguments
    ///
    /// * `crash_count` — The number of crashes that have occurred within the
    ///   current time window.
    /// * `last_crash` — The `Instant` when the most recent crash occurred.
    ///
    /// # Returns
    ///
    /// `true` if the worker should be respawned, `false` otherwise.
    ///
    /// **Stub:** Always returns `true`. Full backoff logic is deferred to P10-A1.
    pub fn should_respawn(&self, _crash_count: u32, _last_crash: Instant) -> bool {
        // Stub: full backoff logic deferred to P10-A1.
        // The real implementation will check:
        // 1. Whether crash_count < max_attempts
        // 2. Whether the crash window has not expired (last_crash + window_s > now)
        // 3. Whether the backoff delay has elapsed
        true
    }

    pub fn next_delay_ms(&self, _attempt: u32) -> u64 {
        // Stub: full backoff logic deferred to P10-A1.
        // The real implementation will apply exponential backoff:
        // delay_ms * 2^attempt (capped at some maximum).
        self.delay_ms
    }
}
