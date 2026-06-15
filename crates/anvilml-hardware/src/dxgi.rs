/// DXGI-based GPU enumeration for Windows.
///
/// This detector uses the Windows Display Graphics Interface (DXGI) to
/// enumerate physical GPUs on the system. It initialises COM, creates a
/// DXGI factory, and iterates adapters to extract device metadata
/// (name, vendor ID, device ID, dedicated video memory).
///
/// This is the primary GPU detection path on Windows systems. It does not
/// require the Vulkan loader — it works with any GPU that has a Windows
/// display driver.
///
/// **Hard constraints:** Never panic on COM initialisation failure or
/// missing adapters. Always return an empty list when detection is
/// unavailable.
pub struct DxgiDetector;

impl DxgiDetector {
    /// Construct a new `DxgiDetector`.
    ///
    /// This is a zero-sized unit struct — no allocation or state is required.
    pub const fn new() -> Self {
        DxgiDetector
    }
}

impl Default for DxgiDetector {
    fn default() -> Self {
        Self::new()
    }
}

use anvilml_core::{CapabilitySource, DeviceType, EnumerationSource, GpuDevice, InferenceCaps};

use crate::DeviceDetector;

impl DeviceDetector for DxgiDetector {
    /// Enumerate available GPUs via the DXGI interface.
    ///
    /// Initialises COM, creates a DXGI factory, and iterates adapters
    /// to extract device metadata for each GPU found.
    ///
    /// # Errors
    ///
    /// Returns `Ok(vec![])` when COM initialisation fails or the DXGI
    /// factory cannot be created. The method never propagates an error —
    /// it treats DXGI unavailability as "no GPUs detected" rather than
    /// a hard failure.
    fn detect(&self) -> Result<Vec<GpuDevice>, anvilml_core::AnvilError> {
        // Initialise COM. DXGI requires COM to be initialised on the
        // calling thread before any factory can be created.
        // S_OK (0) means already initialised — we treat that as success.
        let hr = unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            )
        };

        // RPC_E_CHANGED_MODE means COM is already initialised with a different
        // concurrency model — treat as success; we can still use DXGI.
        const RPC_E_CHANGED_MODE: i32 = 0x80010106u32 as i32;
        if hr.is_err() && hr.0 != RPC_E_CHANGED_MODE {
            // 0x80010106 = RPC_E_CHANGED_MODE (already initialised with different mode).
            // Any other non-zero value means COM initialisation failed.
            tracing::debug!(hr = hr.0, "DXGI COM initialisation failed");
            return Ok(vec![]);
        }

        // Defer COM uninitialisation when this function returns.
        // CoUninitialize is safe to call even if CoInitializeEx returned
        // RPC_E_CHANGED_MODE — COM handles nested initialisation counts.
        let _com_guard = ComGuard;

        // Create the DXGI factory. IDXGIFactory1 supports adapter
        // enumeration starting from Windows Vista SP2.
        // We use CreateDXGIFactory1 which returns IDXGIFactory1.
        let factory: windows::Win32::Graphics::Dxgi::IDXGIFactory1 =
            match unsafe { windows::Win32::Graphics::Dxgi::CreateDXGIFactory1() } {
                Ok(factory) => factory,
                Err(err) => {
                    // DXGI factory creation failed — likely no display driver.
                    // Return empty list; the CPU fallback will handle this.
                    tracing::debug!(error = ?err, "DXGI factory creation failed");
                    return Ok(vec![]);
                }
            };

        let mut devices = Vec::new();
        let mut index: u32 = 0;

        // Enumerate adapters in a loop. EnumAdapters1 returns S_OK
        // while there are more adapters, and S_FALSE (0x80000001) when
        // no more adapters are available. Any other HRESULT is an error.
        loop {
            let adapter = match unsafe { factory.EnumAdapters1(index) } {
                Ok(adapter) => adapter,
                Err(err) => {
                    // DXGI_ERROR_NOT_FOUND (0x887A0002) or S_FALSE signals no more adapters.
                    // windows_core::Error exposes the HRESULT via .code().0
                    let code = err.code().0 as u32;
                    if code == 0x887A0002 || code == 0x80000001 {
                        break;
                    }
                    tracing::debug!(index, error = ?err, "DXGI EnumAdapters1 failed");
                    break;
                }
            };

            // Get the adapter description. DXGI_ADAPTER_DESC1 contains
            // the vendor ID, device ID, description string, and VRAM info.
            let mut desc = windows::Win32::Graphics::Dxgi::DXGI_ADAPTER_DESC1::default();
            if let Err(err) = unsafe { adapter.GetDesc1(&mut desc) } {
                tracing::debug!(index, error = ?err, "DXGI GetDesc1 failed");
                break;
            }

            // Skip software adapters (e.g. Microsoft Basic Render Driver).
            // DXGI_ADAPTER_FLAG_SOFTWARE = 2 identifies any software/render-only
            // adapter that has no physical GPU backing it. These are not valid
            // inference devices and must be excluded.
            if desc.Flags & 2 != 0 {
                tracing::debug!(
                    index,
                    name = %wstring_to_string(&desc.Description),
                    "skipping software adapter"
                );
                index += 1;
                continue;
            }

            // Convert the wide-character description string to a Rust String.
            // DXGI descriptions are null-terminated UTF-16LE.
            // Strip trailing nulls and decode.
            let description = wstring_to_string(&desc.Description);

            // Map the PCI vendor ID to a DeviceType variant.
            // 0x10de = NVIDIA → Cuda, 0x1002 = AMD → Rocm, else → Cpu.
            // This is the standard PCI vendor ID assignment used throughout
            // the crate (see vulkan.rs, cpu.rs for consistency).
            let device_type = match desc.VendorId {
                0x10de => DeviceType::Cuda,
                0x1002 => DeviceType::Rocm,
                _ => DeviceType::Cpu,
            };

            // Convert dedicated video memory from bytes to mebibytes (MiB).
            // DXGI reports VRAM in bytes; we divide by 1024*1024.
            let vram_total_mib = (desc.DedicatedVideoMemory / (1024 * 1024)) as u32;

            // Build the GpuDevice entry.
            // enumeration_source is set to DXGI since we enumerated via
            // the DXGI API. capabilities_source is Fallback because we
            // don't have inference capability data at detection time.
            let device = GpuDevice {
                index,
                name: description,
                device_type,
                vram_total_mib,
                vram_free_mib: vram_total_mib, // no device context for live read
                driver_version: "unknown".to_string(),
                pci_vendor_id: desc.VendorId as u16,
                pci_device_id: desc.DeviceId as u16,
                arch: None,
                caps: InferenceCaps::default(),
                enumeration_source: EnumerationSource::Dxgi,
                capabilities_source: CapabilitySource::Fallback,
            };

            // Log the detected device at INFO level per the mandatory
            // "each detected device" logging convention.
            tracing::info!(
                index = index,
                name = %device.name,
                device_type = ?device.device_type,
                vram_total_mib = vram_total_mib,
                fp8 = false,
                "gpu device detected via DXGI"
            );

            devices.push(device);
            index += 1;
        }

        // Log that DXGI detection completed at DEBUG level per the
        // mandatory "detection fallback used" logging convention.
        tracing::debug!(
            fallback = "dxgi",
            device_count = devices.len(),
            "DXGI detection completed"
        );

        Ok(devices)
    }

    /// Refresh the VRAM for the device at `index`.
    ///
    /// Returns `(0, 0)` because live VRAM queries via DXGI require a
    /// device context (ID3D11Device) which this task does not create.
    /// Live VRAM tracking is handled by NVML on NVIDIA systems.
    ///
    /// # Errors
    ///
    /// This method never returns an error.
    fn refresh_vram(&self, _index: u32) -> Result<(u32, u32), anvilml_core::AnvilError> {
        tracing::debug!(index = _index, "refresh_vram returns (0,0) for DXGI");
        Ok((0, 0))
    }
}

