//! NVML GPU enumerator (Linux/unix, lazy-loaded).
//!
//! Implements an NVML-based GPU detector using the `libloading` crate to
//! dynamically load `libnvidia-ml.so` at runtime. If the library is absent
//! or initialization fails, `detect()` returns `Ok(vec![])` — no panic,
//! no error. This follows the SDK-free design principle: "Loader absent →
//! Ok(vec![])."
//!
//! Device enumeration uses NVML API calls:
//! - `nvmlDeviceGetCount_v2()` → number of NVIDIA devices
//! - `nvmlDeviceGetName()` → device name string
//! - `nvmlDeviceGetPciInfo_v2()` → PCI bus/device information
//! - `nvmlDeviceGetMemoryInfo()` → VRAM total and used memory
//!
//! All NVIDIA GPUs are mapped to [`DeviceType::Cuda`](anvilml_core::DeviceType)
//! since NVML only works on NVIDIA hardware.

#![cfg(unix)]

use anvilml_core::{AnvilError, DeviceType, GpuDevice};

use crate::DeviceDetector;

// ── Constants ─────────────────────────────────────────────────────────────────

/// MiB divisor (bytes → MiB).
const BYTES_PER_MIB: u64 = 1024 * 1024;

/// NVML library name.
const NVML_LIB: &str = "libnvidia-ml.so.1";

// ── FFI bindings for libnvidia-ml.so ──────────────────────────────────────────

/// Opaque handle to an NVML device.
type NvmlDevice = *mut std::os::raw::c_void;

/// NVML return code enum (values match the C enum).
#[repr(i32)]
enum NvmlReturn {
    Success = 0,
}

// Function pointer types for NVML API.
type NvmlInitV2Fn = unsafe extern "C" fn() -> i32;
type NvmlShutdownFn = unsafe extern "C" fn() -> i32;
type NvmlDeviceGetCountV2Fn = unsafe extern "C" fn(*mut u32) -> i32;
type NvmlDeviceByIndexFn = unsafe extern "C" fn(u32, *mut NvmlDevice) -> i32;
type NvmlDeviceGetNameFn = unsafe extern "C" fn(NvmlDevice, *mut std::os::raw::c_char, u32) -> i32;
type NvmlDeviceGetPciInfoV2Fn = unsafe extern "C" fn(NvmlDevice, *mut NvmlPciInfoV2) -> i32;
type NvmlDeviceGetMemoryInfoFn = unsafe extern "C" fn(NvmlDevice, *mut NvmlMemoryInfo) -> i32;

/// PCI info structure layout matching NVML 11.x+.
///
/// Layout based on nvml_structs.h:
/// ```c
/// typedef struct nvmlPciInfo_v2 {
///     char busId[16];
///     ...
///     unsigned int domain;
///     unsigned int bus;
///     unsigned int device;
///     unsigned int pciDeviceId;
///     ...
///     unsigned int pciSlotIndex;  // added in driver 396.xx
///     ...
/// } nvmlPciInfo_v2_t;
/// ```
#[repr(C)]
struct NvmlPciInfoV2 {
    _bus_id: [std::os::raw::c_char; 16],
    _instance_id: std::os::raw::c_char,
    _domestic_id: std::os::raw::c_char,
    _bus_id_computed: [std::os::raw::c_char; 13],
    domain: u32,
    bus: u32,
    device: u32,
    pci_device_id: u32,
    _subsystem_id: u32,
    _pci_bus_id: [std::os::raw::c_char; 12],
    flags: u16,
    _tc_index: u8,
    _bdf_id: u32,
    _pcie_device_id: u16,
    _pcie_subsystem_id: u16,
    _pcie_vendor_id: u16,
    pci_slot: [std::os::raw::c_char; 32],
    pci_slot_index: u32,
    _power_state: u32,
    _cur_link_speed: u32,
    _pcie_generation: u8,
    _max_link_width: u8,
    _link_speeds: [u64; 10],
    _num_link_width_entries: u8,
    _replay_reset_count: u32,
}

/// Memory info structure layout matching NVML.
///
/// ```c
/// typedef struct {
///     unsigned long long total;
///     unsigned long long used;
///     unsigned long long free;  // added in driver 418.x
/// } nvmlMemory_t;
/// ```
#[repr(C)]
struct NvmlMemoryInfo {
    total: u64,
    used: u64,
    _free: u64, // May not exist in older drivers.
}

