//! GPU/CPU detection. Never panics on missing driver. Always returns >=1 CPU device.

pub mod cpu;
pub mod detect;
pub use cpu::CpuDetector;
pub use detect::DeviceDetector;
