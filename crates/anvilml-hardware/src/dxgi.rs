/// Windows DXGI GPU detector using the `windows` crate.
///
/// Creates a DXGI factory via `CreateDXGIFactory1`, enumerates all adapters
/// via `IDXGIFactory1::EnumAdapters1`, and maps each adapter to a `GpuDevice`
/// using the shared `vendor_id_to_device_type` function.
///
/// This detector serves as the Windows fallback when Vulkan enumeration
/// returns empty — per §6.4 of the design doc, the detection priority chain
/// is: Vulkan → DXGI → CPU.
///
/// # Error resilience
///
/// `detect()` **never** panics and **never** returns `Err`. If `CreateDXGIFactory1`
/// fails (access denied, COM initialization failure, etc.), the function logs
/// at DEBUG level and returns `Ok(vec![])`. Each adapter's `GetDesc1` is also
/// individually guarded — a failure on one adapter skips that adapter only.
pub struct DxgiDetector;

use windows::Win32::Graphics::Dxgi::*;

use crate::detect::DeviceDetector;
use crate::vendor_id_to_device_type;
use anvilml_core::{AnvilError, CapabilitySource, EnumerationSource, GpuDevice, InferenceCaps};

/// Enumerate Windows DXGI adapters and map them to `GpuDevice`.
///
/// Creates a DXGI factory, enumerates all adapters, and filters by known
/// vendor IDs (NVIDIA/CUDA, AMD/ROCm). Returns `Ok(vec![])` if the DXGI
/// factory cannot be created — never returns `Err` and never panics.
///
/// # DXGI API contract
///
/// 1. `CreateDXGIFactory1` obtains an `IDXGIFactory1` — the entry point
///    to all DXGI enumeration.
/// 2. `EnumAdapters1(i)` is called in a loop from `i = 0`; when it returns
///    an error, no more adapters exist.
/// 3. `GetDesc1()` on each adapter yields a `DXGI_ADAPTER_DESC1` containing
///    `VendorId`, `Description`, and memory fields.
///
/// # PCI device ID limitation
///
/// `DXGI_ADAPTER_DESC1` (returned by `GetDesc1`) does not include a
/// `DeviceId` field — only the older `DXGI_ADAPTER_DESC` (returned by
/// `GetDesc`) has it. We set `pci_device_id = 0` as a known limitation;
/// the `AdapterLuid` could serve as a unique identifier if needed in the future.
fn detect(_this: &DxgiDetector) -> Result<Vec<GpuDevice>, AnvilError> {
    // Create the DXGI factory — this is the COM-based entry point for
    // all GPU enumeration on Windows. If it fails (COM not initialized,
    // access denied, etc.), return Ok(vec![]) per the "never panic" contract.
    let factory: IDXGIFactory1 = match unsafe { CreateDXGIFactory1() } {
        Ok(factory) => factory,
        Err(e) => {
            tracing::debug!(error = ?e, "DXGI factory creation failed, skipping DXGI detection");
            return Ok(vec![]);
        }
    };

    let mut devices = Vec::new();

    // Enumerate adapters by index. EnumAdapters1 returns an error when
    // there are no more adapters — this is the documented termination
    // condition, not an error to report.
    let mut i = 0;
    loop {
        let adapter = match unsafe { factory.EnumAdapters1(i as u32) } {
            Ok(adapter) => adapter,
            Err(_) => {
                // No more adapters — this is the normal termination condition.
                break;
            }
        };

        // Get the adapter description. On failure, skip this adapter
        // and continue to the next one — a single bad adapter should not
        // abort the entire enumeration.
        let desc = match unsafe { adapter.GetDesc1() } {
            Ok(desc) => desc,
            Err(e) => {
                tracing::debug!(
                    adapter_index = i,
                    error = ?e,
                    "failed to get adapter description, skipping"
                );
                i += 1;
                continue;
            }
        };

        // Map vendor ID to device type. Unknown vendors (Intel, etc.)
        // are skipped — we only target NVIDIA (CUDA) and AMD (ROCm).
        let device_type = match vendor_id_to_device_type(desc.VendorId) {
            Some(dt) => dt,
            None => {
                tracing::debug!(
                    vendor_id = desc.VendorId,
                    adapter_index = i,
                    "skipping unknown vendor"
                );
                i += 1;
                continue;
            }
        };

        // Convert the u16[128] Description field to a Rust String.
        // Find the first null byte and decode as UTF-16LE. If no null
        // byte is found, use the full 128 elements. If decoding fails,
        // fall back to "Unknown device".
        let device_name = desc_to_string(&desc.Description);

        // Extract PCI vendor ID from the lower 16 bits of VendorId.
        // This matches the masking convention used in VulkanDetector.
        let pci_vendor_id = (desc.VendorId & 0xFFFF) as u16;

        // pci_device_id: DXGI_ADAPTER_DESC1 (from GetDesc1) does not
        // include DeviceId — only the older DXGI_ADAPTER_DESC (from
        // GetDesc) has it. Set to 0 as a known limitation.
        let pci_device_id: u16 = 0;

        let device = GpuDevice {
            index: i as u32,
            name: device_name,
            device_type,
            vram_total_mib: 0, // filled by refresh_vram later
            vram_free_mib: 0,
            driver_version: String::new(),
            pci_vendor_id,
            pci_device_id,
            arch: None,
            caps: InferenceCaps::default(), // pre-spawn hint
            enumeration_source: EnumerationSource::Dxgi,
            capabilities_source: CapabilitySource::DeviceTable,
        };

        tracing::debug!(
            vendor_id = desc.VendorId,
            device_name = %device.name,
            index = i,
            "detected GPU via DXGI"
        );

        devices.push(device);
        i += 1;
    }

    Ok(devices)
}

