/// A zero-sized detector that synthesises a single CPU device.
///
/// This detector uses the `sysinfo` crate to read host-level information
/// (OS version, CPU brand, total RAM) and constructs a synthetic
/// `GpuDevice` representing the CPU. CPUs have no VRAM, no driver
/// version, and no PCI identifiers — all such fields are set to their
/// neutral defaults.
///
/// This is the final fallback in the detection pipeline: when no GPU
/// detection backend (Vulkan, DXGI, sysfs) returns any devices, this
/// detector always produces exactly one CPU device so the system
/// remains usable.
pub struct CpuDetector;

use anvilml_core::{CapabilitySource, DeviceType, EnumerationSource, GpuDevice, InferenceCaps};
use sysinfo::System;

use crate::DeviceDetector;

impl CpuDetector {
    /// Construct a new `CpuDetector`.
    ///
    /// This is a zero-sized unit struct — no allocation or state is required.
    pub const fn new() -> Self {
        CpuDetector
    }
}

impl Default for CpuDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceDetector for CpuDetector {
    /// Enumerate available devices.
    ///
    /// Always returns exactly one `GpuDevice` representing the host CPU.
    /// The device uses `DeviceType::Cpu` and `EnumerationSource::Override`
    /// to distinguish it from real GPU devices detected by other backends.
    ///
    /// # Errors
    ///
    /// This method never returns an error — it synthesises its output from
    /// in-memory sysinfo data which is always available.
    fn detect(&self) -> Result<Vec<GpuDevice>, anvilml_core::AnvilError> {
        // Read host-level information from sysinfo.
        // System::new_all() collects CPU, memory, and process info in one call.
        let mut sys = System::new_all();
        sys.refresh_all();

        // Build the CPU device name. The plan specifies "CPU (synthetic)" to
        // distinguish this from real hardware — it is not a real GPU.
        let device_name = "CPU (synthetic)";

        // Get the OS version string from sysinfo. If unavailable, fall back
        // to a generic string. This is a best-effort read — sysinfo may
        // return None on platforms where OS version is not exposed.
        let os_version = System::long_os_version()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Unknown OS".to_string());

        // Get the CPU brand from the first available CPU. If the system has
        // no CPUs (impossible in practice), fall back to "Unknown CPU".
        let cpu_brand = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "Unknown CPU".to_string());

        // Convert total system RAM from bytes to mebibabytes (MiB).
        // sysinfo reports memory in bytes; we divide by 1024*1024.
        let ram_total_mib = sys.total_memory() / (1024 * 1024);

        // Log the synthesised CPU device at INFO level per the mandatory
        // "each detected device" logging convention.
        tracing::info!(
            index = 0u32,
            name = device_name,
            device_type = "cpu",
            vram_total_mib = 0u32,
            fp8 = false,
            "cpu device synthesised"
        );

        // Build the single CPU device. All VRAM-related fields are zero
        // because CPUs do not have dedicated video memory. PCI identifiers
        // are zero since the CPU is synthesised, not enumerated from hardware.
        //
        // EnumerationSource::Override is used because this device is not
        // detected from a real hardware interface — it is a synthetic
        // fallback that always exists.
        let device = GpuDevice {
            index: 0,
            name: device_name.to_string(),
            device_type: DeviceType::Cpu,
            vram_total_mib: 0,
            vram_free_mib: 0,
            driver_version: "n/a".to_string(),
            pci_vendor_id: 0,
            pci_device_id: 0,
            arch: None,
            caps: InferenceCaps::default(),
            enumeration_source: EnumerationSource::Override,
            capabilities_source: CapabilitySource::Fallback,
        };

        // Log the host info at DEBUG level for diagnostic purposes.
        tracing::debug!(
            os = %os_version,
            cpu = %cpu_brand,
            ram_total_mib = ram_total_mib,
            "cpu host info read"
        );

        Ok(vec![device])
    }

    /// Refresh the VRAM for the device at `index`.
    ///
    /// CPUs have no VRAM, so this always returns `(0, 0)`.
    ///
    /// # Errors
    ///
    /// This method never returns an error.
    fn refresh_vram(&self, _index: u32) -> Result<(u32, u32), anvilml_core::AnvilError> {
        tracing::debug!(index = _index, "refresh_vram returns (0,0) for CPU");
        Ok((0, 0))
    }
}