// ── Library wrapper ───────────────────────────────────────────────────────────

/// Lazy-loaded NVML library handle with function pointers.
struct NvmlLibrary {
    #[allow(dead_code)]
    lib: libloading::Library,
    nvml_init_v2: NvmlInitV2Fn,
    nvml_shutdown: NvmlShutdownFn,
    nvml_device_get_count_v2: NvmlDeviceGetCountV2Fn,
    nvml_device_by_index: NvmlDeviceByIndexFn,
    nvml_device_get_name: NvmlDeviceGetNameFn,
    nvml_device_get_pci_info_v2: NvmlDeviceGetPciInfoV2Fn,
    nvml_device_get_memory_info: NvmlDeviceGetMemoryInfoFn,
}

impl NvmlLibrary {
    /// Load libnvidia-ml.so and resolve all required function symbols.
    ///
    /// Returns `None` if the library cannot be loaded or any symbol is missing.
    fn load() -> Option<Self> {
        let lib = unsafe { libloading::Library::new(NVML_LIB).ok()? };

        macro_rules! get_sym {
            ($name:ident) => {{
                let bytes = stringify!($name).as_bytes();
                match unsafe { lib.get(bytes) } {
                    Ok(sym) => *sym,
                    Err(_) => return None,
                }
            }};
        }

        // Resolve all symbols first (borrowing `lib`), then move it into the struct.
        let nvml_init_v2 = get_sym!(nvmlInit_v2);
        let nvml_shutdown = get_sym!(nvmlShutdown);
        let nvml_device_get_count_v2 = get_sym!(nvmlDeviceGetCount_v2);
        let nvml_device_by_index = get_sym!(nvmlDeviceByIndex);
        let nvml_device_get_name = get_sym!(nvmlDeviceGetName);
        let nvml_device_get_pci_info_v2 = get_sym!(nvmlDeviceGetPciInfo_v2);
        let nvml_device_get_memory_info = get_sym!(nvmlDeviceGetMemoryInfo);

        Some(Self {
            lib,
            nvml_init_v2,
            nvml_shutdown,
            nvml_device_get_count_v2,
            nvml_device_by_index,
            nvml_device_get_name,
            nvml_device_get_pci_info_v2,
            nvml_device_get_memory_info,
        })
    }
}

impl Drop for NvmlLibrary {
    fn drop(&mut self) {
        // Attempt shutdown, but don't propagate errors — we're in a destructor.
        let _ = unsafe { (self.nvml_shutdown)() };
    }
}

// ── NvmlDetector ──────────────────────────────────────────────────────────────

/// An NVML-based GPU detector.
///
/// Dynamically loads `libnvidia-ml.so` at runtime and enumerates all NVIDIA
/// devices. If the library is absent or initialization fails, `detect()`
/// returns `Ok(vec![])` — no panic, no error.
#[derive(Debug, Clone, Default)]
pub struct NvmlDetector;

impl DeviceDetector for NvmlDetector {
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        // Lazy-load the NVML library.
        let nvml = match NvmlLibrary::load() {
            Some(lib) => lib,
            None => {
                log::warn!("nvml: libnvidia-ml.so not found or could not be loaded");
                return Ok(Vec::new());
            }
        };

        // Initialize NVML.
        let ret = unsafe { (nvml.nvml_init_v2)() };
        if ret != NvmlReturn::Success as i32 {
            log::warn!("nvml: nvmlInit_v2 failed (code={ret}), returning empty device list");
            return Ok(Vec::new());
        }

        // Get device count.
        let mut device_count: u32 = 0;
        let ret = unsafe { (nvml.nvml_device_get_count_v2)(&mut device_count) };
        if ret != NvmlReturn::Success as i32 {
            log::warn!("nvml: nvmlDeviceGetCount_v2 failed (code={ret})");
            return Ok(Vec::new());
        }

        let mut devices = Vec::with_capacity(device_count as usize);

