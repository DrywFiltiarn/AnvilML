//! `DeviceDetector` trait — the shared contract for all GPU/CPU detectors.
//!
//! Every concrete detector (CPU, mock, Vulkan, DXGI, sysfs) implements this trait.
//! Implementations must never panic on missing drivers or hardware — they return
//! `Ok(vec![])` on detection failure per §6.2 of the design.

use anvilml_core::{
    AnvilError, CapabilitySource, DeviceType, EnumerationSource, GpuDevice, HardwareInfo, HostInfo,
    InferenceCaps, ServerConfig,
};

/// Trait for detecting and refreshing GPU device information.
///
/// Every concrete detector (CPU, mock, Vulkan, DXGI, sysfs) implements this trait.
/// Implementations must never panic on missing drivers or hardware — they return
/// `Ok(vec![])` on detection failure per §6.2 of the design.
pub trait DeviceDetector: Send + Sync {
    /// Enumerate all compute devices on the host.
    ///
    /// Returns a vector of detected `GpuDevice` structs. If no devices are found,
    /// returns `Ok(vec![])` — never an error or a panic. The caller (Phase 5's
    /// `detect_all_devices`) appends a CPU fallback device if the result is empty.
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>;

    /// Refresh VRAM totals for a device by its index.
    ///
    /// Returns `(total_mib, free_mib)` — the total and free VRAM in mebibytes for
    /// the device at the given `index`. This is called at dispatch time to get a
    /// current snapshot rather than relying on the stale value from `detect()`.
    fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>;
}

/// Enumerate all compute devices on the host.
///
/// This is the single entry point for hardware detection, called once at server startup.
/// It follows a priority chain defined in `ANVILML_DESIGN.md §6.4`:
/// 1. Hardware override (config `[hardware_override]`) — short-circuits all other paths.
///    2–6. Mock detector, Vulkan detector, platform fallback, CPU fallback, assembly.
///
/// Steps 2–6 are deferred to `P5-A2` and `P5-A3`. When `cfg.hardware_override` is
/// `None`, this function returns `Err` to make the incomplete state explicit and
/// testable — the full chain is not yet implemented.
///
/// # Override short-circuit
///
/// When `cfg.hardware_override` is `Some`, this function synthesizes exactly one
/// `GpuDevice` from the override config and returns `Ok(HardwareInfo{...})`
/// immediately, skipping all other detectors. This satisfies the design requirement
/// that override always wins unconditionally before any detector runs.
///
/// # Arguments
///
/// * `cfg` — The server configuration, which may contain a `hardware_override` section.
///
/// # Returns
///
/// * `Ok(HardwareInfo)` — A hardware snapshot containing at least one device. When the
///   override is present, exactly one override-synthesized device is returned.
/// * `Err(AnvilError::Internal)` — The full detection chain is not yet implemented.
///   This error is expected until `P5-A2` extends this function with the mock/Vulkan/
///   fallback/CPU chain.
pub async fn detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError> {
    // Step 1: Hardware override — unconditional short-circuit per §6.4 priority order.
    // This always wins before any detector runs, satisfying the design requirement.
    if let Some(override_cfg) = &cfg.hardware_override {
        // Parse the override's device_type string into the DeviceType enum.
        // Unrecognized values default to Cpu with a warning so the operator knows.
        let device_type = match override_cfg.device_type.as_str() {
            "cuda" => DeviceType::Cuda,
            "rocm" => DeviceType::Rocm,
            "cpu" => DeviceType::Cpu,
            other => {
                // Unrecognized device_type falls back to Cpu — the safest default
                // since CPU is always available on every host.
                tracing::warn!(
                    device_type = %other,
                    "unrecognized hardware_override device_type, defaulting to Cpu"
                );
                DeviceType::Cpu
            }
        };

        // Derive a human-readable device name from the parsed type.
        let name = match device_type {
            DeviceType::Cuda => "CUDA",
            DeviceType::Rocm => "ROCm",
            DeviceType::Cpu => "CPU",
        };

        // Build a single override-synthesized GpuDevice.
        // This satisfies the design's requirement that override always wins
        // unconditionally before any detector runs. defers_to: P5-A2 — the
        // full detection chain (mock/Vulkan/fallback/CPU) is implemented by P5-A2.
        let device = GpuDevice {
            index: 0,
            name: name.to_string(),
            device_type,
            vram_total_mib: override_cfg.vram_total_mib,
            vram_free_mib: override_cfg.vram_total_mib, // free = total for override
            driver_version: "override".to_string(),
            pci_vendor_id: 0,
            pci_device_id: 0,
            arch: None,
            caps: InferenceCaps::default(),
            enumeration_source: EnumerationSource::Override,
            capabilities_source: CapabilitySource::Fallback,
        };

        // Build the host info from environment variables.
        // HOSTNAME is the standard Unix env var; COMPUTERNAME is the Windows equivalent.
        let hostname = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "unknown".into());
        let os = std::env::consts::OS.to_string();

        let host = HostInfo { hostname, os };

        // With a single device, the union of inference caps equals that device's caps.
        // Since the override device has default (all-false) caps, the union is also default.
        let inference_caps = InferenceCaps::default();

        let hardware_info = HardwareInfo {
            host,
            gpus: vec![device],
            inference_caps,
        };

        tracing::info!(
            device_type = ?device_type,
            vram_mib = override_cfg.vram_total_mib,
            "hardware override: returning synthesized device"
        );

        return Ok(hardware_info);
    }

    // Override is absent — the full detection chain is not yet implemented.
    // P5-A2 will extend this function with the mock/Vulkan/fallback/CPU chain.
    // Returning Err with a clear message makes the incomplete state explicit and testable.
    Err(AnvilError::Internal(
        "detect_all_devices chain not yet implemented — mock/Vulkan/fallback/CPU chain deferred to P5-A2".to_string(),
    ))
}
