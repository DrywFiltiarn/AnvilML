//! GPU and CPU hardware detection for AnvilML.
//!
//! This crate owns device enumeration via Vulkan (primary), with platform
//! fallbacks: DxgiDetector (Windows), SysfsPciDetector (Linux), and NvmlDetector
//! (Linux VRAM refresh). When the `mock-hardware` feature is active,
//! MockDetector provides deterministic device lists driven by environment variables.
//!
//! **Hard constraints:** Never panic on missing drivers. Always return at least
//! one CPU device. Return `Err` for detection failures — never crash.

#[allow(dead_code)]
pub fn stub() {}
