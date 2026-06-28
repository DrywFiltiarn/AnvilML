/// Mock hardware detector — env-var driven synthetic GPU device.
///
/// Reads three environment variables to construct a single synthetic
/// `GpuDevice`:
///
/// * `ANVILML_MOCK_DEVICE_TYPE` — `"cuda"`, `"rocm"`, or any other value
///   falls back to CPU (per the design doc's three-backend constraint).
///   Default: `"cpu"`.
/// * `ANVILML_MOCK_VRAM_MIB` — total/free VRAM in mebibytes. Default: `8192`.
/// * `ANVILML_MOCK_DEVICE_NAME` — human-readable device name. Default:
///   `"Mock GPU"`.
///
/// This detector is the one every CI job exercises when built with
/// `--features mock-hardware`. It never performs real I/O.
pub struct MockDetector;

use crate::detect::DeviceDetector;
use anvilml_core::{
    AnvilError, CapabilitySource, DeviceType, EnumerationSource, GpuDevice, InferenceCaps,
};

impl DeviceDetector for MockDetector {
    /// Enumerate compute devices from mock environment variables.
    ///
    /// Returns exactly one synthetic `GpuDevice` whose fields are derived
    /// from the three `ANVILML_MOCK_*` environment variables. Never returns
    /// `Err` and never panics.
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        // Read env vars with their design-doc defaults.
        let device_type =
            std::env::var("ANVILML_MOCK_DEVICE_TYPE").unwrap_or_else(|_| "cpu".into());
        let vram = std::env::var("ANVILML_MOCK_VRAM_MIB")
            .unwrap_or_else(|_| "8192".into())
            .parse::<u32>()
            .unwrap_or(8192);
        let name = std::env::var("ANVILML_MOCK_DEVICE_NAME").unwrap_or_else(|_| "Mock GPU".into());

        // Parse device_type: only "cuda" and "rocm" map to GPU backends;
        // any unrecognized value falls back to CPU to honour the design
        // doc's three-backend constraint (cuda, rocm, or cpu).
        let device_type = match device_type.as_str() {
            "cuda" => DeviceType::Cuda,
            "rocm" => DeviceType::Rocm,
            _ => DeviceType::Cpu,
        };

        let device = GpuDevice {
            index: 0,
            name,
            device_type,
            vram_total_mib: vram,
            vram_free_mib: vram,
            driver_version: "mock".into(),
            pci_vendor_id: 0,
            pci_device_id: 0,
            arch: None,
            caps: InferenceCaps::default(),
            enumeration_source: EnumerationSource::Mock,
            capabilities_source: CapabilitySource::Fallback,
        };

        Ok(vec![device])
    }

    /// Refresh VRAM for a mock device.
    ///
    /// Returns `(vram_mib, vram_mib)` — both total and free equal the
    /// `ANVILML_MOCK_VRAM_MIB` env value (default 8192). The `_index`
    /// parameter is unused because mock mode has no per-device VRAM query.
    fn refresh_vram(&self, _index: u32) -> Result<(u32, u32), AnvilError> {
        // _index is unused — mock mode has no per-device VRAM query;
        // there is exactly one synthetic device.
        let vram = std::env::var("ANVILML_MOCK_VRAM_MIB")
            .unwrap_or_else(|_| "8192".into())
            .parse::<u32>()
            .unwrap_or(8192);
        Ok((vram, vram))
    }
}
