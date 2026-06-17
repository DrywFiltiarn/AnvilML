//! Respawn policy for managed worker subprocesses.
//!
//! Defines `RespawnPolicy` — a configuration struct that controls how and when
//! a dead worker subprocess should be respawned. The policy tracks the maximum
//! number of respawn attempts, the delay between attempts, and the time window
//! within which attempts are counted.
//!
//! `should_respawn` performs the window-reset internally: when the elapsed time
//! since `last_crash` exceeds `window_s`, the crash count is reset to zero before
//! the attempt counter is incremented. The caller passes `crash_count` by mutable
//! reference so the method can update it atomically with the decision.
//!
//! Delay computation uses exponential backoff (`delay_ms * 2^attempt`) with a
//! fixed 30-second cap, preventing rapid re-spawn loops while still responding
//! quickly to transient failures.

use std::time::{Duration, Instant};

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
#[derive(Debug, Clone)]
pub struct RespawnPolicy {
    /// Base delay in milliseconds between respawn attempts.
    pub delay_ms: u64,

    /// Maximum number of respawn attempts before giving up.
    pub max_attempts: u32,

    /// Time window in seconds within which attempts are counted toward the maximum.
    pub window_s: u32,
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

/// Maximum delay cap in milliseconds for exponential backoff.
///
/// Prevents the delay from growing unbounded on repeated crashes.
/// 30 seconds is a reasonable upper bound: long enough to let transient
/// system issues resolve, short enough to not leave the worker dead for
/// an unacceptable period.
const MAX_DELAY_MS: u64 = 30_000;

impl RespawnPolicy {
    /// Determine whether the worker should be respawned.
    ///
    /// Performs a window-reset internally: when the elapsed time since
    /// `last_crash` exceeds `window_s`, the crash count is reset to zero
    /// before the attempt counter is incremented. The caller passes
    /// `crash_count` by mutable reference so the method can update it
    /// atomically with the decision.
    ///
    /// # Arguments
    ///
    /// * `crash_count` — A mutable reference to the current crash count
    ///   within the time window. The method resets it to `0` when the
    ///   window expires, then increments it by `1` when allowing a respawn.
    /// * `last_crash` — The `Instant` when the most recent crash occurred.
    ///
    /// # Returns
    ///
    /// `true` if the worker should be respawned, `false` otherwise.
    ///
    /// Returns `false` when `crash_count >= max_attempts` after any
    /// window-reset has been applied. Otherwise returns `true` and
    /// increments `crash_count` by one.
    pub fn should_respawn(&self, crash_count: &mut u32, last_crash: Instant) -> bool {
        // Compute elapsed time since the last crash to determine if the
        // window has expired. If the window has expired, reset the crash
        // count to zero so a fresh set of attempts is allowed.
        let elapsed = last_crash.elapsed();
        if elapsed >= Duration::from_secs(self.window_s as u64) {
            *crash_count = 0;
        }

        // Reject if we have already exhausted the maximum number of attempts
        // (after any window-reset that may have occurred above).
        if *crash_count >= self.max_attempts {
            return false;
        }

        // Allow the respawn and increment the crash counter.
        *crash_count += 1;
        true
    }

    /// Compute the delay in milliseconds for the next respawn attempt.
    ///
    /// Uses exponential backoff: `delay_ms * 2^attempt`, capped at
    /// `MAX_DELAY_MS` (30 seconds) to prevent unbounded delay growth.
    ///
    /// # Arguments
    ///
    /// * `attempt` — The zero-based index of the respawn attempt.
    ///
    /// # Returns
    ///
    /// The delay in milliseconds as a `u64`. The value grows exponentially
    /// with the attempt number but never exceeds 30,000 ms.
    pub fn next_delay_ms(&self, attempt: u32) -> u64 {
        // Compute exponential backoff: delay_ms * 2^attempt.
        // Use checked_pow to avoid overflow for large attempt values,
        // then saturating_mul to handle any remaining overflow at the
        // base delay multiplication step.
        let backoff = 2u64.checked_pow(attempt).unwrap_or(u64::MAX);
        let delay = self.delay_ms.saturating_mul(backoff);

        // Cap the delay at the maximum to prevent unbounded growth.
        delay.min(MAX_DELAY_MS)
    }
}
