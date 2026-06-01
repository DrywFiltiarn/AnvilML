//! Mock GPU detector for CI and local testing without physical GPU hardware.
//!
//! Reads three environment variables with built-in defaults to return a
//! single deterministic [`GpuDevice`] per detection call.

use anvilml_core::{AnvilError, DeviceType, GpuDevice};

use crate::DeviceDetector;

/// A detector that returns one synthetic GPU device based on environment
/// variables. Intended for CI and local testing without physical hardware.
///
/// Environment variables:
/// - `ANVILML_MOCK_DEVICE_TYPE`: `cpu`, `cuda`, or `rocm` (default: `cpu`)
/// - `ANVILML_MOCK_VRAM_MIB`: total VRAM in MiB (default: `8192`)
/// - `ANVILML_MOCK_GFX_ARCH`: device name / GFX architecture (default: `gfx1100`)
#[derive(Debug, Clone, Default)]
pub struct MockDetector;

impl DeviceDetector for MockDetector {
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        let device_type = match std::env::var("ANVILML_MOCK_DEVICE_TYPE")
            .unwrap_or_else(|_| "cpu".to_string())
            .as_str()
        {
            "cuda" => DeviceType::Cuda,
            "rocm" => DeviceType::Rocm,
            _ => DeviceType::Cpu,
        };

        let vram_mib = std::env::var("ANVILML_MOCK_VRAM_MIB")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(8192);

        let name = std::env::var("ANVILML_MOCK_GFX_ARCH").unwrap_or_else(|_| "gfx1100".to_string());

        Ok(vec![GpuDevice {
            index: 0,
            name,
            device_type,
            vram_total_mib: vram_mib,
            vram_free_mib: vram_mib,
            driver_version: "mock".to_string(),
        }])
    }

    fn refresh_vram(&self, _idx: u32) -> Result<(u32, u32), AnvilError> {
        let vram_mib = std::env::var("ANVILML_MOCK_VRAM_MIB")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(8192);
        Ok((vram_mib, vram_mib))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// MockDetector with default env vars must return a CPU device.
    #[test]
    #[serial]
    fn mock_detect_default_cpu() {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cpu");
        let detector = MockDetector::default();
        let devices = detector.detect().expect("detect should succeed");
        assert_eq!(devices.len(), 1);
        assert!(matches!(devices[0].device_type, DeviceType::Cpu));
        assert_eq!(devices[0].vram_total_mib, 8192);
        assert_eq!(devices[0].name, "gfx1100");
    }

    /// MockDetector with ANVILML_MOCK_DEVICE_TYPE=cuda must return a CUDA device.
    #[test]
    #[serial]
    fn mock_detect_cuda() {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cuda");
        let detector = MockDetector::default();
        let devices = detector.detect().expect("detect should succeed");
        assert_eq!(devices.len(), 1);
        assert!(matches!(devices[0].device_type, DeviceType::Cuda));
    }

    /// MockDetector with ANVILML_MOCK_DEVICE_TYPE=rocm must return a ROCm device.
    #[test]
    #[serial]
    fn mock_detect_rocm() {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "rocm");
        let detector = MockDetector::default();
        let devices = detector.detect().expect("detect should succeed");
        assert_eq!(devices.len(), 1);
        assert!(matches!(devices[0].device_type, DeviceType::Rocm));
    }
}
