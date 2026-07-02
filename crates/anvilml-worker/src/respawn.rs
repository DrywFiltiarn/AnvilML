//! Worker crash-recovery backoff policy.
//!
//! Implements `RespawnPolicy` — a pure, zero-I/O struct that encodes the
//! worker crash-recovery backoff policy per `ANVILML_DESIGN.md §19.4`.
//! It holds three configurable parameters (delay, max attempts, window)
//! and provides decision logic for whether a crashed worker may be
//! respawned, plus the constant delay duration.

use std::time::Duration;

/// Configurable backoff policy for worker crash recovery.
///
/// Holds the three parameters that govern when a crashed worker subprocess
/// may be respawned: the constant delay between attempts, the maximum number
/// of crash attempts allowed within a sliding window, and the window duration
/// itself.
///
/// Defaults (via `Default::default()`): 2000ms delay, 5 max attempts,
/// 300s window — matching `ANVILML_DESIGN.md §19.4`.
pub struct RespawnPolicy {
    /// Constant delay in milliseconds between respawn attempts.
    respawn_delay_ms: u32,
    /// Maximum number of crash attempts allowed within the window before
    /// respawn is permanently halted.
    respawn_max_attempts: u32,
    /// Trailing window duration in seconds. Only crash attempts within
    /// this window are counted; older attempts are discarded.
    respawn_window_s: u32,
}

impl Default for RespawnPolicy {
    fn default() -> Self {
        // Documented defaults from ANVILML_DESIGN.md §19.4:
        // 2-second delay, 5 max attempts, 5-minute window.
        Self {
            respawn_delay_ms: 2000,
            respawn_max_attempts: 5,
            respawn_window_s: 300,
        }
    }
}

impl RespawnPolicy {
    /// Creates a `RespawnPolicy` with the given parameters.
    ///
    /// # Arguments
    /// * `respawn_delay_ms` — Constant delay in milliseconds between respawn
    ///   attempts. Must be > 0.
    /// * `respawn_max_attempts` — Maximum crash attempts within the window
    ///   before respawn halts permanently. Must be > 0.
    /// * `respawn_window_s` — Trailing window in seconds. Attempts older
    ///   than this are excluded from the count. Must be > 0.
    pub fn new(respawn_delay_ms: u32, respawn_max_attempts: u32, respawn_window_s: u32) -> Self {
        Self {
            respawn_delay_ms,
            respawn_max_attempts,
            respawn_window_s,
        }
    }

    /// Decides whether a crashed worker may be respawned.
    ///
    /// Counts the number of crash attempts whose timestamps fall within
    /// the trailing `respawn_window_s` window (relative to the current
    /// `Instant::now()` at call time). Returns `false` if the count is
    /// >= `respawn_max_attempts`, `true` otherwise.
    ///
    /// Attempts outside the window are discarded — they do not accumulate
    /// across calls. This implements the "sliding window" backoff: a worker
    /// that crashes rapidly will be permanently halted, but one whose
    /// crashes are spread thin enough to fall outside the window can
    /// recover.
    pub fn should_respawn(&self, attempt_history: &[std::time::Instant]) -> bool {
        let now = std::time::Instant::now();
        let window = std::time::Duration::from_secs(self.respawn_window_s as u64);

        let count_in_window = match now.checked_sub(window) {
            Some(cutoff) => attempt_history.iter().filter(|&&t| t >= cutoff).count(),
            // now is closer to this process's clock origin than the window size
            // (observed on Windows CI runners) — every entry in attempt_history
            // is therefore within the window by definition, so count them all
            // rather than panicking on the underflowing subtraction.
            None => attempt_history.len(),
        };

        count_in_window < self.respawn_max_attempts as usize
    }

    /// Returns the constant delay between respawn attempts as a `Duration`.
    ///
    /// This is a pure accessor — no exponential backoff logic is applied.
    /// The caller is responsible for sleeping for this duration before
    /// attempting to respawn.
    pub fn next_delay(&self) -> Duration {
        Duration::from_millis(self.respawn_delay_ms as u64)
    }
}
