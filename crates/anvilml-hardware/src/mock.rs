/// Mock hardware detector for CI and isolated test environments.
///
/// This detector synthesises a single `GpuDevice` from environment variables
/// rather than reading real hardware. The device type, VRAM, and name are
/// driven by `ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, and
/// `ANVILML_MOCK_DEVICE_NAME` respectively.
///
/// When the environment variable values are valid (`cuda`, `rocm`, or `cpu`),
/// the detector returns one synthetic device. When the value is invalid,
/// the detector returns an empty list (graceful fallback — never an error).
///
/// This detector is only compiled when the `mock-hardware` cargo feature
/// is active. It is the primary detection path in CI pipelines and
/// isolated test environments where real GPU hardware is unavailable.
#[cfg(feature = "mock-hardware")]
pub struct MockDetector;

use anvilml_core::{CapabilitySource, DeviceType, EnumerationSource, GpuDevice, InferenceCaps};

use crate::DeviceDetector;

impl MockDetector {
    /// Construct a new `MockDetector`.
    ///
    /// This is a zero-sized unit struct — no allocation or state is required.
    #[cfg(feature = "mock-hardware")]
    pub const fn new() -> Self {
        MockDetector
    }
}

impl Default for MockDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "mock-hardware")]
impl DeviceDetector for MockDetector {
    /// Enumerate available devices.
    ///
    /// Reads `ANVILML_MOCK_DEVICE_TYPE` (default `"cpu"`),
    /// `ANVILML_MOCK_VRAM_MIB` (default `8192`), and
    /// `ANVILML_MOCK_DEVICE_NAME` (default `"Mock GPU"`) from the
    /// process environment, then constructs a single synthetic
    /// `GpuDevice` with `enumeration_source = Mock`.
    ///
    /// If the device type value is not one of `"cuda"`, `"rocm"`, or
    /// `"cpu"`, the function returns an empty list as a graceful
    /// fallback — the caller should fall through to real detection.
    ///
    /// # Errors
    ///
    /// This method never returns an error. Invalid environment values
    /// produce an empty device list rather than an error.
    fn detect(&self) -> Result<Vec<GpuDevice>, anvilml_core::AnvilError> {
        // Read the device type from the environment variable.
        // Default to "cpu" if the variable is unset — CPU is the safest
        // fallback that always works regardless of GPU hardware.
        let device_type_str =
            std::env::var("ANVILML_MOCK_DEVICE_TYPE").unwrap_or_else(|_| "cpu".to_string());

        // Map the device type string to a DeviceType variant.
        // If the string is not one of the recognised values, return
        // an empty list — this signals to the caller that mock detection
        // should be skipped in favour of real hardware detection.
        let device_type = match device_type_str.as_str() {
            "cuda" => DeviceType::Cuda,
            "rocm" => DeviceType::Rocm,
            "cpu" => DeviceType::Cpu,
            _ => {
                // Invalid device type — return empty list so the caller
                // falls through to real detection paths.
                tracing::debug!(
                    device_type = %device_type_str,
                    "mock device type invalid, returning empty list"
                );
                return Ok(vec![]);
            }
        };

        // Read VRAM from the environment variable, defaulting to 8192 MiB.
        // This is a reasonable default for a mid-range GPU.
        let vram_total_mib = std::env::var("ANVILML_MOCK_VRAM_MIB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8192);

        // Read the device name, defaulting to "Mock GPU".
        let device_name =
            std::env::var("ANVILML_MOCK_DEVICE_NAME").unwrap_or_else(|_| "Mock GPU".to_string());

        // Build the single mock device. All PCI identifiers are zero
        // because this device is not a real piece of hardware.
        // The enumeration_source is Mock to distinguish it from
        // devices detected from real hardware interfaces.
        let device = GpuDevice {
            index: 0,
            name: device_name,
            db_name: None,
            device_type,
            vram_total_mib,
            vram_free_mib: vram_total_mib, // mock has no live VRAM tracking
            driver_version: "mock".to_string(),
            pci_vendor_id: 0,
            pci_device_id: 0,
            arch: None,
            caps: InferenceCaps::default(),
            enumeration_source: EnumerationSource::Mock,
            capabilities_source: CapabilitySource::Fallback,
        };

        // Log the detected device at INFO level per the mandatory
        // "each detected device" logging convention.
        tracing::info!(
            index = 0u32,
            name = %device.name,
            device_type = ?device.device_type,
            vram_total_mib = vram_total_mib,
            fp8 = false,
            "mock device detected"
        );

        Ok(vec![device])
    }

    /// Refresh the VRAM for the device at `index`.
    ///
    /// Mock devices have no live VRAM tracking, so this always returns
    /// `(0, 0)`.
    ///
    /// # Errors
    ///
    /// This method never returns an error.
    fn refresh_vram(&self, _index: u32) -> Result<(u32, u32), anvilml_core::AnvilError> {
        tracing::debug!(index = _index, "refresh_vram returns (0,0) for mock");
        Ok((0, 0))
    }
}
