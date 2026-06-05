//! CPU detector implementation.
//!
//! Returns a single synthetic CPU device for fallback / host-info population.

use anvilml_core::{AnvilError, DeviceType, EnumerationSource, GpuDevice};

use crate::DeviceDetector;

/// A detector that returns one synthetic CPU device.
#[derive(Debug, Clone, Default)]
pub struct CpuDetector;

impl DeviceDetector for CpuDetector {
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        Ok(vec![GpuDevice {
            index: 0,
            name: "CPU".to_string(),
            device_type: DeviceType::Cpu,
            vram_total_mib: 0,
            vram_free_mib: 0,
            driver_version: "n/a".to_string(),
            pci_vendor_id: 0,
            pci_device_id: 0,
            arch: None,
            caps: anvilml_core::InferenceCaps::default(),
            enumeration_source: EnumerationSource::Mock,
            capabilities_source: anvilml_core::CapabilitySource::Fallback,
        }])
    }

    fn refresh_vram(&self, _idx: u32) -> Result<(u32, u32), AnvilError> {
        Ok((0, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `CpuDetector::detect` must return exactly one device.
    #[test]
    fn cpu_detect_returns_one_device() {
        let detector = CpuDetector::default();
        let devices = detector.detect().expect("detect should succeed");
        assert_eq!(devices.len(), 1, "must return exactly one CPU device");
    }

    /// The returned CPU device must have correct field values.
    #[test]
    fn cpu_device_fields() {
        let detector = CpuDetector::default();
        let devices = detector.detect().expect("detect should succeed");
        let dev = &devices[0];

        assert_eq!(dev.index, 0);
        assert_eq!(dev.name, "CPU");
        assert!(matches!(dev.device_type, DeviceType::Cpu));
        assert_eq!(dev.vram_total_mib, 0);
        assert_eq!(dev.vram_free_mib, 0);
        assert_eq!(dev.driver_version, "n/a");
    }

    /// `refresh_vram` must return (0, 0) for CPU.
    #[test]
    fn cpu_refresh_vram() {
        let detector = CpuDetector::default();
        let (total, free) = detector
            .refresh_vram(0)
            .expect("refresh_vram should succeed");
        assert_eq!(total, 0);
        assert_eq!(free, 0);
    }

    /// CPU device new fields must have sensible defaults.
    #[test]
    fn cpu_device_new_fields() {
        let detector = CpuDetector::default();
        let devices = detector.detect().expect("detect should succeed");
        let dev = &devices[0];

        assert_eq!(dev.pci_vendor_id, 0);
        assert_eq!(dev.pci_device_id, 0);
        assert!(dev.arch.is_none());
        assert!(!dev.caps.fp32);
        assert!(!dev.caps.fp16);
        assert!(!dev.caps.bf16);
        assert!(!dev.caps.fp8);
        assert!(!dev.caps.fp4);
        assert!(!dev.caps.nvfp4);
        assert!(!dev.caps.flash_attention);
        assert!(matches!(dev.enumeration_source, EnumerationSource::Mock));
        assert!(matches!(
            dev.capabilities_source,
            anvilml_core::CapabilitySource::Fallback
        ));
    }
}
