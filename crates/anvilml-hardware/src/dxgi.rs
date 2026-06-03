//! DXGI GPU enumerator (Windows).
//!
//! Implements a DXGI-based GPU detector using the `winapi` crate's COM interop.
//! The detector creates an [`IDXGIFactory1`](winapi::shared::dxgi::IDXGIFactory1)
//! via [`CreateDXGIFactory1`](winapi::shared::dxgi::CreateDXGIFactory1), enumerates
//! adapters via `EnumAdapters`, and for each adapter reads:
//!
//! - **Adapter name** from `DXGI_ADAPTER_DESC::Description`
//! - **Vendor/Device IDs** from `DXGI_ADAPTER_DESC::{VendorId, DeviceId}`
//! - **Dedicated video memory** from `DXGI_ADAPTER_DESC::DedicatedVideoMemory`
//!
//! Vendor → [`DeviceType`](anvilml_core::DeviceType) mapping:
//!
//! | vendorID | DeviceType |
//! |----------|-----------|
//! | 0x10DE   | Cuda      |
//! | 0x1002   | Rocm      |
//! | other    | Cpu       |
//!
//! When DXGI is unavailable or COM initialization fails, `detect()` returns
//! `Ok(vec![])` — no panic, no error. This follows ANVILML_DESIGN §5:
//! "Loader absent → Ok(vec![])."

#![cfg(windows)]

use anvilml_core::{AnvilError, CapabilitySource, DeviceType, EnumerationSource, GpuDevice};

use crate::DeviceDetector;

// ── Constants ─────────────────────────────────────────────────────────────────

/// MiB divisor (bytes → MiB).
const BYTES_PER_MIB: u64 = 1024 * 1024;

/// NVIDIA PCI vendor ID.
const VENDOR_NVIDIA: u32 = 0x10de;

/// AMD PCI vendor ID.
const VENDOR_AMD: u32 = 0x1002;

// ── Vendor → DeviceType mapping ───────────────────────────────────────────────

/// Map a PCI vendor ID to a [`DeviceType`].
///
/// Per ANVILML_DESIGN §5.3:
/// - `0x10DE` → Cuda (NVIDIA)
/// - `0x1002` → Rocm (AMD)
/// - Intel or anything else → Cpu
pub fn vendor_id_to_device_type(vendor_id: u32) -> DeviceType {
    match vendor_id {
        VENDOR_NVIDIA => DeviceType::Cuda,
        VENDOR_AMD => DeviceType::Rocm,
        _ => DeviceType::Cpu,
    }
}

// ── COM initialization guard ──────────────────────────────────────────────────

/// One-shot COM initialization guard.
///
/// Ensures `CoInitializeEx` is called exactly once per process thread before
/// any DXGI/COM calls are made. If initialization fails, the guard records the
/// error and subsequent calls return an error instead of panicking.
#[derive(Debug, Clone, Default)]
struct ComGuard {
    initialized: std::cell::Cell<bool>,
    last_error: std::cell::Cell<Option<i32>>,
}

impl ComGuard {
    fn ensure(&self) -> Result<(), i32> {
        if self.initialized.get() {
            if let Some(err) = self.last_error.get() {
                return Err(err);
            }
            return Ok(());
        }

        // S_OK = 0. We ignore S_FALSE (already initialized) as success.
        let hr = unsafe {
            winapi::um::combaseapi::CoInitializeEx(
                std::ptr::null_mut(),
                winapi::um::objbase::COINIT_APARTMENTTHREADED,
            )
        };

        if hr == 0 || hr == winapi::shared::winerror::S_OK {
            self.initialized.set(true);
            Ok(())
        } else if hr == winapi::shared::winerror::S_FALSE {
            // Already initialized — treat as success.
            self.initialized.set(true);
            Ok(())
        } else {
            self.last_error.set(Some(hr));
            Err(hr)
        }
    }
}

// ── DxgiDetector ──────────────────────────────────────────────────────────────

/// A DXGI-based GPU detector.
///
/// Constructs an [`IDXGIFactory1`](winapi::shared::dxgi::IDXGIFactory1), enumerates
/// all adapters, and populates [`GpuDevice`](anvilml_core::GpuDevice) records
/// from adapter description data.
///
/// If DXGI is not available or COM initialization fails, `detect()` returns
/// `Ok(vec![])` — no panic, no error.
#[derive(Debug, Clone, Default)]
pub struct DxgiDetector {
    com_guard: ComGuard,
}

impl DeviceDetector for DxgiDetector {
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        // Ensure COM is initialized on this thread.
        if let Err(hr) = self.com_guard.ensure() {
            log::warn!("DXGI: CoInitializeEx failed (hr=0x{hr:x}), skipping enumeration");
            return Ok(Vec::new());
        }

