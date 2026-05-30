//! CPU detector — the fallback device detector.
//!
//! Always returns exactly one `GpuDevice` representing the host CPU.

use anvilml_core::types::{DeviceType, GpuDevice};
use anvilml_core::AnvilError;

use crate::DeviceDetector;

/// Detects the host CPU as a fallback device.
///
/// `CpuDetector::detect()` always returns exactly one `GpuDevice`:
/// - `index = 0`
/// - `name = "CPU"`
/// - `device_type = DeviceType::Cpu`
/// - `vram_total_mib = 0` (CPU has no dedicated VRAM)
/// - `vram_free_mib = 0`
/// - `driver_version = "n/a"`
#[derive(Debug, Clone)]
pub struct CpuDetector;

impl DeviceDetector for CpuDetector {
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        Ok(vec![GpuDevice {
            index: 0,
            name: "CPU".into(),
            device_type: DeviceType::Cpu,
            vram_total_mib: 0,
            vram_free_mib: 0,
            driver_version: "n/a".into(),
        }])
    }

    fn refresh_vram(&self, _device_index: u32) -> Result<(u32, u32), AnvilError> {
        Ok((0, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_detect_returns_single_device() {
        let detector = CpuDetector;
        let devices = detector.detect().unwrap();
        assert_eq!(devices.len(), 1);
    }

    #[test]
    fn cpu_device_fields() {
        let detector = CpuDetector;
        let devices = detector.detect().unwrap();
        let dev = &devices[0];

        assert_eq!(dev.index, 0);
        assert_eq!(dev.name, "CPU");
        assert!(matches!(dev.device_type, DeviceType::Cpu));
        assert_eq!(dev.vram_total_mib, 0);
        assert_eq!(dev.vram_free_mib, 0);
        assert_eq!(dev.driver_version, "n/a");
    }

    #[test]
    fn cpu_refresh_vram_returns_zeros() {
        let detector = CpuDetector;
        let (used, total) = detector.refresh_vram(0).unwrap();
        assert_eq!(used, 0);
        assert_eq!(total, 0);
    }

    #[test]
    fn cpu_detector_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<CpuDetector>();
        assert_sync::<CpuDetector>();
    }
}
