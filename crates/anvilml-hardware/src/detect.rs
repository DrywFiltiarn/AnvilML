/// The `detect_all_devices` orchestration function.
///
/// This module contains the full hardware detection pipeline that
/// enumerates GPUs and CPUs from the host machine, resolves device
/// capabilities from a PCI-ID lookup table, enriches them from the
/// `device_capabilities` SQLite table, and assembles a
/// `HardwareInfo` snapshot.
use anvilml_core::{
    AnvilError, CapabilitySource, DeviceType, EnumerationSource, GpuDevice, HardwareInfo, HostInfo,
    InferenceCaps, ServerConfig,
};
use anvilml_registry::DeviceCapabilityStore;
use sqlx::SqlitePool;
use tracing::instrument;

use crate::{resolve_caps_from_row, CpuDetector, DeviceDetector, VulkanDetector};

/// Orchestrate the full hardware detection pipeline.
///
/// This function runs through a priority chain to detect all available
/// compute devices on the host:
///
/// 1. **Hardware override** — if `cfg.hardware_override` is set, construct
///    a synthetic device from the override config (skips all detection).
/// 2. **Mock detection** — if the `mock-hardware` feature is active,
///    attempt mock detection from environment variables.
/// 3. **Vulkan detection** — enumerate GPUs via the Vulkan loader.
/// 4. **Platform fallbacks** — on Windows, use DXGI; on Unix, use sysfs.
/// 5. **CPU fallback** — always synthesise one CPU device.
///
/// After detection, the function resolves per-device inference capabilities
/// from the PCI-ID device table, enriches each device from the
/// `device_capabilities` SQLite table, populates `HostInfo` from `sysinfo`,
/// and computes the union of all GPU capabilities as `inference_caps`.
///
/// The `pool` parameter is used for device capability enrichment via
/// `DeviceCapabilityStore`.
///
/// # Arguments
///
/// * `cfg` — Server configuration, which may include a hardware override.
/// * `pool` — SQLite connection pool for device capability enrichment.
///
/// # Returns
///
/// A `HardwareInfo` snapshot containing host information, all detected
/// devices, and the union of their inference capabilities.
///
/// # Errors
///
/// This function never returns `Err` under normal circumstances.
/// Detection failures are treated as "no device detected" rather than
/// hard errors — the CPU fallback always produces at least one device.
#[instrument(name = "detect_all_devices", skip(cfg, pool))]
pub async fn detect_all_devices(
    cfg: &ServerConfig,
    pool: &SqlitePool,
) -> Result<HardwareInfo, AnvilError> {
    let mut devices: Vec<GpuDevice> = Vec::new();

    // ── Step a: Hardware override check ──────────────────────────────
    // If a hardware override is configured, use it directly and skip
    // all real detection paths. This is the highest-priority path.
    if let Some(override_cfg) = &cfg.hardware_override {
        // Map the override device type string to a DeviceType variant.
        // This mirrors the same mapping used by MockDetector and the
        // real detectors for consistency.
        let device_type = match override_cfg.device_type.as_str() {
            "cuda" => DeviceType::Cuda,
            "rocm" => DeviceType::Rocm,
            "cpu" => DeviceType::Cpu,
            other => {
                // Unrecognised device type in override — fall through to
                // normal detection rather than crashing.
                tracing::warn!(
                    device_type = %other,
                    "hardware override device_type unrecognised, falling through to normal detection"
                );
                DeviceType::Cpu
            }
        };

        // Build a synthetic device from the override config.
        // PCI IDs are zero because this is a synthetic device, not
        // real hardware. The enumeration_source is Override to mark
        // that the device was forced by configuration.
        let mut device = GpuDevice {
            index: 0,
            name: format!(
                "Override {} ({})",
                device_type_str(&device_type),
                override_cfg.device_type
            ),
            db_name: None,
            device_type,
            vram_total_mib: override_cfg.vram_total_mib,
            vram_free_mib: override_cfg.vram_total_mib, // no live VRAM for override
            driver_version: "override".to_string(),
            pci_vendor_id: 0,
            pci_device_id: 0,
            arch: None,
            caps: InferenceCaps::default(),
            enumeration_source: EnumerationSource::Override,
            capabilities_source: CapabilitySource::Fallback,
        };

        // Resolve capabilities via the PCI-ID table. Since the override
        // device has zero PCI IDs, no match will be found and caps
        // remain at defaults. This is expected — override devices are
        // synthetic and don't appear in the device database.
        resolve_caps_from_row(&mut device, None);

        // Log the override device at INFO level per the mandatory
        // "each detected device" logging convention.
        tracing::info!(
            index = 0u32,
            name = %device.name,
            device_type = ?device.device_type,
            vram_total_mib = device.vram_total_mib,
            fp8 = device.caps.fp8,
            "hardware override device"
        );

        devices.push(device);

        // Override takes priority — skip mock, Vulkan, and platform
        // detection. Proceed directly to CPU fallback and capability
        // resolution.
        tracing::debug!("hardware override active, skipping real detection");
    } else {
        // ── Step b: Mock detector ──────────────────────────────────────
        // When the mock-hardware feature is active, try mock detection
        // first. If it returns devices, use them; otherwise fall through
        // to real detection paths.
        #[cfg(feature = "mock-hardware")]
        {
            let mock = crate::MockDetector::new();
            let mock_devices = mock.detect()?;
            if !mock_devices.is_empty() {
                devices = mock_devices;
                tracing::debug!("mock detection returned devices, skipping real detection");
            } else {
                tracing::debug!("mock detection returned empty, falling through to real detection");
            }
        }

        // ── Step c: Vulkan detection ───────────────────────────────────
        // If no devices yet (no override, and mock returned empty),
        // try Vulkan detection. Vulkan is the primary GPU detection
        // path on both Linux and Windows.
        if devices.is_empty() {
            let vulkan = VulkanDetector::new();
            let vulkan_devices = vulkan.detect()?;
            if !vulkan_devices.is_empty() {
                devices = vulkan_devices;
            } else {
                // Vulkan returned no devices — try platform-specific
                // fallbacks. On Windows, use DXGI; on Unix, use sysfs.
                // These are lower-fidelity detectors that work when
                // Vulkan is unavailable.
                #[cfg(windows)]
                {
                    if devices.is_empty() {
                        let dxgi = crate::DxgiDetector::new();
                        let dxgi_devices = dxgi.detect()?;
                        if !dxgi_devices.is_empty() {
                            devices = dxgi_devices;
                        }
                    }
                }

                #[cfg(unix)]
                {
                    if devices.is_empty() {
                        let sysfs = crate::SysfsPciDetector::new();
                        let sysfs_devices = sysfs.detect()?;
                        if !sysfs_devices.is_empty() {
                            devices = sysfs_devices;
                        }
                    }
                }
            }
        }
    }

    // ── Step d: CPU fallback ─────────────────────────────────────────
    // Always instantiate the CPU detector and append its device to the
    // list. This guarantees at least one device is always returned,
    // even when no GPU is detected. The CPU device is appended after
    // GPU devices so GPU devices get lower indices.
    let cpu_detector = CpuDetector::new();
    let cpu_devices = cpu_detector.detect()?;

    // Merge CPU devices into the list. Since CpuDetector always returns
    // exactly one device, we append it at the end. The CPU device
    // gets an index equal to the total number of GPU devices, so it
    // is always the last device in the list.
    let cpu_start_index = devices.len() as u32;
    for mut cpu_dev in cpu_devices {
        cpu_dev.index = cpu_start_index;
        devices.push(cpu_dev);
    }

    // ── Step e: Resolve capabilities ─────────────────────────────────
    // For each GPU device (not the CPU device), look up the PCI-ID
    // table to populate arch, caps, and canonical name. CPU devices
    // are skipped because they have zero PCI IDs and won't match
    // any entry in the device database.
    for dev in devices.iter_mut() {
        // Skip CPU devices — they have no PCI IDs and won't match
        // any entry in the device database.
        if dev.device_type == DeviceType::Cpu {
            continue;
        }

        // Look up the device in the PCI-ID capability table.
        // If a match is found, arch, caps, and name are populated.
        // If no match is found, the device is left unchanged.
        resolve_caps_from_row(dev, None);
    }

    // ── Step e2: SQLite capability enrichment ────────────────────────
    // Look up each non-CPU device in the device_capabilities table
    // to populate full capability data (arch, all six inference caps,
    // db_name) from the seeded database. This is a non-fatal lookup —
    // if the table doesn't exist or a query fails, the device retains
    // the capabilities resolved in step e from the PCI-ID table.
    let store = DeviceCapabilityStore::new(pool.clone()).await;

    for dev in devices.iter_mut() {
        // Skip CPU devices — they have no real PCI IDs and won't
        // match any entry in the device_capabilities table.
        if dev.device_type == DeviceType::Cpu {
            continue;
        }

        let vendor_id = dev.pci_vendor_id;
        let device_id = dev.pci_device_id;

        // Look up the device in the seeded capability table.
        // Ok(None) means the device isn't in the seed data —
        // the step-e resolution from DEVICE_DB is the final word.
        // Err is non-fatal: a DB query failure must not abort
        // hardware detection. The device keeps its step-e caps.
        match store.get(vendor_id, device_id).await {
            Ok(Some(row)) => {
                // Overwrite arch, all six inference capability fields,
                // capabilities_source, and db_name from the DB row.
                // Never overwrite dev.name — the enumerator-reported
                // name is the specific installed SKU and must be preserved.
                dev.arch = Some(row.arch.clone());
                dev.caps.fp32 = row.fp32;
                dev.caps.fp16 = row.fp16;
                dev.caps.bf16 = row.bf16;
                dev.caps.fp8 = row.fp8;
                dev.caps.fp4 = row.fp4;
                dev.caps.flash_attention = row.flash_attention;
                dev.capabilities_source = CapabilitySource::DeviceTable;
                dev.db_name = Some(row.name.clone());

                tracing::debug!(
                    vendor_id = vendor_id,
                    device_id = device_id,
                    arch = %row.arch,
                    source = "sqlite",
                    "device capability enriched from device_capabilities table"
                );
            }
            Ok(None) => {
                // No matching row in the device_capabilities table.
                // The device retains its step-e resolution from DEVICE_DB.
                tracing::warn!(
                    vendor_id = vendor_id,
                    device_id = device_id,
                    name = %dev.name,
                    "device not found in device_capabilities table"
                );
            }
            Err(e) => {
                // DB query failed — non-fatal. Log the error and continue
                // with step-e resolved capabilities. This prevents a
                // corrupted or missing seed table from blocking hardware
                // detection entirely.
                tracing::error!(
                    vendor_id = vendor_id,
                    device_id = device_id,
                    error = %e,
                    "device_capabilities lookup failed, using step-e resolution"
                );
            }
        }
    }

    // ── Step f: Populate HostInfo ────────────────────────────────────
    // Read host-level information using sysinfo — the same approach
    // as CpuDetector::detect(). This gives us OS version, CPU brand,
    // and total RAM for the HardwareInfo snapshot.
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    // Build the OS version string. If unavailable, fall back to a
    // generic string. This is a best-effort read — sysinfo may
    // return None on platforms where OS version is not exposed.
    let os_version = sysinfo::System::long_os_version()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Unknown OS".to_string());

    // Build the CPU brand string from the first available CPU.
    // If the system has no CPUs (impossible in practice), fall back.
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    // Convert total system RAM from bytes to mebibytes (MiB).
    let ram_total_mib = sys.total_memory() / (1024 * 1024);

    let host = HostInfo {
        os: os_version,
        cpu: cpu_brand,
        ram_total_mib: ram_total_mib as u32,
    };

    // ── Step g: Build HardwareInfo ───────────────────────────────────
    // Compute the union of all GPU inference capabilities by OR-ing
    // across all GpuDevice.caps values. This represents the best
    // capabilities available across all devices on the system.
    let inference_caps = devices
        .iter()
        .filter(|d| d.device_type != DeviceType::Cpu)
        .fold(InferenceCaps::default(), |acc, dev| InferenceCaps {
            fp32: acc.fp32 || dev.caps.fp32,
            fp16: acc.fp16 || dev.caps.fp16,
            bf16: acc.bf16 || dev.caps.bf16,
            fp8: acc.fp8 || dev.caps.fp8,
            fp4: acc.fp4 || dev.caps.fp4,
            flash_attention: acc.flash_attention || dev.caps.flash_attention,
        });

    let hardware_info = HardwareInfo {
        host,
        gpus: devices,
        inference_caps,
    };

    Ok(hardware_info)
}

/// Convert a `DeviceType` enum to its string representation.
///
/// Used for building human-readable device names in the override path.
fn device_type_str(dt: &DeviceType) -> &'static str {
    match dt {
        DeviceType::Cuda => "CUDA",
        DeviceType::Rocm => "ROCm",
        DeviceType::Cpu => "CPU",
    }
}
