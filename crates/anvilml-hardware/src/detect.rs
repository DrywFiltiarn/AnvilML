//! `DeviceDetector` trait — the shared contract for all GPU/CPU detectors.
//!
//! Every concrete detector (CPU, mock, Vulkan, DXGI, sysfs) implements this trait.
//! Implementations must never panic on missing drivers or hardware — they return
//! `Ok(vec![])` on detection failure per §6.2 of the design.

use anvilml_core::{
    AnvilError, CapabilitySource, DeviceType, EnumerationSource, GpuDevice, HardwareInfo, HostInfo,
    InferenceCaps, ServerConfig,
};

#[cfg(feature = "mock-hardware")]
use crate::MockDetector;

#[cfg(not(feature = "mock-hardware"))]
use crate::VulkanDetector;

// Platform-specific fallback detectors — only needed when mock-hardware is absent.
#[cfg(all(not(feature = "mock-hardware"), target_os = "windows"))]
use crate::DxgiDetector;

#[cfg(all(not(feature = "mock-hardware"), target_os = "linux"))]
use crate::SysfsPciDetector;

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
/// 2. Mock detector (when `mock-hardware` cargo feature is active).
/// 3. Vulkan detector (headless GPU enumeration).
/// 4. Platform-specific fallback (DXGI on Windows, sysfs PCI on Linux).
///    5–6. CPU fallback and final assembly — deferred to `P5-A3`.
///
/// Steps 5–6 are deferred to `P5-A3`. The function always returns `Ok(HardwareInfo)`
/// (never `Err`), constructing a partial `HardwareInfo` with the detected GPUs and
/// default `InferenceCaps` — the CPU append and caps union are deferred to P5-A3.
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
/// * `Ok(HardwareInfo)` — A hardware snapshot containing detected GPUs. When the
///   override is present, exactly one override-synthesized device is returned.
///   Otherwise, devices from the mock/Vulkan/fallback chain are returned.
pub async fn detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError> {
    // Construct host info once, before both the override and mock/real branches.
    // HOSTNAME is the standard Unix env var; COMPUTERNAME is the Windows equivalent.
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".into());
    let os = std::env::consts::OS.to_string();
    let host = HostInfo { hostname, os };

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
        // unconditionally before any detector runs. defers_to: P5-A3 — the
        // CPU append and caps union (full HardwareInfo assembly) are implemented
        // by P5-A3; this is a partial result with default inference_caps.
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

    // Steps 2–4: Mock-vs-real branch + Vulkan fallback chain.
    // Override is absent — proceed to the priority chain.
    // Steps 5–6 (CPU append, caps union) deferred to P5-A3.

    #[cfg(feature = "mock-hardware")]
    {
        // Mock and real detection are mutually exclusive per build.
        // When mock-hardware is compiled in, use MockDetector exclusively.
        let detector = MockDetector;
        let gpus = detector.detect()?;
        tracing::debug!(
            device_count = gpus.len(),
            "mock-hardware feature: returning mock-detected devices"
        );
        // Partial HardwareInfo — CPU append and caps union deferred to P5-A3.
        Ok(HardwareInfo {
            host,
            gpus,
            inference_caps: InferenceCaps::default(),
        })
    }

    #[cfg(not(feature = "mock-hardware"))]
    {
        // Primary real-hardware path: Vulkan enumeration.
        // Vulkan is the preferred detector because it provides the most
        // accurate device information (PCI IDs, driver version, etc.).
        let detector = VulkanDetector;
        let gpus = detector.detect()?;

        if gpus.is_empty() {
            // Vulkan returned no devices — try platform-specific fallback.
            // This handles cases where the Vulkan loader is absent but
            // the GPU is still present (e.g. missing drivers).
            tracing::debug!("Vulkan returned empty, trying platform fallback");

            // Platform-specific fallback — cfg-gated by target OS.
            #[cfg(target_os = "windows")]
            {
                // DXGI is the Windows equivalent of Vulkan enumeration.
                // It uses COM-based DXGI factory to enumerate adapters.
                let detector = DxgiDetector;
                let gpus = detector.detect()?;
                if !gpus.is_empty() {
                    tracing::debug!(
                        device_count = gpus.len(),
                        "platform fallback (DXGI) returned devices"
                    );
                    // Partial HardwareInfo — CPU append deferred to P5-A3.
                    return Ok(HardwareInfo {
                        host,
                        gpus,
                        inference_caps: InferenceCaps::default(),
                    });
                }
            }

            #[cfg(target_os = "linux")]
            {
                // sysfs PCI enumeration reads /sys/bus/pci/devices/ for
                // display controllers (class 0x03) and maps vendor IDs.
                let detector = SysfsPciDetector;
                let gpus = detector.detect()?;
                if !gpus.is_empty() {
                    tracing::debug!(
                        device_count = gpus.len(),
                        "platform fallback (sysfs) returned devices"
                    );
                    // Partial HardwareInfo — CPU append deferred to P5-A3.
                    return Ok(HardwareInfo {
                        host,
                        gpus,
                        inference_caps: InferenceCaps::default(),
                    });
                }
            }

            // Neither Vulkan nor platform fallback found devices.
            // Return empty Vec<GpuDevice> — P5-A3 will append the CPU device.
            tracing::debug!("Vulkan and platform fallback both returned empty");
            return Ok(HardwareInfo {
                host,
                gpus: vec![],
                inference_caps: InferenceCaps::default(),
            });
        }

        tracing::debug!(
            device_count = gpus.len(),
            "Vulkan detection returned devices"
        );
        // Partial HardwareInfo — CPU append deferred to P5-A3.
        Ok(HardwareInfo {
            host,
            gpus,
            inference_caps: InferenceCaps::default(),
        })
    }
}
