//! Deterministic mock detector for CI.
//!
//! Returns a single `GpuDevice` whose values are driven by environment variables:
//! - `ANVILML_MOCK_DEVICE_TYPE` — `"cpu"`, `"cuda"`, or `"rocm"` (default: `"cpu"`)
//! - `ANVILML_MOCK_VRAM_MIB`   — total/free VRAM in MiB (default: `8192`)
//! - `ANVILML_MOCK_GFX_ARCH`  — graphics architecture string (default: `"gfx1100"`)

use anvilml_core::types::{DeviceType, GpuDevice};
use anvilml_core::AnvilError;

use crate::DeviceDetector;

/// A mock detector that returns a single deterministic GPU device.
///
/// The device properties are controlled entirely by environment variables,
/// making CI runs fully hermetic without requiring real hardware.
#[derive(Debug, Clone)]
pub struct MockDetector;

impl DeviceDetector for MockDetector {
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        let device_type = match std::env::var("ANVILML_MOCK_DEVICE_TYPE")
            .ok()
            .as_deref()
        {
            Some("cuda") | Some("CUDA") => DeviceType::Cuda,
            Some("rocm") | Some("ROCm") | Some("ROCM") => DeviceType::Rocm,
            _ => DeviceType::Cpu,
        };

        let vram_total_mib = std::env::var("ANVILML_MOCK_VRAM_MIB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8192);

        let gfx_arch = std::env::var("ANVILML_MOCK_GFX_ARCH").unwrap_or_else(|_| "gfx1100".into());

        let name = match device_type {
            DeviceType::Cpu => "Mock CPU".to_string(),
            DeviceType::Cuda => "Mock CUDA GPU".to_string(),
            DeviceType::Rocm => format!("Mock ROCm GPU ({gfx_arch})"),
        };

        Ok(vec![GpuDevice {
            index: 0,
            name,
            device_type,
            vram_total_mib,
            vram_free_mib: vram_total_mib,
            driver_version: "mock".into(),
        }])
    }

    fn refresh_vram(&self, _device_index: u32) -> Result<(u32, u32), AnvilError> {
        let vram = std::env::var("ANVILML_MOCK_VRAM_MIB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8192);
        Ok((0, vram))
    }
}

// ---------------------------------------------------------------------------
// Tests — gated by mock-hardware feature; serialized to avoid env-var pollution
// ---------------------------------------------------------------------------

#[cfg(all(test, feature = "mock-hardware"))]
mod tests {
    use super::*;

    /// Helper: remove all mock env vars so the detector hits its defaults.
    fn clean_env() {
        std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE");
        std::env::remove_var("ANVILML_MOCK_VRAM_MIB");
        std::env::remove_var("ANVILML_MOCK_GFX_ARCH");
    }

    // Fixture 1: default env vars → DeviceType::Cpu with 8192 MiB VRAM
    #[serial_test::serial]
    #[test]
    fn mock_detect_defaults_to_cpu() {
        clean_env();

        let detector = MockDetector;
        let devices = detector.detect().unwrap();
        assert_eq!(devices.len(), 1);

        let dev = &devices[0];
        assert!(matches!(dev.device_type, DeviceType::Cpu));
        assert_eq!(dev.name, "Mock CPU");
        assert_eq!(dev.vram_total_mib, 8192);
        assert_eq!(dev.vram_free_mib, 8192);
        assert_eq!(dev.driver_version, "mock");

        clean_env();
    }

    // Fixture 2: ANVILML_MOCK_DEVICE_TYPE=cuda → DeviceType::Cuda with correct VRAM
    #[serial_test::serial]
    #[test]
    fn mock_detect_cuda() {
        clean_env();
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cuda");
        std::env::set_var("ANVILML_MOCK_VRAM_MIB", "16384");

        let detector = MockDetector;
        let devices = detector.detect().unwrap();
        assert_eq!(devices.len(), 1);

        let dev = &devices[0];
        assert!(matches!(dev.device_type, DeviceType::Cuda));
        assert_eq!(dev.name, "Mock CUDA GPU");
        assert_eq!(dev.vram_total_mib, 16384);
        assert_eq!(dev.vram_free_mib, 16384);
        assert_eq!(dev.driver_version, "mock");

        clean_env();
    }

    // Fixture 3: ANVILML_MOCK_DEVICE_TYPE=rocm + custom VRAM → DeviceType::Rocm
    #[serial_test::serial]
    #[test]
    fn mock_detect_rocm() {
        clean_env();
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "rocm");
        std::env::set_var("ANVILML_MOCK_VRAM_MIB", "32768");
        std::env::set_var("ANVILML_MOCK_GFX_ARCH", "gfx1030");

        let detector = MockDetector;
        let devices = detector.detect().unwrap();
        assert_eq!(devices.len(), 1);

        let dev = &devices[0];
        assert!(matches!(dev.device_type, DeviceType::Rocm));
        assert_eq!(dev.name, "Mock ROCm GPU (gfx1030)");
        assert_eq!(dev.vram_total_mib, 32768);
        assert_eq!(dev.vram_free_mib, 32768);
        assert_eq!(dev.driver_version, "mock");

        clean_env();
    }

    // Fixture 4: refresh_vram returns correct values
    #[serial_test::serial]
    #[test]
    fn mock_refresh_vram() {
        clean_env();
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cuda");
        std::env::set_var("ANVILML_MOCK_VRAM_MIB", "24576");

        let detector = MockDetector;
        let (used, total) = detector.refresh_vram(0).unwrap();
        assert_eq!(used, 0);
        assert_eq!(total, 24576);

        clean_env();
    }
}