        // Create IDXGIFactory1.
        let mut factory: *mut winapi::shared::dxgi::IDXGIFactory1 = std::ptr::null_mut();
        let hr = unsafe {
            winapi::shared::dxgi::CreateDXGIFactory1(
                &winapi::shared::dxgi::IID_IDXGIFactory1,
                &mut factory as *mut _ as *mut _,
            )
        };

        if hr != 0 {
            log::warn!("DXGI: CreateDXGIFactory1 failed (hr=0x{hr:x}), skipping enumeration");
            return Ok(Vec::new());
        }

        // SAFETY: factory is a valid vtable pointer from CreateDXGIFactory1.
        let factory_ref = unsafe { &*factory };

        let mut devices = Vec::new();
        let mut idx: u32 = 0;

        loop {
            let mut adapter: *mut winapi::shared::dxgi::IDXGIAdapter = std::ptr::null_mut();
            let hr = unsafe { factory_ref.EnumAdapters(idx, &mut adapter) };

            if hr == winapi::shared::winerror::DXGI_ERROR_NOT_FOUND {
                break;
            }

            if hr != 0 {
                log::warn!("DXGI: EnumAdapters({idx}) failed (hr=0x{hr:x}), skipping");
                idx += 1;
                continue;
            }

            // SAFETY: adapter is valid from successful EnumAdapters call.
            let adapter_ref = unsafe { &*adapter };

            // Get adapter description (DXGI_ADAPTER_DESC).
            let mut desc: winapi::shared::dxgi::DXGI_ADAPTER_DESC = unsafe { std::mem::zeroed() };
            let hr = unsafe { adapter_ref.GetDesc(&mut desc) };

            if hr != 0 {
                log::warn!(
                    "DXGI: GetDesc for adapter {} failed (hr=0x{hr:x}), skipping",
                    idx
                );
                idx += 1;
                continue;
            }

            // Extract name from Description field (WCHAR array).
            let device_name = {
                let end = desc
                    .Description
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(desc.Description.len());
                String::from_utf16_lossy(&desc.Description[..end])
            };

            // Map vendor ID to device type.
            let device_type = vendor_id_to_device_type(desc.VendorId);

            // VRAM from DedicatedVideoMemory (bytes → MiB).
            let vram_total_mib = (desc.DedicatedVideoMemory as u64 / BYTES_PER_MIB) as u32;

            let pci_vendor_id = (desc.VendorId & 0xFFFF) as u16;
            let pci_device_id = (desc.DeviceId & 0xFFFF) as u16;

            devices.push(GpuDevice {
                index: idx,
                name: device_name,
                device_type,
                vram_total_mib,
                vram_free_mib: u32::MAX, // DXGI doesn't expose per-app VRAM usage.
                driver_version: String::new(),
                pci_vendor_id,
                pci_device_id,
                arch: None, // resolved later by device_db::resolve_caps
                caps: anvilml_core::InferenceCaps::default(),
                enumeration_source: EnumerationSource::Dxgi,
                capabilities_source: CapabilitySource::Fallback,
            });

            idx += 1;
        }

        Ok(devices)
    }

    fn refresh_vram(&self, _idx: u32) -> Result<(u32, u32), AnvilError> {
        // DXGI doesn't expose per-app VRAM usage in this API path.
        Ok((0, 0))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anvilml_core::DeviceType;
    use serial_test::serial;

    /// DxgiDetector must implement the DeviceDetector trait.
    #[test]
    #[serial]
    fn dxgi_detect_returns_ok() {
        let detector = DxgiDetector::default();
        let result = detector.detect();
        // Must always return Ok — never panics, never Err.
        assert!(result.is_ok(), "detect() must return Ok, got {:?}", result);
    }

    /// Vendor ID mapping must produce correct DeviceType values.
    #[test]
    #[serial]
    fn vendor_id_maps_cuda() {
        assert_eq!(vendor_id_to_device_type(0x10de), DeviceType::Cuda);
    }

    #[test]
    #[serial]
    fn vendor_id_maps_rocm() {
        assert_eq!(vendor_id_to_device_type(0x1002), DeviceType::Rocm);
    }

    #[test]
    #[serial]
    fn vendor_id_maps_cpu_intel() {
        assert_eq!(vendor_id_to_device_type(0x8086), DeviceType::Cpu);
    }

    #[test]
    #[serial]
    fn vendor_id_maps_cpu_unknown() {
        assert_eq!(vendor_id_to_device_type(0xDEAD), DeviceType::Cpu);
    }
}
