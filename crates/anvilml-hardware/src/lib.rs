//! GPU/CPU detection. Never panics on missing driver. Always returns >=1 CPU device.

pub mod cpu;
pub mod detect;
pub mod vulkan;
pub use cpu::CpuDetector;
pub use detect::DeviceDetector;
pub use detect::detect_all_devices;
pub use vulkan::VulkanDetector;
pub use vulkan::vendor_id_to_device_type;

#[cfg(feature = "mock-hardware")]
pub mod mock;
#[cfg(feature = "mock-hardware")]
pub use mock::MockDetector;

#[cfg(target_os = "windows")]
pub mod dxgi;
#[cfg(target_os = "windows")]
pub use dxgi::DxgiDetector;

#[cfg(target_os = "linux")]
pub mod sysfs;
#[cfg(target_os = "linux")]
pub use sysfs::{SysfsPciDetector, detect_from_path};
