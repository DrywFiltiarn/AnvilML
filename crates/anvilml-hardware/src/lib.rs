//! Hardware detection abstractions for AnvilML.
//!
//! Defines the [`DeviceDetector`] trait that all hardware backends must implement,
//! and provides a concrete [`CpuDetector`] implementation for CPU fallback.

pub mod cpu;
pub mod vulkan;

#[cfg(feature = "mock-hardware")]
pub mod mock;

// Re-export hardware types from anvilml-core for ergonomic downstream use.
pub use anvilml_core::{AnvilError, DeviceType, GpuDevice};

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-check: `CpuDetector` must implement `DeviceDetector`.
    #[test]
    fn cpu_detector_implements_trait() {
        let detector: &dyn DeviceDetector = &cpu::CpuDetector::default();
        let devices = detector.detect().expect("detect must succeed");
        assert!(!devices.is_empty());
    }

    /// Compile-check: `VulkanDetector` must implement `DeviceDetector`.
    #[test]
    fn vulkan_detector_implements_trait() {
        let detector: &dyn DeviceDetector = &vulkan::VulkanDetector::default();
        let devices = detector.detect().expect("detect must not return Err");
        // Result is always Ok — may be empty if no Vulkan loader present.
        let _ = devices;
    }
}
