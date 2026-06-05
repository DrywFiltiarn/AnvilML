//! Hardware detection abstractions for AnvilML.
//!
//! Defines the [`DeviceDetector`] trait that all hardware backends must implement,
//! and provides concrete implementations for multiple detection backends:
//!
//! - **Vulkan** — primary SDK-based detector via `ash`
//! - **CPU** — synthetic CPU fallback
//! - **DXGI** (Windows) — DXGI IDXGIFactory1 adapter enumeration
//! - **sysfs** (Linux/unix) — PCI sysfs device enumeration
//! - **NVML** (Linux/unix) — NVIDIA Management Library enumerator
//! - **Mock** (feature: `mock-hardware`) — synthetic devices from env vars

use anvilml_core::{
    AnvilError, CapabilitySource, EnumerationSource, GpuDevice, HardwareInfo, HostInfo,
    InferenceCaps, ServerConfig,
};

#[cfg(not(feature = "mock-hardware"))]
use anvilml_core::DeviceType;

pub mod cpu;
pub mod vulkan;

#[cfg(windows)]
pub mod dxgi;

#[cfg(unix)]
pub mod sysfs;

#[cfg(unix)]
pub mod nvml;

#[cfg(feature = "mock-hardware")]
pub mod mock;

pub mod device_db;

/// Trait that all hardware device detectors must implement.
///
/// Backends (CUDA, ROCm, CPU, mock) implement this trait to provide
/// device discovery and VRAM refresh capabilities.
pub trait DeviceDetector {
    /// Detect available devices and return a list of [`GpuDevice`] structs.
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>;

    /// Refresh VRAM usage for the device at `idx`.
    ///
    /// Returns `(total_mib, free_mib)` for the given device index.
    fn refresh_vram(&self, idx: u32) -> Result<(u32, u32), AnvilError>;
}

/// Map a PCI vendor ID to a [`DeviceType`].
///
/// Per ANVILML_DESIGN §5.3:
/// - `0x10DE` → Cuda (NVIDIA)
/// - `0x1002` → Rocm (AMD)
/// - Intel (`0x8086`) or anything else → Cpu
#[cfg(not(feature = "mock-hardware"))]
fn map_vendor_to_device_type(vendor_id: u16) -> DeviceType {
    match vendor_id {
        0x10DE => DeviceType::Cuda,
        0x1002 => DeviceType::Rocm,
        _ => DeviceType::Cpu,
    }
}

/// Compute the OR of all [`InferenceCaps`] in a device list.
fn or_all_caps(caps_list: &[&InferenceCaps]) -> InferenceCaps {
    let mut result = InferenceCaps::default();
    for caps in caps_list {
        result.fp32 |= caps.fp32;
        result.fp16 |= caps.fp16;
        result.bf16 |= caps.bf16;
        result.fp8 |= caps.fp8;
        result.fp4 |= caps.fp4;
        result.nvfp4 |= caps.nvfp4;
        result.flash_attention |= caps.flash_attention;
    }
    result
}

/// Populate [`HostInfo`] from sysinfo.
fn populate_host_info() -> HostInfo {
    let mut system = sysinfo::System::new_all();
    system.refresh_cpu_specifics(sysinfo::CpuRefreshKind::everything());
    system.refresh_memory();

    let os = sysinfo::System::long_os_version()
        .or_else(sysinfo::System::name)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown".to_string());
    let cpu_model = system
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    let ram_total_mib = system.total_memory() / 1024 / 1024;
    let ram_free_mib = system.free_memory() / 1024 / 1024;

    HostInfo {
        os,
        cpu_model,
        ram_total_mib,
        ram_free_mib,
    }
}

