//! Per-device VRAM ledger for the scheduler's dispatch admission logic.
//!
//! Tracks total and used VRAM (in MiB) per GPU device index. Supports
//! initialisation from `HardwareInfo` and provides `free_mib` / `would_fit`
//! queries for dispatch ranking.

use std::collections::HashMap;

use anvilml_core::types::hardware::HardwareInfo;
use tracing;

/// A ledger of per-device VRAM usage.
///
/// The map key is the GPU device index; the value is `(total_mib, used_mib)`.
pub struct VramLedger {
    devices: HashMap<u32, (u32, u32)>,
}

impl VramLedger {
    /// Create a new, empty `VramLedger`.
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }

    /// Record or update the VRAM state for a device.
    ///
    /// `used_mib` and `total_mib` are stored as-is.
    pub fn update(&mut self, device_index: u32, used_mib: u32, total_mib: u32) {
        self.devices.insert(device_index, (total_mib, used_mib));
        tracing::debug!(device_index, total_mib, used_mib, "VRAM ledger updated");
    }

    /// Return the free VRAM (in MiB) for a device.
    ///
    /// Returns `total - used` if the device is known, `0` otherwise.
    pub fn free_mib(&self, device_index: u32) -> u32 {
        self.devices
            .get(&device_index)
            .map(|(total, used)| total.saturating_sub(*used))
            .unwrap_or(0)
    }

    /// Return `true` if `required_mib` would fit in the free VRAM of the device.
    pub fn would_fit(&self, device_index: u32, required_mib: u32) -> bool {
        self.free_mib(device_index) >= required_mib
    }

    /// Populate the ledger from a `HardwareInfo` snapshot.
    ///
    /// For each GPU, `total_mib` is set to `vram_total_mib` and `used_mib`
    /// is derived as `total - vram_free_mib` (clamped via `saturating_sub`).
    pub fn init_from(&mut self, hw: &HardwareInfo) {
        for gpu in &hw.gpus {
            let total = gpu.vram_total_mib;
            let used = total.saturating_sub(gpu.vram_free_mib);
            self.update(gpu.index, used, total);
        }
    }
}

impl Default for VramLedger {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anvilml_core::types::hardware::{DeviceType, GpuDevice, HostInfo, InferenceCaps};

    /// `init_from` correctly populates the ledger from a `HardwareInfo` with
    /// two GPUs, each with distinct total/free values.
    #[test]
    fn test_init_from() {
        let hw = HardwareInfo {
            host: HostInfo {
                os: "Linux 6.1.0".to_string(),
                cpu_model: "Test CPU".to_string(),
                ram_total_mib: 32768,
                ram_free_mib: 28000,
            },
            gpus: vec![
                GpuDevice {
                    index: 0,
                    name: "GPU 0".to_string(),
                    device_type: DeviceType::Cuda,
                    vram_total_mib: 8192,
                    vram_free_mib: 6192,
                    driver_version: "1.0".to_string(),
                    pci_vendor_id: 0x10de,
                    pci_device_id: 0x20b0,
                    arch: None,
                    caps: InferenceCaps::default(),
                    enumeration_source: Default::default(),
                    capabilities_source: Default::default(),
                    db_group_name: None,
                },
                GpuDevice {
                    index: 1,
                    name: "GPU 1".to_string(),
                    device_type: DeviceType::Rocm,
                    vram_total_mib: 16384,
                    vram_free_mib: 14384,
                    driver_version: "1.0".to_string(),
                    pci_vendor_id: 0x1002,
                    pci_device_id: 0x73a0,
                    arch: None,
                    caps: InferenceCaps::default(),
                    enumeration_source: Default::default(),
                    capabilities_source: Default::default(),
                    db_group_name: None,
                },
            ],
            inference_caps: InferenceCaps::default(),
        };

        let mut ledger = VramLedger::new();
        ledger.init_from(&hw);

        // Device 0: total=8192, used=8192-6192=2000, free=6192.
        assert_eq!(ledger.free_mib(0), 6192);
        // Device 1: total=16384, used=16384-14384=2000, free=14384.
        assert_eq!(ledger.free_mib(1), 14384);
    }

    /// `update` inserts a new device entry and `free_mib` returns the correct
    /// difference.
    #[test]
    fn test_update() {
        let mut ledger = VramLedger::new();
        ledger.update(0, 4096, 8192);

        assert_eq!(ledger.free_mib(0), 4096);
    }

    /// `would_fit` returns `true` when free VRAM >= required.
    #[test]
    fn test_would_fit_true() {
        let mut ledger = VramLedger::new();
        ledger.update(0, 4096, 8192);

        assert!(ledger.would_fit(0, 3000));
    }

    /// `would_fit` returns `false` when free VRAM < required.
    #[test]
    fn test_would_fit_false() {
        let mut ledger = VramLedger::new();
        ledger.update(0, 4096, 8192);

        assert!(!ledger.would_fit(0, 5000));
    }

    /// `free_mib` returns `0` for an unknown device index.
    #[test]
    fn test_free_mib_unknown_device() {
        let ledger = VramLedger::new();

        assert_eq!(ledger.free_mib(99), 0);
    }
}
