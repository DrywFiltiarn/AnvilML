//! Hardware detection for AnvilML.
//!
//! Provides the `DeviceDetector` trait and concrete implementations for
//! detecting available compute devices (CPU, CUDA GPUs, ROCm GPUs).
//!
//! # Modules
//!
//! - `cpu` — CPU fallback detector
//! - `cuda` — NVIDIA CUDA detector (via `nvidia-smi`) — added in P3-A3
//! - `rocm` — AMD ROCm detector (via `rocm-smi`) — added in P3-A4
//! - `mock` — Deterministic mock detector for CI (feature-gated) — added in P3-A2

pub mod cpu;
pub mod cuda;
#[cfg(feature = "mock-hardware")]
pub mod mock;
pub mod rocm;

pub use anvilml_core::{types::*, AnvilError};

// ---------------------------------------------------------------------------
// DeviceDetector trait — object-safe for Box<dyn DeviceDetector>
// ---------------------------------------------------------------------------

/// Trait for detecting hardware devices.
///
/// All concrete detectors (CPU, CUDA, ROCm, mock) implement this trait.
/// It is object-safe so it can be used as `Box<dyn DeviceDetector>`.
pub trait DeviceDetector {
    /// Detect available devices.
    ///
    /// Returns a vector of detected `GpuDevice`s. An empty vector means
    /// no devices of this type are present — this is not an error.
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>;

    /// Refresh VRAM usage for a specific device.
    ///
    /// Returns `(used_mib, total_mib)` for the device at `device_index`.
    fn refresh_vram(&self, device_index: u32) -> Result<(u32, u32), AnvilError>;
}

// ---------------------------------------------------------------------------
// detect_all_devices — override wiring + sysinfo host population (P3-B1)
// ---------------------------------------------------------------------------

/// Detect all available devices.
///
/// When the `mock-hardware` feature is active, uses `MockDetector`
/// exclusively for a fully hermetic CI run. Otherwise falls back to
/// the real `CudaDetector` (if NVIDIA GPU hardware is present), then
/// the `RocmDetector` (if AMD GPU hardware is present), and finally
/// the `CpuDetector` as a fallback.
///
/// When `override_config` is `Some`, the detector pipeline is forced to
/// run only the matching device type, skipping all other detectors.
/// This is useful for testing / CI scenarios where real GPU hardware
/// may not be available.
pub fn detect_all_devices(
    override_config: Option<&anvilml_core::config::HardwareOverrideConfig>,
) -> HardwareInfo {
    // Populate host info using sysinfo.
    let mut sys = sysinfo::System::new_all();
    sys.refresh_memory();
    let host_info = populate_host_info(&sys);

    #[cfg(feature = "mock-hardware")]
    {
        // Mock mode: override_config is intentionally ignored — always uses
        // MockDetector for a fully hermetic CI run.
        let _override_config = override_config;
        let detector = mock::MockDetector;
        let gpus = detector
            .detect()
            .expect("mock detector should always succeed");

        HardwareInfo {
            host: host_info,
            gpus,
            inference_caps: InferenceCaps {
                fp16: false,
                bf16: false,
                flash_attention: false,
            },
        }
    }

    #[cfg(not(feature = "mock-hardware"))]
    match override_config {
        Some(override_cfg) => {
            // Override mode: run only the specified detector.
            let gpus = match &override_cfg.device_type {
                anvilml_core::config::DeviceType::Cuda => {
                    let detector = cuda::CudaDetector;
                    detector
                        .detect()
                        .expect("CUDA detector should always succeed")
                }
                anvilml_core::config::DeviceType::Rocm => {
                    let detector = rocm::RocmDetector;
                    detector
                        .detect()
                        .expect("ROCm detector should always succeed")
                }
                anvilml_core::config::DeviceType::Cpu => {
                    let detector = cpu::CpuDetector;
                    detector
                        .detect()
                        .expect("CPU detector should always succeed")
                }
            };

            HardwareInfo {
                host: host_info,
                gpus,
                inference_caps: InferenceCaps {
                    fp16: false,
                    bf16: false,
                    flash_attention: false,
                },
            }
        }
        None => {
            // Auto-detection mode: CUDA → ROCm → CPU fallback.
            let cuda_detector = cuda::CudaDetector;
            let mut gpus = cuda_detector
                .detect()
                .expect("CUDA detector should always succeed");

            if gpus.is_empty() {
                let rocm_detector = rocm::RocmDetector;
                gpus = rocm_detector
                    .detect()
                    .expect("ROCm detector should always succeed");
            }

            if gpus.is_empty() {
                let cpu_detector = cpu::CpuDetector;
                gpus = cpu_detector
                    .detect()
                    .expect("CPU detector should always succeed");
            }

            HardwareInfo {
                host: host_info,
                gpus,
                inference_caps: InferenceCaps {
                    fp16: false,
                    bf16: false,
                    flash_attention: false,
                },
            }
        }
    }
}

