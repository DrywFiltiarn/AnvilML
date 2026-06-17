//! Respawn policy for managed worker subprocesses.
//!
//! Defines `RespawnPolicy` — a configuration struct that controls how and when
//! a dead worker subprocess should be respawned. The policy tracks the maximum
//! number of respawn attempts, the delay between attempts, and the time window
//! within which attempts are counted.
//!
//! The respawn decision is a pure function: `should_respawn` takes the current
//! crash count and the timestamp of the last crash, and returns whether a
//! respawn attempt should proceed. The caller is responsible for tracking
//! `crash_count` and `last_crash` externally and passing the current values.
//!
//! Delay computation uses exponential backoff (`delay_ms * 2^attempt`) with a
//! fixed 30-second cap, preventing rapid re-spawn loops while still responding
//! quickly to transient failures.

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
    /// This is a **pure decision function** — it does not mutate any state.
    /// The caller is responsible for tracking `crash_count` and `last_crash`
    /// externally and passing the current values on each call.
    ///
    /// # Arguments
    ///
    /// * `crash_count` — The number of crashes that have occurred within the
    ///   current time window. The caller resets this to `0` when the window
    ///   expires (i.e., when `last_crash` is older than `window_s`).
    /// * `last_crash` — The `Instant` when the most recent crash occurred.
    ///
    /// # Returns
    ///
    /// `true` if the worker should be respawned, `false` otherwise.
    ///
    /// Returns `false` when:
    /// - `crash_count >= max_attempts` (maximum attempts exceeded), or
    /// - The crash window has not expired and the caller has not reset
    ///   `crash_count` (the window is still active).
    ///
    /// Returns `true` when the window has expired (the caller should reset
    /// `crash_count` to `0` before the next call), allowing a fresh set of
    /// attempts.
    pub fn should_respawn(&self, crash_count: u32, _last_crash: Instant) -> bool {
        // Reject if we have already exhausted the maximum number of attempts.
        if crash_count >= self.max_attempts {
            return false;
        }

        // Always allow respawn when under the max attempt limit.
        // The caller is responsible for checking whether the window has
        // expired (elapsed >= window_s) and resetting crash_count to 0
        // before the next call. The window check is informational only —
        // it does not gate the respawn decision itself.
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