/// Helper to convert a null-terminated wide string (`u16` array) to a Rust `String`.
///
/// DXGI adapter descriptions are null-terminated UTF-16LE strings stored in
/// a fixed-size `[u16; 128]` array. This function finds the first null byte,
/// slices the array to that point, and decodes as UTF-16LE.
fn wstring_to_string(wstr: &[u16; 128]) -> String {
    // Find the first null byte to determine the string length.
    // DXGI descriptions are always null-terminated per the DXGI spec.
    let len = wstr.iter().position(|&c| c == 0).unwrap_or(128);
    // Decode as UTF-16LE. The wstring_to_string_lossy function replaces
    // invalid UTF-16 sequences with the replacement character.
    String::from_utf16_lossy(&wstr[..len])
}

/// RAII guard that calls `CoUninitialize()` when dropped.
///
/// DXGI requires COM to be initialised on the calling thread. This guard
/// ensures `CoUninitialize` is called when the detection function returns,
/// even if an early return or panic occurs.
///
/// **Note:** `CoUninitialize` is safe to call even if `CoInitializeEx`
/// returned `RPC_E_CHANGED_MODE` — COM handles nested initialisation
/// counts internally.
struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        // CoUninitialize is safe to call unconditionally.
        // If COM was never successfully initialised, this is a no-op.
        unsafe { windows::Win32::System::Com::CoUninitialize() };
    }
}