/// Enumerate GPUs via Vulkan, then fall back to platform-specific detectors.
#[cfg(not(feature = "mock-hardware"))]
fn enumerate_gpus() -> Vec<GpuDevice> {
    // Primary: Vulkan detector.
    let vulkan_devices = match vulkan::VulkanDetector.detect() {
        Ok(devs) if !devs.is_empty() => devs,
        Ok(_) => {
            tracing::warn!(
                detector = "Vulkan",
                "Vulkan detector returned empty device list"
            );
            Vec::new()
        }
        Err(e) => {
            tracing::warn!(detector = "Vulkan", error = %e, "Vulkan detection failed");
            Vec::new()
        }
    };

    if !vulkan_devices.is_empty() {
        return vulkan_devices;
    }

    #[cfg(windows)]
    {
        // Fallback: DXGI on Windows.
        let dxgi_devices = match dxgi::DxgiDetector::default().detect() {
            Ok(devs) if !devs.is_empty() => devs,
            Ok(_) => {
                tracing::warn!(
                    detector = "Dxgi",
                    "DXGI detector returned empty device list"
                );
                Vec::new()
            }
            Err(e) => {
                tracing::warn!(detector = "Dxgi", error = %e, "DXGI detection failed");
                Vec::new()
            }
        };
        if !dxgi_devices.is_empty() {
            return dxgi_devices;
        }
    }

    #[cfg(unix)]
    {
        // Fallback: sysfs on Unix.
        let mut devices = match sysfs::SysfsDetector.detect() {
            Ok(devs) if !devs.is_empty() => devs,
            Ok(_) => {
                tracing::warn!(
                    detector = "Sysfs",
                    "sysfs detector returned empty device list"
                );
                Vec::new()
            }
            Err(e) => {
                tracing::warn!(detector = "Sysfs", error = %e, "sysfs detection failed");
                Vec::new()
            }
        };

        // Additional: NVML on Unix (NVIDIA only, deduplicate by PCI ID).
        let nvml_devices = match nvml::NvmlDetector.detect() {
            Ok(devs) if !devs.is_empty() => devs,
            Ok(_) => {
                tracing::warn!(
                    detector = "Nvml",
                    "NVML detector returned empty device list"
                );
                Vec::new()
            }
            Err(e) => {
                tracing::warn!(detector = "Nvml", error = %e, "NVML detection failed");
                Vec::new()
            }
        };
        for nvml_dev in nvml_devices {
            if !devices.iter().any(|d| {
                d.pci_vendor_id == nvml_dev.pci_vendor_id
                    && d.pci_device_id == nvml_dev.pci_device_id
            }) {
                devices.push(nvml_dev);
            }
        }

        return devices;
    }

    // Fallback for platforms without windows or unix (e.g. macOS).
    #[allow(unreachable_code)]
    Vec::new()
}

