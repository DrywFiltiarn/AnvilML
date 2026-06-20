/// Per-device VRAM reservation tracking ledger.
///
/// Tracks how much VRAM has been reserved per GPU device index.
/// Pure synchronous, advisory-only — the dispatch loop must call
/// `would_fit` before `reserve` to enforce capacity limits.
/// The ledger itself panics on over-reservation as a programming
/// error guard (the dispatcher should never call `reserve` when
/// `would_fit` is false).
use std::collections::HashMap;

use tracing;

/// Per-device VRAM reservation tracking ledger.
///
/// Each registered device has a total VRAM budget and a cumulative
/// reservation counter. `would_fit` checks whether a requested amount
/// would exceed the unreserved portion; `reserve` and `release` modify
/// the reservation counter.
///
/// The ledger is advisory-only: it panics on over-reservation because
/// that represents a bug in the dispatch loop (which should have
/// checked `would_fit` first), not a recoverable runtime condition.
pub struct VramLedger {
    /// Per-device reservation totals in MiB.
    ///
    /// The value at `reservations[index]` is the cumulative amount of
    /// VRAM that has been reserved but not yet released for that device.
    reservations: HashMap<u32, u32>,

    /// Per-device total VRAM in MiB (set at registration time).
    ///
    /// This is the hardware's total VRAM, established once during
    /// device registration. It never changes after registration.
    totals: HashMap<u32, u32>,
}

impl VramLedger {
    /// Create an empty `VramLedger` with no registered devices.
    ///
    /// Returns a ledger with zero reservations and no device entries.
    /// Devices must be registered via `register_device` before any
    /// `would_fit`, `reserve`, or `release` call can succeed.
    pub fn new() -> Self {
        Self {
            reservations: HashMap::new(),
            totals: HashMap::new(),
        }
    }

    /// Register a GPU device with its total VRAM capacity.
    ///
    /// Sets the device's total VRAM in `totals` and initialises its
    /// reservation counter to zero. This is a no-op if the device is
    /// already registered — duplicate registration is harmless and
    /// prevents errors from repeated discovery scans.
    ///
    /// # Arguments
    ///
    /// * `index` — The GPU device index (zero-based).
    /// * `vram_total_mib` — The device's total VRAM in mebibytes.
    pub fn register_device(&mut self, index: u32, vram_total_mib: u32) {
        // No-op if already registered — duplicate registration from
        // repeated discovery scans must not error or double-count.
        if self.totals.contains_key(&index) {
            return;
        }

        self.totals.insert(index, vram_total_mib);
        self.reservations.insert(index, 0);
    }

    /// Check whether a requested VRAM amount would fit on a device.
    ///
    /// Returns `false` if the device index is unknown (not registered).
    /// Otherwise returns `true` if the unreserved VRAM (`total - reserved`)
    /// is greater than or equal to the requested amount.
    ///
    /// This is a pure computation with no side effects.
    ///
    /// # Arguments
    ///
    /// * `index` — The GPU device index to check.
    /// * `requested_mib` — The VRAM amount in MiB to check availability for.
    pub fn would_fit(&self, index: u32, requested_mib: u32) -> bool {
        // Unknown device: treat as zero free VRAM — the dispatch loop
        // should always register all devices before scheduling.
        let Some(&total) = self.totals.get(&index) else {
            return false;
        };

        // Pure computation: subtract current reservations from total
        // and compare against the requested amount.
        let reserved = self.reservations.get(&index).copied().unwrap_or(0);
        total - reserved >= requested_mib
    }

    /// Reserve VRAM on a device.
    ///
    /// Increments the reservation counter for the device by `mib`.
    /// Panics if the reservation would exceed total VRAM — this is
    /// intentional: it represents a programming error in the dispatch
    /// loop (which should have called `would_fit` first).
    ///
    /// Panics if the device is not registered.
    ///
    /// # Arguments
    ///
    /// * `index` — The GPU device index.
    /// * `mib` — The amount of VRAM to reserve in MiB.
    pub fn reserve(&mut self, index: u32, mib: u32) {
        // Assert the device exists — a missing device here is a bug
        // in the dispatch loop which should have registered all GPUs.
        let total = self.totals.get(&index).expect("device not registered");

        // Check for overflow before adding — this should never happen
        // if the dispatch loop calls `would_fit` first, but catch it
        // defensively in case of a scheduling bug.
        let current = self.reservations.get(&index).copied().unwrap_or(0);
        let new_total = current + mib;

        // Over-reservation is a programming error in the dispatcher.
        // The dispatch loop must call `would_fit` before `reserve`.
        // Panicking here catches the bug immediately rather than
        // silently corrupting the ledger state.
        assert!(
            new_total <= *total,
            "VRAM reservation overflow: device {index} has {total} MiB total, {} MiB already reserved, requested {} MiB more",
            current,
            mib
        );

        self.reservations.insert(index, new_total);

        let free_after = *total - new_total;
        tracing::debug!(
            device_index = index,
            reserved_mib = mib,
            free_after_mib = free_after,
            "vram reserved"
        );
    }

    /// Release previously reserved VRAM on a device.
    ///
    /// Decrements the reservation counter for the device by `mib`.
    /// Panics if the release would underflow (reservation cannot go
    /// negative — this represents a bug in the release logic).
    ///
    /// Panics if the device is not registered.
    ///
    /// # Arguments
    ///
    /// * `index` — The GPU device index.
    /// * `mib` — The amount of VRAM to release in MiB.
    pub fn release(&mut self, index: u32, mib: u32) {
        // Assert the device exists — a missing device here is a bug
        // in the dispatch loop's cleanup path.
        self.totals.get(&index).expect("device not registered");

        let current = self.reservations.get(&index).copied().unwrap_or(0);

        // Underflow check — release cannot exceed reserved amount.
        // This represents a bug in the release logic: releasing more
        // than was ever reserved.
        assert!(
            current >= mib,
            "VRAM release underflow: device {index} has {current} MiB reserved, requested to release {mib} MiB"
        );

        self.reservations.insert(index, current - mib);
    }

    /// Return a reference to the per-device reservation totals.
    ///
    /// The returned map allows the dispatch loop to compute free VRAM
    /// (total - reserved) for worker ranking without exposing the
    /// internal `HashMap` as mutable.
    pub fn reservations(&self) -> &HashMap<u32, u32> {
        &self.reservations
    }

    /// Return the total VRAM for a registered device, or `None` if
    /// the device is not registered.
    ///
    /// The dispatch loop uses this to compute free VRAM for ranking
    /// idle workers: `free = total_vram - reservations`.
    pub fn total_vram(&self, index: u32) -> Option<u32> {
        self.totals.get(&index).copied()
    }
}

impl Default for VramLedger {
    fn default() -> Self {
        Self::new()
    }
}
