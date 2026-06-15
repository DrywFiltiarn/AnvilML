//! Vulkan-based GPU enumeration using the `ash` crate.
//!
//! This detector loads the Vulkan loader at runtime via `dlopen` (no SDK
//! required), creates a minimal Vulkan instance with no extensions, enumerates
//! physical devices, and extracts device metadata (name, driver version, VRAM).
//!
//! The detector is the primary GPU detection path on both Linux and Windows.
//! It returns `Ok(vec![])` gracefully when the Vulkan loader is absent or
//! instance creation fails — never panics.

pub struct VulkanDetector;

impl VulkanDetector {
    /// Construct a new `VulkanDetector`.
    ///
    /// This is a zero-sized unit struct — no allocation or state is required.
    pub const fn new() -> Self {
        VulkanDetector
    }
}

impl Default for VulkanDetector {
    fn default() -> Self {
        Self::new()
    }
}

use anvilml_core::{CapabilitySource, DeviceType, EnumerationSource, GpuDevice, InferenceCaps};

use ash::vk;

use crate::DeviceDetector;

impl DeviceDetector for VulkanDetector {
    /// Enumerate available Vulkan GPUs.
    ///
    /// Loads the Vulkan loader at runtime, creates an instance, enumerates
    /// physical devices, and extracts device metadata for each GPU found.
    ///
    /// # Errors
    ///
    /// Returns `Ok(vec![])` (empty list) when the Vulkan loader is absent,
    /// instance creation fails, or physical device enumeration fails. The
    /// method never propagates an error — it treats Vulkan unavailability
    /// as "no GPUs detected" rather than a hard failure.
    fn detect(&self) -> Result<Vec<GpuDevice>, anvilml_core::AnvilError> {
        // Load the Vulkan entry point. ash::Entry::load() dlopen's the
        // Vulkan loader at runtime, so this works without a compile-time
        // link to libvulkan.so (SDK-free approach).
        let entry = match unsafe { ash::Entry::load() } {
            Ok(entry) => entry,
            Err(err) => {
                // Vulkan loader not found (common in CI, WSL2 without GPU).
                // Return empty list — the CPU fallback will handle this.
                tracing::debug!(error = ?err, "Vulkan loader not available");
                return Ok(vec![]);
            }
        };

        // Build a minimal Vulkan application info. We target Vulkan 1.0
        // because that is sufficient for all required queries (properties,
        // memory) and maximises compatibility with older drivers.
        // ash 0.38 uses struct literal syntax (no builder pattern).
        let app_info = vk::ApplicationInfo {
            s_type: vk::StructureType::APPLICATION_INFO,
            p_next: std::ptr::null(),
            p_application_name: std::ptr::null(),
            application_version: 0,
            p_engine_name: std::ptr::null(),
            engine_version: 0,
            api_version: vk::API_VERSION_1_0,
            _marker: std::marker::PhantomData,
        };

        let create_info = vk::InstanceCreateInfo {
            s_type: vk::StructureType::INSTANCE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::InstanceCreateFlags::default(),
            p_application_info: &app_info,
            enabled_layer_count: 0,
            pp_enabled_layer_names: std::ptr::null(),
            enabled_extension_count: 0,
            pp_enabled_extension_names: std::ptr::null(),
            _marker: std::marker::PhantomData,
        };

        let instance = match unsafe { entry.create_instance(&create_info, None) } {
            Ok(instance) => instance,
            Err(err) => {
                // Instance creation failed — no GPUs will be detected.
                // This is expected on systems without Vulkan drivers.
                tracing::debug!(error = ?err, "Vulkan instance creation failed");
                return Ok(vec![]);
            }
        };

        // Enumerate physical devices (GPUs) visible to the Vulkan instance.
        let physical_devices = match unsafe { instance.enumerate_physical_devices() } {
            Ok(devs) => devs,
            Err(err) => {
                // Enumeration failed — treat as no GPUs.
                tracing::debug!(error = ?err, "Vulkan physical device enumeration failed");
                return Ok(vec![]);
            }
        };

        if physical_devices.is_empty() {
            // No GPUs on the system — the CPU fallback will handle this.
            return Ok(vec![]);
        }

        // Process each physical GPU and build a GpuDevice entry.
        let mut devices = Vec::with_capacity(physical_devices.len());

        for (dev_index, physical_device) in physical_devices.iter().enumerate() {
            // Query device properties: driver version, device name, vendor/device IDs.
            // These methods are unsafe in ash because they call Vulkan C API functions
            // that require valid handles — we hold the instance alive via borrow.
            let props = unsafe { instance.get_physical_device_properties(*physical_device) };

            // Convert the driver version from Vulkan's packed u32 format
            // (major << 22 | minor << 12 | patch) into a human-readable string.
            // Using the non-deprecated api_version_* functions from ash.
            let driver_version = format!(
                "{}.{}.{}",
                vk::api_version_major(props.driver_version),
                vk::api_version_minor(props.driver_version),
                vk::api_version_patch(props.driver_version),
            );

            // Convert the device name from a null-terminated [i8; 256] array
            // to a Rust String. Find the first null byte and slice to it.
            // c_char is i8 on Linux; cast to u8 for UTF-8 decoding.
            let device_name = cstr_from_i8bytes(&props.device_name);

            // Map the PCI vendor ID to a DeviceType variant.
            // 0x10de = NVIDIA → Cuda, 0x1002 = AMD → Rocm, else → Cpu.
            // This is the standard PCI vendor ID assignment.
            let device_type = match props.vendor_id {
                0x10de => DeviceType::Cuda,
                0x1002 => DeviceType::Rocm,
                _ => DeviceType::Cpu,
            };

            // Query memory properties to find the largest DEVICE_LOCAL heap.
            // DEVICE_LOCAL heaps are dedicated GPU memory (VRAM).
            // We take the largest one because a GPU may have multiple memory
            // heaps (e.g. dedicated + shared), and the dedicated heap is the
            // one that counts as VRAM.
            let mem_props =
                unsafe { instance.get_physical_device_memory_properties(*physical_device) };
            let vram_total_mib = largest_device_local_heap_mib(&mem_props);

            // Set enumeration_source to Vulkan and capabilities_source to
            // Fallback — actual inference capabilities are reported by the
            // Python worker at Ready, not at detection time.
            let device = GpuDevice {
                index: dev_index as u32,
                name: device_name,
                db_name: None,
                device_type,
                vram_total_mib,
                vram_free_mib: vram_total_mib, // best-effort: no device context exists
                driver_version,
                pci_vendor_id: props.vendor_id as u16,
                pci_device_id: props.device_id as u16,
                arch: None,
                caps: InferenceCaps::default(),
                enumeration_source: EnumerationSource::Vulkan,
                capabilities_source: CapabilitySource::Fallback,
            };

            // Log the detected device at INFO level per the mandatory
            // "each detected device" logging convention.
            tracing::info!(
                index = dev_index,
                name = %device.name,
                device_type = ?device.device_type,
                vram_total_mib = vram_total_mib,
                fp8 = false,
                "gpu device detected via Vulkan"
            );

            devices.push(device);
        }

        Ok(devices)
    }

