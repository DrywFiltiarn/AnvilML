/// Unconditional final-fallback CPU detector.
///
/// Always returns exactly one synthesized `GpuDevice` with
/// `enumeration_source: EnumerationSource::Cpu`. This guarantees
/// `detect_all_devices()` (Phase 5) always returns at least one device,
/// per §6.2 of the design.
pub struct CpuDetector;

use crate::detect::DeviceDetector;
use anvilml_core::{
    AnvilError, CapabilitySource, DeviceType, EnumerationSource, GpuDevice, InferenceCaps,
};

impl DeviceDetector for CpuDetector {
    /// Enumerate compute devices. Always returns exactly one synthesized CPU device.
    ///
    /// This is the unconditional fallback — never returns `Err` and never panics.
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        let device = GpuDevice {
            index: 0,
            name: "CPU".into(),
            device_type: DeviceType::Cpu,
            vram_total_mib: 0,
            vram_free_mib: 0,
            driver_version: "n/a".into(),
            pci_vendor_id: 0,
            pci_device_id: 0,
            arch: None,
            caps: InferenceCaps::default(),
            enumeration_source: EnumerationSource::Cpu,
            capabilities_source: CapabilitySource::Fallback,
        };
        // CPU detector always returns exactly one synthesized fallback device.
        // This satisfies §6.2's invariant that detect_all_devices() always
        // produces Ok(HardwareInfo) with at least one device.
        Ok(vec![device])
    }

    /// Refresh VRAM for a CPU device. Always returns (0, 0) — CPU has no VRAM.
    fn refresh_vram(&self, _index: u32) -> Result<(u32, u32), AnvilError> {
        // CPU has no VRAM; the _index parameter is unused (underscore-prefixed
        // to suppress the unused-variable lint warning).
        Ok((0, 0))
    }
}
