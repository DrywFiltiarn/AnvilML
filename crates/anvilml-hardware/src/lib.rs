//! GPU and CPU hardware detection for AnvilML.
//!
//! This crate owns device enumeration via the `DeviceDetector` trait, with
//! concrete implementations: `CpuDetector` (synthesises a CPU device using
//! `sysinfo`), and platform-specific detectors (Vulkan, DXGI, sysfs) that
//! will be added in subsequent phases. When the `mock-hardware` feature is
//! active, `MockDetector` provides deterministic device lists driven by
//! environment variables.
//!
//! **Hard constraints:** Never panic on missing drivers. Always return at
//! least one CPU device. Return `Err` for detection failures — never crash.

//! GPU and CPU hardware detection for AnvilML.
//!
//! This crate owns device enumeration via the `DeviceDetector` trait, with
//! concrete implementations: `CpuDetector` (synthesises a CPU device using
//! `sysinfo`), `VulkanDetector` (enumerates GPUs via the Vulkan loader),
//! and platform-specific detectors (DXGI on Windows, sysfs/NVML on Linux).
//! When the `mock-hardware` feature is active, `MockDetector` provides
//! deterministic device lists driven by environment variables.
//!
//! **Hard constraints:** Never panic on missing drivers. Always return at
//! least one CPU device. Return `Err` for detection failures — never crash.

pub mod cpu;
pub mod vulkan;

#[cfg(all(windows, feature = "dxgi"))]
pub mod dxgi;
#[cfg(all(unix, feature = "nvml"))]
pub mod nvml;
#[cfg(unix)]
pub mod sysfs;

use anvilml_core::{AnvilError, GpuDevice};

/// Trait implemented by all hardware enumeration backends.
///
/// A backend can enumerate devices (GPUs, CPUs) and refresh live VRAM
/// snapshots. Implementations must be `Send + Sync` so they can be shared
/// across threads and tokio tasks.
pub trait DeviceDetector: Send + Sync {
    /// Enumerate available devices.
    ///
    /// Returns a list of detected devices. Each device is assigned a
    /// zero-based index that remains stable across calls.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Io` if the underlying detection mechanism
    /// fails (e.g. Vulkan loader absent, DXGI COM initialization error).
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>;

    /// Refresh the VRAM total and free values for the device at `index`.
    ///
    /// Returns `(total_mib, free_mib)` where both values are in mebibytes.
    /// The `total_mib` value is stable across calls; `free_mib` changes
    /// as the device is used.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Io` if the VRAM query mechanism fails.
    fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>;
}

pub use cpu::CpuDetector;
pub use vulkan::VulkanDetector;

#[cfg(all(windows, feature = "dxgi"))]
pub use dxgi::DxgiDetector;
#[cfg(all(unix, feature = "nvml"))]
pub use nvml::NvmlDetector;
#[cfg(unix)]
pub use sysfs::SysfsPciDetector;