/// Refresh VRAM totals for a device by its index.
///
/// DXGI does not provide a direct VRAM query API equivalent to Vulkan's
/// `VK_EXT_memory_budget` extension. Returns `(0, 0)` — signaling "unknown"
/// to the caller. This is consistent with the design: VRAM refresh is
/// best-effort, and `(0, 0)` is the sentinel for "cannot determine."
fn refresh_vram(_this: &DxgiDetector, _index: u32) -> Result<(u32, u32), AnvilError> {
    // DXGI has no VRAM query API — return (0, 0) as the "unknown" sentinel.
    // This matches the Vulkan fallback when memory budget is unavailable.
    tracing::debug!("DXGI has no VRAM query API, returning (0, 0)");
    Ok((0, 0))
}

/// Convert a `DXGI_ADAPTER_DESC1::Description` (`u16[128]`) to a `String`.
///
/// Finds the first null byte and decodes the preceding elements as UTF-16LE.
/// If no null byte is found, uses the full 128 elements. If UTF-16 decoding
/// fails, returns `"Unknown device"`.
fn desc_to_string(desc: &[u16; 128]) -> String {
    // Find the first null terminator in the wide string. If no null byte
    // exists, use all 128 elements (unlikely but handle gracefully).
    let len = desc.iter().position(|&c| c == 0).unwrap_or(128);
    let slice = &desc[..len];

    // Decode as UTF-16LE. If the adapter name contains invalid UTF-16
    // sequences (extremely rare), use lossy decoding to produce a valid
    // Rust String rather than panicking.
    String::from_utf16_lossy(slice)
}

impl DeviceDetector for DxgiDetector {
    /// Enumerate Windows DXGI adapters and map them to `GpuDevice`.
    ///
    /// See the module-level `detect()` function for full details.
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        detect(self)
    }

    /// Refresh VRAM for a DXGI device.
    ///
    /// DXGI has no VRAM query API — always returns `(0, 0)`.
    fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError> {
        refresh_vram(self, index)
    }
}
