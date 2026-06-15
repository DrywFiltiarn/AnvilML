//! GPU and CPU hardware detection for AnvilML.
//!
//! This crate owns device enumeration via the `DeviceDetector` trait, with
//! concrete implementations: `CpuDetector` (synthesises a CPU device using
//! `sysinfo`), `VulkanDetector` (enumerates GPUs via the Vulkan loader),
//! and platform-specific detectors (DXGI on Windows, sysfs/NVML on Linux).
//! The `detect_all_devices` function orchestrates the full detection
//! pipeline with a defined priority chain: hardware override → mock →
//! Vulkan → platform fallbacks → CPU.
//! When the `mock-hardware` feature is active, `MockDetector` provides
//! deterministic device lists driven by environment variables.
//!
//! **Hard constraints:** Never panic on missing drivers. Always return at
//! least one CPU device. Return `Err` for detection failures — never crash.

pub mod cpu;
pub mod detect;
pub mod device_db;
pub mod vulkan;

#[cfg(feature = "mock-hardware")]
pub mod mock;

#[cfg(windows)]
pub mod dxgi;
#[cfg(unix)]
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
pub use detect::detect_all_devices;
pub use device_db::{resolve_caps_from_row, DeviceRow, DEVICE_DB};
pub use vulkan::VulkanDetector;

#[cfg(feature = "mock-hardware")]
pub use mock::MockDetector;

#[cfg(windows)]
pub use dxgi::DxgiDetector;
#[cfg(unix)]
pub use nvml::NvmlDetector;
#[cfg(unix)]
pub use sysfs::SysfsPciDetector;
