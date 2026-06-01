//! CPU detector implementation.
//!
//! Returns a single synthetic CPU device for fallback / host-info population.

use anvilml_core::{AnvilError, DeviceType, GpuDevice};

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
}