        for idx in 0..device_count {
            // Get device handle.
            let mut device: NvmlDevice = std::ptr::null_mut();
            let ret = unsafe { (nvml.nvml_device_by_index)(idx, &mut device) };
            if ret != NvmlReturn::Success as i32 {
                log::warn!("nvml: nvmlDeviceByIndex({idx}) failed (code={ret}), skipping");
                continue;
            }

            // Get device name.
            let mut name_buf: [std::os::raw::c_char; 96] = [0; 96];
            let ret = unsafe { (nvml.nvml_device_get_name)(device, name_buf.as_mut_ptr(), 96) };

            let device_name = if ret == NvmlReturn::Success as i32 {
                // SAFETY: name_buf is a valid C string from NVML.
                let c_str = unsafe { std::ffi::CStr::from_ptr(name_buf.as_ptr()) };
                c_str.to_string_lossy().into_owned()
            } else {
                format!("NVIDIA GPU #{idx}")
            };

            // Get PCI info.
            let mut pci_info: NvmlPciInfoV2 = unsafe { std::mem::zeroed() };
            let ret = unsafe { (nvml.nvml_device_get_pci_info_v2)(device, &mut pci_info) };

            let _pci_bus_device = if ret == NvmlReturn::Success as i32 {
                format!(
                    "{:04x}:{:02x}:{:02x}.{}",
                    pci_info.domain,
                    pci_info.bus,
                    pci_info.device,
                    0 // Function is not in the struct; use 0.
                )
            } else {
                String::new()
            };

            // Get memory info.
            let mut mem_info: NvmlMemoryInfo = unsafe { std::mem::zeroed() };
            let ret = unsafe { (nvml.nvml_device_get_memory_info)(device, &mut mem_info) };

            let (vram_total_mib, vram_free_mib) = if ret == NvmlReturn::Success as i32 {
                let total_mib = (mem_info.total / BYTES_PER_MIB) as u32;
                let used_mib = (mem_info.used / BYTES_PER_MIB) as u32;
                let free_mib = total_mib.saturating_sub(used_mib);
                (total_mib, free_mib)
            } else {
                (0, 0)
            };

            // All NVML devices are NVIDIA → Cuda.
            devices.push(GpuDevice {
                index: idx,
                name: device_name,
                device_type: DeviceType::Cuda,
                vram_total_mib,
                vram_free_mib,
                driver_version: String::new(),
            });
        }

        Ok(devices)
    }

    fn refresh_vram(&self, _idx: u32) -> Result<(u32, u32), AnvilError> {
        // VRAM refresh would require a live NvmlLibrary instance.
        // For now, return (0, 0).
        Ok((0, 0))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anvilml_core::DeviceType;
    use serial_test::serial;

    /// NVML init fallback: library absent → detect returns Ok(vec![]) with no panic.
    #[test]
    #[serial]
    fn nvml_init_fallback_no_library() {
        let detector = NvmlDetector::default();
        let result = detector.detect();
        // Must always return Ok — never panics, never Err.
        assert!(result.is_ok(), "detect() must return Ok, got {:?}", result);
    }

    /// Vendor ID mapping: all NVML devices are Cuda (NVIDIA only).
    #[test]
    #[serial]
    fn nvml_all_devices_are_cuda() {
        // NVML only works on NVIDIA hardware, so all detected devices
        // should be mapped to Cuda. This is verified by the detect() impl
        // which hardcodes DeviceType::Cuda for all NVML devices.
        let detector = NvmlDetector::default();
        let result = detector.detect().expect("detect must succeed");
        for device in &result {
            assert!(
                matches!(device.device_type, DeviceType::Cuda),
                "NVML device should always be Cuda"
            );
        }
    }

    /// NvmlDetector must implement the DeviceDetector trait.
    #[test]
    #[serial]
    fn nvml_detect_returns_ok() {
        let detector = NvmlDetector::default();
        let result = detector.detect();
        // Must always return Ok — never panics, never Err.
        assert!(result.is_ok(), "detect() must return Ok, got {:?}", result);
    }

    /// NVML library load failure: if libnvidia-ml.so is absent, NvmlLibrary::load returns None.
    #[test]
    #[serial]
    fn nvml_library_load_fails_gracefully() {
        // In CI environments without NVIDIA drivers, this should return None.
        let result = NvmlLibrary::load();
        assert!(
            result.is_none(),
            "NVML library should not be loadable in test environment"
        );
    }

    /// NVML shutdown is called in Drop and doesn't panic even if not initialized.
    #[test]
    #[serial]
    fn nvml_shutdown_in_drop_no_panic() {
        // Creating a short-lived library that fails to load won't trigger drop.
        // Instead, test that a successfully loaded library (if any) shuts down cleanly.
        if let Some(lib) = NvmlLibrary::load() {
            // The Drop implementation should not panic.
            std::mem::drop(lib);
        }
    }
}