/// The central hardware detection entry point.
///
/// Priority logic:
/// 1. If `config.hardware_override.is_some()` → return one synthetic device with
///    [`EnumerationSource::Override`].
/// 2. If `mock-hardware` feature is enabled → use [`MockDetector`] directly.
/// 3. Else enumerate via [`VulkanDetector`]; if empty, fall back to platform-specific
///    detectors (DXGI on Windows, sysfs+NVML on Unix).
/// 4. For each enumerated device, query the device DB store by PCI vendor/device ID and
///    call [`device_db::resolve_caps_from_row`] to populate name/arch/caps.
/// 5. Map PCI vendor ID → [`DeviceType`]: `0x10DE` = Cuda, `0x1002` = Rocm,
///    `0x8086` or unknown = Cpu.
/// 6. If zero GPUs detected → add one CPU device via [`CpuDetector`].
/// 7. Populate [`HostInfo`] via sysinfo (os, cpu_model, ram_total_mib, ram_free_mib).
/// 8. Set `inference_caps` on [`HardwareInfo`] to the OR of all device caps
///    (or defaults if no GPUs).
pub async fn detect_all_devices(
    cfg: &ServerConfig,
    pool: &anvilml_registry::SqlitePool,
) -> Result<HardwareInfo, AnvilError> {
    // Seed the device capability store via SQL seed files.
    anvilml_registry::seed_loader::run(pool, &cfg.seeds_path).await?;

    let store = anvilml_registry::DeviceCapabilityStore::new(pool.clone());

    // Branch 1: hardware override takes highest priority.
    if let Some(override_cfg) = &cfg.hardware_override {
        return Ok(HardwareInfo {
            host: populate_host_info(),
            gpus: vec![GpuDevice {
                index: 0,
                name: format!(
                    "Override ({:?}, {} MiB)",
                    override_cfg.device_type, override_cfg.vram_total_mib
                ),
                device_type: override_cfg.device_type,
                vram_total_mib: override_cfg.vram_total_mib,
                vram_free_mib: override_cfg.vram_total_mib,
                driver_version: "override".to_string(),
                pci_vendor_id: 0,
                pci_device_id: 0,
                arch: None,
                caps: InferenceCaps::default(),
                enumeration_source: EnumerationSource::Override,
                capabilities_source: CapabilitySource::Fallback,
            }],
            inference_caps: InferenceCaps::default(),
        });
    }

    // Branch 2: mock-hardware feature → MockDetector.
    #[cfg(feature = "mock-hardware")]
    {
        let mut gpus = mock::MockDetector.detect()?;

        // Resolve capabilities from device DB via store lookup.
        for dev in &mut gpus {
            let row = store.get(dev.pci_vendor_id, dev.pci_device_id).await?;
            device_db::resolve_caps_from_row(dev, row.as_ref());
        }

        let host = populate_host_info();
        let inference_caps = if !gpus.is_empty() {
            let caps_refs: Vec<&InferenceCaps> = gpus.iter().map(|d| &d.caps).collect();
            or_all_caps(&caps_refs)
        } else {
            InferenceCaps::default()
        };

        Ok(HardwareInfo {
            host,
            gpus,
            inference_caps,
        })
    }

    // Branch 3: enumerate GPUs (Vulkan → platform fallback).
    // This path is only compiled when mock-hardware feature is NOT enabled.
    #[cfg(not(feature = "mock-hardware"))]
    {
        let mut gpus = enumerate_gpus();

        // Resolve capabilities from device DB for each enumerated device.
        for dev in &mut gpus {
            if dev.pci_vendor_id != 0 || dev.pci_device_id != 0 {
                let row = store.get(dev.pci_vendor_id, dev.pci_device_id).await?;
                device_db::resolve_caps_from_row(dev, row.as_ref());
            } else {
                // No PCI IDs available — set fallback defaults.
                dev.caps = InferenceCaps::default();
                dev.capabilities_source = CapabilitySource::Fallback;
            }

            // Ensure device_type matches vendor ID (redundant for Vulkan but ensures
            // consistency for platform-specific fallbacks).
            if dev.pci_vendor_id != 0 {
                dev.device_type = map_vendor_to_device_type(dev.pci_vendor_id);
            }
        }

        // Branch 6: zero GPUs → add CPU device.
        if gpus.is_empty() {
            let cpu_devices = cpu::CpuDetector.detect()?;
            gpus = cpu_devices;
        }

        // Re-index devices sequentially.
        for (i, dev) in gpus.iter_mut().enumerate() {
            dev.index = i as u32;
        }

        let host = populate_host_info();

        // Compute inference_caps as OR of all device caps.
        let caps_refs: Vec<&InferenceCaps> = gpus.iter().map(|d| &d.caps).collect();
        let inference_caps = or_all_caps(&caps_refs);

        Ok(HardwareInfo {
            host,
            gpus,
            inference_caps,
        })
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use anvilml_core::config::HardwareOverrideConfig;
    use anvilml_core::DeviceType;
    use serial_test::serial;
    use std::fs;

    /// RAII guard that keeps a temp seeds directory alive for the duration of a test.
    struct SeedsGuard {
        _tmp: tempfile::TempDir,
        path: std::path::PathBuf,
    }

    impl SeedsGuard {
        fn new() -> Self {
            let tmp = tempfile::tempdir().expect("create temp dir");
            let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("backend")
                .join("seeds")
                .join("devices.sql");
            let dst = tmp.path().join("devices.sql");
            fs::copy(&src, &dst).expect("copy devices.sql into temp seeds dir");
            let path = tmp.path().to_path_buf();
            Self { _tmp: tmp, path }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    /// Vendor ID 0x10DE must map to Cuda.
    #[cfg(not(feature = "mock-hardware"))]
    #[test]
    fn vendor_map_cuda() {
        assert_eq!(map_vendor_to_device_type(0x10DE), DeviceType::Cuda);
    }

    /// Vendor ID 0x1002 must map to Rocm.
    #[cfg(not(feature = "mock-hardware"))]
    #[test]
    fn vendor_map_rocm() {
        assert_eq!(map_vendor_to_device_type(0x1002), DeviceType::Rocm);
    }

    /// Vendor ID 0x8086 (Intel) must map to Cpu.
    #[cfg(not(feature = "mock-hardware"))]
    #[test]
    fn vendor_map_cpu_intel() {
        assert_eq!(map_vendor_to_device_type(0x8086), DeviceType::Cpu);
    }

    /// Unknown vendor ID must map to Cpu.
    #[cfg(not(feature = "mock-hardware"))]
    #[test]
    fn vendor_map_cpu_unknown() {
        assert_eq!(map_vendor_to_device_type(0xDEAD), DeviceType::Cpu);
    }

    /// OR-ing caps: if any device has fp16, the result should have fp16.
    #[test]
    fn or_all_caps_merges() {
        let caps_a = InferenceCaps {
            fp32: false,
            fp16: true,
            bf16: false,
            fp8: false,
            fp4: false,
            nvfp4: false,
            flash_attention: false,
        };
        let caps_b = InferenceCaps {
            fp32: false,
            fp16: false,
            bf16: true,
            fp8: false,
            fp4: false,
            nvfp4: false,
            flash_attention: true,
        };

        let result = or_all_caps(&[&caps_a, &caps_b]);
        assert!(result.fp32 == false);
        assert!(result.fp16);
        assert!(result.bf16);
        assert!(result.fp8 == false);
        assert!(result.fp4 == false);
        assert!(result.nvfp4 == false);
        assert!(result.flash_attention);
    }

    /// OR-ing empty list returns defaults.
    #[test]
    fn or_all_caps_empty() {
        let result = or_all_caps(&[] as &[&InferenceCaps]);
        assert!(!result.fp32);
        assert!(!result.fp16);
        assert!(!result.bf16);
        assert!(!result.fp8);
        assert!(!result.fp4);
        assert!(!result.nvfp4);
        assert!(!result.flash_attention);
    }

    /// detect_all_devices with hardware_override must return one Override device.
    #[tokio::test]
    async fn detect_all_devices_override() {
        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            hardware_override: Some(HardwareOverrideConfig {
                device_type: DeviceType::Cuda,
                vram_total_mib: 16384,
            }),
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };

        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let info = detect_all_devices(&cfg, &pool)
            .await
            .expect("detect_all_devices should succeed");
        assert_eq!(info.gpus.len(), 1);
        let dev = &info.gpus[0];
        assert!(matches!(dev.device_type, DeviceType::Cuda));
        assert_eq!(dev.vram_total_mib, 16384);
        assert!(matches!(
            dev.enumeration_source,
            EnumerationSource::Override
        ));
        assert!(!info.gpus[0].name.is_empty());
    }

    /// detect_all_devices with override and Rocm device type.
    #[tokio::test]
    async fn detect_all_devices_override_rocm() {
        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            hardware_override: Some(HardwareOverrideConfig {
                device_type: DeviceType::Rocm,
                vram_total_mib: 24576,
            }),
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };

        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let info = detect_all_devices(&cfg, &pool)
            .await
            .expect("detect_all_devices should succeed");
        assert_eq!(info.gpus.len(), 1);
        assert!(matches!(info.gpus[0].device_type, DeviceType::Rocm));
        assert_eq!(info.gpus[0].vram_total_mib, 24576);
    }

    /// detect_all_devices with mock-hardware feature must return a Mock device.
    #[cfg(feature = "mock-hardware")]
    #[tokio::test]
    #[serial]
    async fn detect_all_devices_mock_cuda() {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cuda");
        std::env::set_var("ANVILML_MOCK_VRAM_MIB", "12288");

        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };
        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let info = detect_all_devices(&cfg, &pool)
            .await
            .expect("detect_all_devices should succeed");

        assert_eq!(info.gpus.len(), 1);
        assert!(matches!(info.gpus[0].device_type, DeviceType::Cuda));
        assert_eq!(info.gpus[0].vram_total_mib, 12288);
        assert!(matches!(
            info.gpus[0].enumeration_source,
            EnumerationSource::Mock
        ));
    }

    /// detect_all_devices with mock-hardware feature and ROCm.
    #[cfg(feature = "mock-hardware")]
    #[tokio::test]
    #[serial]
    async fn detect_all_devices_mock_rocm() {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "rocm");

        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };
        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let info = detect_all_devices(&cfg, &pool)
            .await
            .expect("detect_all_devices should succeed");

        assert_eq!(info.gpus.len(), 1);
        assert!(matches!(info.gpus[0].device_type, DeviceType::Rocm));
    }

    /// Vulkan detection must not panic even without a GPU.
    #[tokio::test]
    async fn detect_all_devices_vulkan_empty() {
        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };
        let pool = anvilml_registry::open_in_memory().await.unwrap();
        // This should always succeed — may return empty GPUs on systems without Vulkan.
        let result = detect_all_devices(&cfg, &pool).await;
        assert!(result.is_ok(), "detect_all_devices must never return Err");

        let info = result.unwrap();
        // On a system with no GPUs, should have at least one CPU device.
        assert!(!info.gpus.is_empty(), "must have at least one device");
    }

    /// detect_all_devices with override returns Override enumeration source.
    #[tokio::test]
    async fn detect_all_devices_override_source() {
        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            hardware_override: Some(HardwareOverrideConfig {
                device_type: DeviceType::Cuda,
                vram_total_mib: 16384,
            }),
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };

        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let info = detect_all_devices(&cfg, &pool)
            .await
            .expect("detect_all_devices should succeed");
        assert!(matches!(
            info.gpus[0].enumeration_source,
            EnumerationSource::Override
        ));
        assert!(matches!(
            info.gpus[0].capabilities_source,
            CapabilitySource::Fallback
        ));
    }

    /// detect_all_devices with override and CPU device type.
    #[tokio::test]
    async fn detect_all_devices_override_cpu() {
        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            hardware_override: Some(HardwareOverrideConfig {
                device_type: DeviceType::Cpu,
                vram_total_mib: 0,
            }),
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };

        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let info = detect_all_devices(&cfg, &pool)
            .await
            .expect("detect_all_devices should succeed");
        assert_eq!(info.gpus.len(), 1);
        assert!(matches!(info.gpus[0].device_type, DeviceType::Cpu));
        assert_eq!(info.gpus[0].vram_total_mib, 0);
    }

    /// detect_all_devices must always return Ok (never Err).
    #[tokio::test]
    async fn detect_all_devices_never_errs() {
        let pool = anvilml_registry::open_in_memory().await.unwrap();

        // Test with default config.
        let guard1 = SeedsGuard::new();
        let cfg1 = ServerConfig {
            seeds_path: guard1.path().to_path_buf(),
            ..ServerConfig::default()
        };
        let result = detect_all_devices(&cfg1, &pool).await;
        assert!(result.is_ok());

        // Test with override config.
        let guard2 = SeedsGuard::new();
        let cfg2 = ServerConfig {
            hardware_override: Some(HardwareOverrideConfig {
                device_type: DeviceType::Cuda,
                vram_total_mib: 8192,
            }),
            seeds_path: guard2.path().to_path_buf(),
            ..ServerConfig::default()
        };
        let result = detect_all_devices(&cfg2, &pool).await;
        assert!(result.is_ok());
    }

    /// HostInfo fields must be populated correctly.
    #[tokio::test]
    async fn host_info_populated() {
        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };
        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let info = detect_all_devices(&cfg, &pool)
            .await
            .expect("detect_all_devices should succeed");

        assert!(!info.host.os.is_empty());
        assert!(!info.host.cpu_model.is_empty());
        assert!(info.host.ram_total_mib > 0);
        assert!(info.host.ram_free_mib <= info.host.ram_total_mib);
    }

    /// detect_all_devices must produce sequential device indices.
    #[tokio::test]
    async fn devices_have_sequential_indices() {
        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };
        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let info = detect_all_devices(&cfg, &pool)
            .await
            .expect("detect_all_devices should succeed");

        for (i, dev) in info.gpus.iter().enumerate() {
            assert_eq!(dev.index, i as u32);
        }
    }

    /// Override device new fields must be correct.
    #[tokio::test]
    async fn override_device_new_fields() {
        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            hardware_override: Some(HardwareOverrideConfig {
                device_type: DeviceType::Cuda,
                vram_total_mib: 16384,
            }),
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };

        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let info = detect_all_devices(&cfg, &pool)
            .await
            .expect("detect_all_devices should succeed");
        let dev = &info.gpus[0];

        assert_eq!(dev.pci_vendor_id, 0);
        assert_eq!(dev.pci_device_id, 0);
        assert!(dev.arch.is_none());
        assert!(matches!(
            dev.enumeration_source,
            EnumerationSource::Override
        ));
        assert!(matches!(
            dev.capabilities_source,
            CapabilitySource::Fallback
        ));
    }

    /// Mock device new fields must be correct (mock-hardware feature).
    #[cfg(feature = "mock-hardware")]
    #[tokio::test]
    #[serial]
    async fn mock_device_new_fields_in_detect_all() {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cuda");

        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };
        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let info = detect_all_devices(&cfg, &pool)
            .await
            .expect("detect_all_devices should succeed");

        let dev = &info.gpus[0];
        assert!(matches!(dev.enumeration_source, EnumerationSource::Mock));
        assert!(matches!(
            dev.capabilities_source,
            CapabilitySource::Fallback
        ));
    }

    /// detect_all_devices with mock-hardware returns Mock enumeration source.
    #[cfg(feature = "mock-hardware")]
    #[tokio::test]
    #[serial]
    async fn detect_all_devices_mock_enum_source() {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cpu");

        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };
        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let info = detect_all_devices(&cfg, &pool)
            .await
            .expect("detect_all_devices should succeed");

        assert!(matches!(
            info.gpus[0].enumeration_source,
            EnumerationSource::Mock
        ));
    }

    /// detect_all_devices with mock-hardware returns Mock device type.
    #[cfg(feature = "mock-hardware")]
    #[tokio::test]
    #[serial]
    async fn detect_all_devices_mock_device_type() {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "rocm");

        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };
        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let info = detect_all_devices(&cfg, &pool)
            .await
            .expect("detect_all_devices should succeed");

        assert!(matches!(info.gpus[0].device_type, DeviceType::Rocm));
    }

    /// detect_all_devices with mock-hardware returns Mock device VRAM.
    #[cfg(feature = "mock-hardware")]
    #[tokio::test]
    #[serial]
    async fn detect_all_devices_mock_vram() {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cuda");
        std::env::set_var("ANVILML_MOCK_VRAM_MIB", "32768");

        let guard = SeedsGuard::new();
        let cfg = ServerConfig {
            seeds_path: guard.path().to_path_buf(),
            ..ServerConfig::default()
        };
        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let info = detect_all_devices(&cfg, &pool)
            .await
            .expect("detect_all_devices should succeed");

        assert_eq!(info.gpus[0].vram_total_mib, 32768);
    }
}
