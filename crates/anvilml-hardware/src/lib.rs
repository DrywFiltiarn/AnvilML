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
// detect_all_devices — stub (host info filled in P3-B1)
// ---------------------------------------------------------------------------

/// Detect all available devices.
///
/// When the `mock-hardware` feature is active, uses `MockDetector`
/// exclusively for a fully hermetic CI run. Otherwise falls back to
/// the real `CudaDetector` (if NVIDIA GPU hardware is present) and then
/// the `CpuDetector` as a fallback.
pub fn detect_all_devices() -> HardwareInfo {
    #[cfg(feature = "mock-hardware")]
    {
        let detector = mock::MockDetector;
        let gpus = detector
            .detect()
            .expect("mock detector should always succeed");

        HardwareInfo {
            host: HostInfo {
                os: String::new(),
                cpu_model: String::new(),
                ram_total_mib: 0,
                ram_free_mib: 0,
            },
            gpus,
            inference_caps: InferenceCaps {
                fp16: false,
                bf16: false,
                flash_attention: false,
            },
        }
    }

    #[cfg(not(feature = "mock-hardware"))]
    {
        // Try CUDA detector first.
        let cuda_detector = cuda::CudaDetector;
        let mut gpus = cuda_detector
            .detect()
            .expect("CUDA detector should always succeed");

        // If no CUDA GPUs found, fall back to CPU.
        if gpus.is_empty() {
            let cpu_detector = cpu::CpuDetector;
            gpus = cpu_detector
                .detect()
                .expect("CPU detector should always succeed");
        }

        HardwareInfo {
            host: HostInfo {
                os: String::new(),
                cpu_model: String::new(),
                ram_total_mib: 0,
                ram_free_mib: 0,
            },
            gpus,
            inference_caps: InferenceCaps {
                fp16: false,
                bf16: false,
                flash_attention: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_all_devices_returns_cpu_device() {
        let info = detect_all_devices();
        assert_eq!(info.gpus.len(), 1);
        #[cfg(feature = "mock-hardware")]
        assert_eq!(info.gpus[0].name, "Mock CPU");
        #[cfg(not(feature = "mock-hardware"))]
        assert_eq!(info.gpus[0].name, "CPU");

        assert!(matches!(info.gpus[0].device_type, DeviceType::Cpu));
    }

    #[test]
    fn detect_all_devices_host_fields_empty() {
        let info = detect_all_devices();
        assert!(info.host.os.is_empty());
        assert!(info.host.cpu_model.is_empty());
        assert_eq!(info.host.ram_total_mib, 0);
        assert_eq!(info.host.ram_free_mib, 0);
    }

    #[test]
    fn detect_all_devices_inference_caps_cpu() {
        let info = detect_all_devices();
        assert!(!info.inference_caps.fp16);
        assert!(!info.inference_caps.bf16);
        assert!(!info.inference_caps.flash_attention);
    }

    #[test]
    fn device_detector_trait_is_object_safe() {
        let detector: Box<dyn DeviceDetector> = Box::new(cpu::CpuDetector);
        let devices = detector.detect().unwrap();
        assert_eq!(devices.len(), 1);
    }
}
