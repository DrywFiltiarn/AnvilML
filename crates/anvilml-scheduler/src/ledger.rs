/// Per-device VRAM reservation ledger.
///
/// `VramLedger` tracks how much VRAM (in MiB) has been reserved on each device
/// index. It is advisory only — it prevents over-scheduling but does not claim to
/// guarantee VRAM sufficiency. A real OOM during execution is still possible; the
/// worker emits `Failed`, and the scheduler calls `release()` to return the
/// reservation.
///
/// Reservations are keyed by device index (`u32`). A device index that has never
/// been reserved is treated as having zero reservation, which means
/// `free_mib(device, total)` returns `total` for an unknown device.
///
/// `release()` uses saturating subtraction so that releasing more than was
/// reserved (which can legitimately happen with imprecise estimates) never
/// panics or underflows.
#[derive(Debug, Default)]
pub struct VramLedger {
    /// Per-device VRAM reservations: device_index → reserved_mib.
    reservations: std::collections::HashMap<u32, u32>,
}

impl VramLedger {
    /// Create a new, empty `VramLedger`.
    ///
    /// Both the inner `HashMap` and all derived state start empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserve `vram_mib` MiB of VRAM on the given device.
    ///
    /// Adds `vram_mib` to the existing reservation for `device_index`, or
    /// inserts it as a new entry if the device has never been reserved.
    /// Uses saturating addition to prevent overflow on repeated reserves.
    pub fn reserve(&mut self, device_index: u32, vram_mib: u32) {
        // entry().or_insert(0) handles both the "device already has a
        // reservation" and "first reservation on this device" cases in
        // a single expression — no need for an if-else branch.
        *self.reservations.entry(device_index).or_insert(0) += vram_mib;
    }

    /// Release `vram_mib` MiB of VRAM on the given device.
    ///
    /// Subtracts `vram_mib` from the existing reservation using saturating
    /// subtraction. If the device is unknown (no reservation exists), the
    /// result is zero and the entry is not created. If more is released
    /// than was reserved (over-release), the reservation is clamped to zero
    /// rather than underflowing — this is intentional: imprecise estimates
    /// happen in practice, and the ledger is advisory.
    ///
    /// After the subtraction, if the result is zero the entry is removed
    /// from the map to keep the reservation set lean. A device with zero
    /// reservation is functionally identical to an unknown device for
    /// `free_mib()`.
    pub fn release(&mut self, device_index: u32, vram_mib: u32) {
        // Saturating subtract so over-release never panics or underflows.
        // If the device has no reservation, saturating_sub(0, vram_mib) = 0,
        // so the entry is not inserted for zero balances.
        let entry = self.reservations.entry(device_index).or_insert(0);
        *entry = entry.saturating_sub(vram_mib);

        // Remove the entry if the reservation is now zero to keep the map
        // lean. A device with zero reservation is functionally the same as
        // an unknown device for free_mib().
        if let Some(reserved) = self.reservations.get(&device_index)
            && *reserved == 0
        {
            self.reservations.remove(&device_index);
        }
    }

    /// Return the amount of VRAM (in MiB) still free on the given device.
    ///
    /// Computes `total_mib - reservation` for the given device using
    /// saturating subtraction, so that if somehow the reservation exceeds
    /// total (which shouldn't happen with correct usage but the ledger is
    /// advisory so we defend against it) the result is `0`.
    ///
    /// A device index that has never been reserved is treated as having
    /// zero reservation, so this returns `total_mib` for unknown devices.
    pub fn free_mib(&self, device_index: u32, total_mib: u32) -> u32 {
        let reserved = self.reservations.get(&device_index).copied().unwrap_or(0);
        // Saturating sub: if reservation somehow exceeds total, return 0
        // rather than panicking — the ledger is advisory so we defend
        // against incorrect caller behaviour.
        total_mib.saturating_sub(reserved)
    }

    /// Get the current VRAM reservation amount for a device index.
    ///
    /// Returns the amount currently reserved (MiB) for the given device.
    /// Returns `0` if the device has no reservation entry.
    ///
    /// Used by the event loop to determine how much VRAM to release when
    /// a terminal `WorkerEvent` arrives, without needing direct access to
    /// the worker pool's device metadata.
    pub fn get_reservation(&self, device_index: u32) -> u32 {
        *self.reservations.get(&device_index).unwrap_or(&0)
    }

    /// Test-only accessor: returns a reference to the reservations map.
    ///
    /// Allows integration tests to verify that VRAM reservations have been
    /// correctly released after terminal events.
    #[cfg(feature = "test-util")]
    pub fn reservations(&self) -> &std::collections::HashMap<u32, u32> {
        &self.reservations
    }
}
