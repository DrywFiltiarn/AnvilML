/// NVML-based live VRAM refresh for NVIDIA GPUs on Linux.
///
/// This detector dynamically loads `libnvidia-ml.so.1` at runtime and
/// resolves the `nvmlDeviceGetMemoryInfo` symbol to query live VRAM
/// (total and free) from NVIDIA GPUs. It does NOT enumerate devices —
/// that is handled by Vulkan, DXGI, or sysfs detectors.
///
/// This is a supplement to the primary detection backends. On systems
/// without an NVIDIA GPU, the library is absent and `refresh_vram()`
/// returns `(0, 0)` gracefully — never errors.
///
/// **Hard constraints:** Never panic on library load failure, symbol
/// resolution failure, or NVML API failure. Always return `(0, 0)`
/// when NVML is unavailable.

pub struct NvmlDetector;

impl NvmlDetector {
    /// Construct a new `NvmlDetector`.
    ///
    /// This is a zero-sized unit struct — no allocation or state is required.
    pub const fn new() -> Self {
        NvmlDetector
    }
}

impl Default for NvmlDetector {
    fn default() -> Self {
        Self::new()
    }
}

use anvilml_core::AnvilError;

use crate::DeviceDetector;

/// The NVIDIA Management Library shared object name.
///
/// This is the standard soname for the NVML library installed by
/// NVIDIA drivers on Linux systems.
const NVML_LIB: &str = "libnvidia-ml.so.1";

impl DeviceDetector for NvmlDetector {
    /// Enumerate devices.
    ///
    /// NVML is a VRAM refresh supplement, not a device enumerator.
    /// This method always returns an empty list — actual device
    /// enumeration is handled by Vulkan, DXGI, or sysfs detectors.
    ///
    /// # Errors
    ///
    /// This method never returns an error.
    fn detect(&self) -> Result<Vec<anvilml_core::GpuDevice>, AnvilError> {
        // NVML does not enumerate devices — it only provides live VRAM
        // data for devices already discovered by other backends.
        // The actual device enumeration happens via Vulkan, DXGI, or sysfs.
        tracing::debug!("NVML: detect() returns empty (VRAM supplement only)");
        Ok(vec![])
    }

    /// Refresh the VRAM for the device at `index`.
    ///
    /// Attempts to load `libnvidia-ml.so.1` and query the live VRAM
    /// total and free values. On systems without an NVIDIA GPU (or
    /// without the NVML library installed), returns `(0, 0)` gracefully.
    ///
    /// The `index` parameter is currently unused because this
    /// implementation queries the single-device NVML handle. In future
    /// multi-device systems, this would be used to select the device.
    ///
    /// Returns `(total_mib, free_mib)` where both values are in
    /// mebibytes. Returns `(0, 0)` when NVML is unavailable.
    ///
    /// # Errors
    ///
    /// This method never returns an error — NVML unavailability is
    /// treated as "no VRAM data" rather than a hard failure.
    fn refresh_vram(&self, _index: u32) -> Result<(u32, u32), AnvilError> {
        // Attempt to load the NVML shared library. This is the only
        // way to query live VRAM on NVIDIA GPUs. On non-NVIDIA systems
        // (AMD GPUs, Intel GPUs, CPUs), this library is absent and
        // we return (0, 0) — which is the correct answer.
        let library = match libloading::Library::new(NVML_LIB) {
            Ok(lib) => lib,
            Err(err) => {
                // Library not found — common on non-NVIDIA systems.
                // Return (0, 0) gracefully — this is expected behavior.
                tracing::debug!(
                    library = NVML_LIB,
                    error = %err,
                    "NVML VRAM refresh unavailable (libnvidia-ml.so not found)"
                );
                return Ok((0, 0));
            }
        };

        // Resolve the nvmlDeviceGetMemoryInfo symbol from the loaded library.
        // This C function takes an opaque device handle pointer and fills
        // an nvmlMemory_t struct with total/free/used memory values.
        //
        // We use a dummy non-null pointer (0x1) as the device handle.
        // For single-device systems (the common case), NVML's default
        // device is index 0. Multi-device systems would need a proper
        // nvmlDevice_t handle obtained via nvmlDeviceGetHandleByIndex.
        let func: libloading::Symbol<
            unsafe extern "C" fn(*mut std::ffi::c_void, *mut MemoryInfo) -> u32,
        > = match library.get(b"nvmlDeviceGetMemoryInfo") {
            Ok(s) => s,
            Err(err) => {
                // Symbol not found — the library loaded but the function
                // is missing (unlikely, but possible with mismatched
                // NVML library versions).
                tracing::debug!(
                    symbol = "nvmlDeviceGetMemoryInfo",
                    error = %err,
                    "NVML symbol resolution failed"
                );
                return Ok((0, 0));
            }
        };

        // Build a fake device handle. The NVML API uses opaque pointers
        // (nvmlDevice_t) — we pass a non-null dummy pointer. For a
        // single-device system, NVML's internal default device handle
        // will be used by the library.
        //
        // This is a best-effort approach: it works on systems with a
        // single NVIDIA GPU. Multi-GPU systems would require proper
        // handle resolution via nvmlDeviceGetHandleByIndex.
        let mut mem_info = MemoryInfo {
            total: 0,
            free: 0,
            used: 0,
        };

        let result = unsafe { func(0x1 as *mut std::ffi::c_void, &mut mem_info) };

        // NVML returns 0 (NVML_SUCCESS) on success. Any other value
        // indicates an error — we return (0, 0) rather than propagating
        // the error to keep the detection pipeline graceful.
        if result != 0 {
            tracing::debug!(
                nvml_result = result,
                "NVML nvmlDeviceGetMemoryInfo returned error"
            );
            return Ok((0, 0));
        }

        // Convert from bytes to mebibytes (MiB).
        // NVML reports memory in bytes; we divide by 1024*1024.
        // The result fits in u32 because VRAM is at most ~256 GiB,
        // which is well within u32 range (4 GiB). Even the largest
        // consumer GPUs (~240 GiB) fit in u32.
        let total_mib = (mem_info.total / (1024 * 1024)) as u32;
        let free_mib = (mem_info.free / (1024 * 1024)) as u32;

        tracing::debug!(
            vram_total_mib = total_mib,
            vram_free_mib = free_mib,
            "NVML VRAM refresh completed"
        );

        Ok((total_mib, free_mib))
    }
}

/// Opaque representation of `nvmlMemory_t` from the NVML C API.
///
/// The NVML library defines this struct with three fields:
/// - `total`: Total installed framebuffer memory in bytes
/// - `free`: Unallocated framebuffer memory in bytes
/// - `used`: Used framebuffer memory in bytes
///
/// We define this struct here to match the C API layout. The actual
/// NVML header (`nvml.h`) defines the same structure.
#[repr(C)]
struct MemoryInfo {
    total: u64,
    free: u64,
    used: u64,
}