/// Populate `HostInfo` fields using the `sysinfo` crate.
fn populate_host_info(sys: &sysinfo::System) -> HostInfo {
    let os = sysinfo::System::long_os_version().unwrap_or_else(|| String::from("unknown"));

    let cpu_model = sys
        .cpus()
        .first()
        .map(|c| c.name().to_string())
        .unwrap_or_else(|| String::from("unknown"));

    // sysinfo reports memory in bytes; convert to MiB.
    let ram_total_mib = sys.total_memory() / 1024 / 1024;
    let ram_free_mib = sys.available_memory() / 1024 / 1024;

    HostInfo {
        os,
        cpu_model,
        ram_total_mib,
        ram_free_mib,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Re-export config DeviceType for constructing HardwareOverrideConfig.
    use anvilml_core::config::DeviceType as ConfigDeviceType;

    // ------------------------------------------------------------------
    // Integration tests for detect_all_devices (no override)
    // ------------------------------------------------------------------

    #[test]
    fn detect_all_devices_returns_cpu_device() {
        let info = detect_all_devices(None);
        assert_eq!(info.gpus.len(), 1);
        #[cfg(feature = "mock-hardware")]
        assert_eq!(info.gpus[0].name, "Mock CPU");
        #[cfg(not(feature = "mock-hardware"))]
        assert_eq!(info.gpus[0].name, "CPU");

        assert!(matches!(info.gpus[0].device_type, DeviceType::Cpu));
    }

    /// Host info fields are now populated via sysinfo — verify they are non-empty.
    #[test]
    fn detect_all_devices_host_fields_populated() {
        let info = detect_all_devices(None);
        assert!(
            !info.host.os.is_empty(),
            "OS name should be populated by sysinfo"
        );
        assert!(
            !info.host.cpu_model.is_empty(),
            "CPU model should be populated by sysinfo"
        );
        assert!(info.host.ram_total_mib > 0, "total RAM (MiB) should be > 0");
        // ram_free_mib is u64, so it's always >= 0 — just verify it exists.
        let _ = info.host.ram_free_mib;
    }

    #[test]
    fn detect_all_devices_inference_caps_cpu() {
        let info = detect_all_devices(None);
        assert!(!info.inference_caps.fp16);
        assert!(!info.inference_caps.bf16);
        assert!(!info.inference_caps.flash_attention);
    }

    // ------------------------------------------------------------------
    // Override integration tests
    // ------------------------------------------------------------------

    #[test]
    fn detect_all_devices_force_cpu_override() {
        let override_cfg = anvilml_core::config::HardwareOverrideConfig {
            device_type: ConfigDeviceType::Cpu,
            vram_total_mib: 8192,
        };
        let info = detect_all_devices(Some(&override_cfg));

        assert_eq!(info.gpus.len(), 1);
        assert!(matches!(info.gpus[0].device_type, DeviceType::Cpu));
        #[cfg(feature = "mock-hardware")]
        assert_eq!(info.gpus[0].name, "Mock CPU");
        #[cfg(not(feature = "mock-hardware"))]
        assert_eq!(info.gpus[0].name, "CPU");

        // Host info should still be populated.
        assert!(!info.host.os.is_empty());
        assert!(info.host.ram_total_mib > 0);
    }

    #[test]
    #[cfg(not(feature = "mock-hardware"))]
    fn detect_all_devices_force_cuda_override() {
        let override_cfg = anvilml_core::config::HardwareOverrideConfig {
            device_type: ConfigDeviceType::Cuda,
            vram_total_mib: 16384,
        };
        let info = detect_all_devices(Some(&override_cfg));

        // On a machine without CUDA, the detector returns an empty list
        // but never errors — so we expect an empty vec here.
        assert!(info.gpus.is_empty());

        // Host info should still be populated.
        assert!(!info.host.os.is_empty());
        assert!(info.host.ram_total_mib > 0);
    }

    #[test]
    #[cfg(not(feature = "mock-hardware"))]
    fn detect_all_devices_force_rocm_override() {
        let override_cfg = anvilml_core::config::HardwareOverrideConfig {
            device_type: ConfigDeviceType::Rocm,
            vram_total_mib: 32768,
        };
        let info = detect_all_devices(Some(&override_cfg));

        // On a machine without ROCm, the detector returns an empty list.
        assert!(info.gpus.is_empty());

        // Host info should still be populated.
        assert!(!info.host.os.is_empty());
        assert!(info.host.ram_total_mib > 0);
    }

    #[test]
    fn detect_all_devices_override_host_info_fields() {
        let override_cfg = anvilml_core::config::HardwareOverrideConfig {
            device_type: ConfigDeviceType::Cpu,
            vram_total_mib: 8192,
        };
        let info = detect_all_devices(Some(&override_cfg));

        // Verify all host info fields are populated.
        assert!(!info.host.os.is_empty());
        assert!(!info.host.cpu_model.is_empty());
        assert!(info.host.ram_total_mib > 0);
        let _ = info.host.ram_free_mib;
    }

    // ------------------------------------------------------------------
    // DeviceDetector trait object-safety
    // ------------------------------------------------------------------

    #[test]
    fn device_detector_trait_is_object_safe() {
        let detector: Box<dyn DeviceDetector> = Box::new(cpu::CpuDetector);
        let devices = detector.detect().unwrap();
        assert_eq!(devices.len(), 1);
    }
}