    /// Refresh the VRAM for the device at `index`.
    ///
    /// Returns `(0, 0)` because live VRAM queries require a Vulkan device
    /// context (queue, command buffer) which this task does not create.
    /// Live VRAM tracking is handled by NVML (NVIDIA) or similar APIs
    /// in subsequent tasks.
    ///
    /// # Errors
    ///
    /// This method never returns an error.
    fn refresh_vram(&self, _index: u32) -> Result<(u32, u32), anvilml_core::AnvilError> {
        tracing::debug!(index = _index, "refresh_vram returns (0,0) for Vulkan");
        Ok((0, 0))
    }
}

/// Convert a null-terminated `[i8; 256]` (C string) to a Rust `String`.
///
/// Finds the first null byte, slices the array to that point, casts each
/// byte to `u8`, and decodes as UTF-8. Uses `String::from_utf8_lossy` as
/// a fallback for non-UTF-8 device names (should not happen in practice).
fn cstr_from_i8bytes(bytes: &[i8; 256]) -> String {
    // Find the first null byte to determine the string length.
    // Vulkan device names are always null-terminated per the spec.
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(256);
    // Cast i8 bytes to u8, then decode as UTF-8.
    // Vulkan device names are UTF-8 encoded per the Vulkan spec.
    let u8_slice: &[u8] = unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u8, len) };
    String::from_utf8_lossy(u8_slice).into_owned()
}

/// Find the largest DEVICE_LOCAL memory heap and return its size in MiB.
///
/// DEVICE_LOCAL heaps are dedicated GPU memory (VRAM). A physical device
/// may have multiple memory heaps; the largest DEVICE_LOCAL one is the
/// GPU's dedicated VRAM capacity. Returns 0 if no DEVICE_LOCAL heap exists.
///
/// MemoryHeap.size is a `DeviceSize` (u64) per the Vulkan spec.
fn largest_device_local_heap_mib(mem_props: &vk::PhysicalDeviceMemoryProperties) -> u32 {
    let mut largest_mib: u64 = 0;

    for heap in mem_props.memory_heaps_as_slice() {
        // Only consider heaps marked as DEVICE_LOCAL (dedicated GPU memory).
        // MemoryHeapFlags::DEVICE_LOCAL is bit 0.
        if heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL) {
            let heap_mib = heap.size / (1024 * 1024);
            if heap_mib > largest_mib {
                largest_mib = heap_mib;
            }
        }
    }

    largest_mib as u32
}
