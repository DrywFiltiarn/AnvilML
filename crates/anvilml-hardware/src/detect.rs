//! `DeviceDetector` trait — the shared contract for all GPU/CPU detectors.
//!
//! Every concrete detector (CPU, mock, Vulkan, DXGI, sysfs) implements this trait.
//! Implementations must never panic on missing drivers or hardware — they return
//! `Ok(vec![])` on detection failure per §6.2 of the design.

use anvilml_core::{AnvilError, GpuDevice};

/// Trait for detecting and refreshing GPU device information.
///
/// Every concrete detector (CPU, mock, Vulkan, DXGI, sysfs) implements this trait.
/// Implementations must never panic on missing drivers or hardware — they return
/// `Ok(vec![])` on detection failure per §6.2 of the design.
pub trait DeviceDetector: Send + Sync {
    /// Enumerate all compute devices on the host.
    ///
    /// Returns a vector of detected `GpuDevice` structs. If no devices are found,
    /// returns `Ok(vec![])` — never an error or a panic. The caller (Phase 5's
    /// `detect_all_devices`) appends a CPU fallback device if the result is empty.
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>;

    /// Refresh VRAM totals for a device by its index.
    ///
    /// Returns `(total_mib, free_mib)` — the total and free VRAM in mebibytes for
    /// the device at the given `index`. This is called at dispatch time to get a
    /// current snapshot rather than relying on the stale value from `detect()`.
    fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>;
}
