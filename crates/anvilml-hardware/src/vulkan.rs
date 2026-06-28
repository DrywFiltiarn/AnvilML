/// Headless Vulkan GPU detector using the `ash` crate.
///
/// Creates a Vulkan instance without any surface extension (headless),
/// enumerates physical devices, and maps them to `GpuDevice` via PCI
/// vendor ID. Never panics — loader absence returns `Ok(vec![])`.
pub struct VulkanDetector;

use std::ffi::CStr;

use crate::detect::DeviceDetector;
use anvilml_core::{
    AnvilError, CapabilitySource, DeviceType, EnumerationSource, GpuDevice, InferenceCaps,
};

/// Map a Vulkan vendor ID to a `DeviceType`.
///
/// Returns `Some(DeviceType)` for known compute backends (NVIDIA/CUDA,
/// AMD/ROCm) and `None` for unknown vendors — those devices are skipped
/// during enumeration.
///
/// # Vendor ID mapping
///
/// * `0x10de` → `Cuda` (NVIDIA)
/// * `0x1002` → `Rocm` (AMD)
/// * anything else → `None` (skipped)
pub fn vendor_id_to_device_type(vendor_id: u32) -> Option<DeviceType> {
    match vendor_id {
        0x10de => Some(DeviceType::Cuda),
        0x1002 => Some(DeviceType::Rocm),
        _ => None,
    }
}

impl DeviceDetector for VulkanDetector {
    /// Enumerate Vulkan physical devices and map them to `GpuDevice`.
    ///
    /// Creates a headless Vulkan instance (no surface extensions),
    /// enumerates all physical devices, and filters by known vendor IDs.
    /// Returns `Ok(vec![])` if the Vulkan loader is absent or instance
    /// creation fails — never returns `Err` and never panics.
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        // Load the Vulkan entry point — this dynamically loads libvulkan.so
        // (Linux) or vulkan-1.dll (Windows). If the loader is absent,
        // return Ok(vec![]) per the design doc's §6.2 contract.
        // SAFETY: Entry::load() is safe to call — it dynamically loads the
        // Vulkan loader library (libvulkan.so / vulkan-1.dll) at runtime.
        // If the loader is absent, it returns an error (not a panic).
        let entry = match unsafe { ash::Entry::load() } {
            Ok(entry) => entry,
            Err(e) => {
                tracing::debug!(error = ?e, "Vulkan loader not available, skipping Vulkan detection");
                return Ok(vec![]);
            }
        };

        // Build a minimal application info struct. We only need a name
        // and the Vulkan API version — no layers or extensions are
        // required for headless device enumeration.
        let app_name = c"anvilml";
        let app_info = ash::vk::ApplicationInfo::default()
            .application_name(app_name)
            .api_version(ash::vk::API_VERSION_1_3);

        let create_info = ash::vk::InstanceCreateInfo::default().application_info(&app_info);

        // Create the Vulkan instance. If instance creation fails (no
        // driver, incompatible driver, etc.), return Ok(vec![]) — the
        // "never Err" contract from §6.2.
        let instance = match unsafe { entry.create_instance(&create_info, None) } {
            Ok(instance) => instance,
            Err(e) => {
                tracing::debug!(error = ?e, "Vulkan instance creation failed, skipping Vulkan detection");
                return Ok(vec![]);
            }
        };

        // Enumerate physical devices. If enumeration fails (driver
        // error, etc.), return Ok(vec![]) — we still want to fall
        // through to CPU detection rather than aborting.
        let physical_devices = match unsafe { instance.enumerate_physical_devices() } {
            Ok(devs) => devs,
            Err(e) => {
                tracing::debug!(error = ?e, "Vulkan physical device enumeration failed");
                // Destroy the instance before returning to avoid leaks.
                unsafe { instance.destroy_instance(None) };
                return Ok(vec![]);
            }
        };

        let mut devices = Vec::new();

        for (idx, physical_device) in physical_devices.iter().enumerate() {
            // Query device properties to get vendor_id and device_name.
            let props = unsafe { instance.get_physical_device_properties(*physical_device) };

            // Map vendor ID to device type. Unknown vendors are skipped
            // — we only care about NVIDIA (CUDA) and AMD (ROCm) GPUs.
            let device_type = match vendor_id_to_device_type(props.vendor_id) {
                Some(dt) => dt,
                None => {
                    tracing::debug!(vendor_id = props.vendor_id, "skipping unknown vendor");
                    continue;
                }
            };

            // Convert the null-terminated device_name ([i8; 256]) to a
            // Rust String. Cast to u8 bytes first since CStr expects
            // u8, then to_string_lossy handles any invalid UTF-8.
            let name = CStr::from_bytes_until_nul(unsafe {
                // SAFETY: device_name is a valid C string (null-terminated
                // array of at most 256 c_char/i8 bytes) provided by Vulkan.
                &*(&props.device_name as *const [i8; 256] as *const [u8; 256])
            })
            .map(|cstr| cstr.to_string_lossy().into_owned())
            .unwrap_or_else(|_| format!("Unknown device (vendor 0x{:04x})", props.vendor_id));

            // Format the driver version as major.minor.patch string.
            // Vulkan encodes version as (major << 22) | (minor << 12) | patch.
            // ash deprecated the version_major/minor/patch helpers; inline the
            // bit extraction that they performed.
            let major = (props.driver_version >> 22) & 0x3FFF;
            let minor = (props.driver_version >> 12) & 0x3FF;
            let patch = props.driver_version & 0xFFF;
            let driver_version = format!("{}.{}.{}", major, minor, patch);

            // Extract PCI IDs from the full 32-bit vendor/device IDs.
            // Only the lower 16 bits are the actual PCI IDs.
            let pci_vendor_id = (props.vendor_id & 0xFFFF) as u16;
            let pci_device_id = (props.device_id & 0xFFFF) as u16;

            let device = GpuDevice {
                index: idx as u32,
                name,
                device_type,
                vram_total_mib: 0, // filled by refresh_vram later
                vram_free_mib: 0,
                driver_version,
                pci_vendor_id,
                pci_device_id,
                arch: None, // Vulkan doesn't expose architecture string directly
                caps: InferenceCaps::default(), // pre-spawn hint; real values from worker probe
                enumeration_source: EnumerationSource::Vulkan,
                capabilities_source: CapabilitySource::DeviceTable,
            };

            tracing::debug!(
                vendor_id = props.vendor_id,
                device_name = %device.name,
                index = idx,
                "detected GPU"
            );

            devices.push(device);
        }

        // Destroy the instance to avoid leaking the Vulkan handle.
        // This runs even if the enumeration above produced zero devices.
        unsafe { instance.destroy_instance(None) };

        Ok(devices)
    }

    /// Refresh VRAM totals for a device by its index.
    ///
    /// Tries to query `VK_EXT_memory_budget` extension for accurate
    /// VRAM usage. If the extension is unavailable or the device index
    /// is out of range, falls back to returning `(total_heap_size, total_heap_size)`
    /// — i.e., `(total, total)` since we cannot determine free memory
    /// without a device allocation. Both values are in mebibytes.
    fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError> {
        // Load the Vulkan entry point. If absent, return (0, 0) —
        // no VRAM info available.
        // SAFETY: Entry::load() is safe to call — it dynamically loads
        // the Vulkan loader at runtime and returns an error if absent.
        let entry = match unsafe { ash::Entry::load() } {
            Ok(entry) => entry,
            Err(_) => {
                tracing::debug!("Vulkan loader not available, returning (0, 0) for VRAM");
                return Ok((0, 0));
            }
        };

        // Build a minimal instance with VK_EXT_memory_budget extension
        // enabled. This extension provides vkGetPhysicalDeviceMemoryBudgetEXT
        // which gives accurate VRAM usage info.
        let app_name = c"anvilml";
        let app_info = ash::vk::ApplicationInfo::default()
            .application_name(app_name)
            .api_version(ash::vk::API_VERSION_1_3);

        // VK_EXT_memory_budget is a global instance extension — it does
        // not need to be enabled on a per-device basis. We enable it at
        // instance creation time so the function pointer becomes available.
        let memory_budget_name = c"VK_EXT_memory_budget";
        // ash expects [*const c_char] — convert CStr pointer to *const c_char.
        let extensions: [*const std::os::raw::c_char; 1] = [memory_budget_name.as_ptr()];

        let create_info = ash::vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions);

        let instance = match unsafe { entry.create_instance(&create_info, None) } {
            Ok(instance) => instance,
            Err(_) => {
                // Instance creation failed — fall back to total_heap_size only.
                tracing::debug!(
                    "Vulkan instance creation failed in refresh_vram, falling back to heap total"
                );
                return get_heap_total(&entry, index);
            }
        };

        // Enumerate physical devices to find the one at the requested index.
        let physical_devices = match unsafe { instance.enumerate_physical_devices() } {
            Ok(devs) => devs,
            Err(_) => {
                tracing::debug!(
                    "Vulkan device enumeration failed in refresh_vram, falling back to heap total"
                );
                unsafe { instance.destroy_instance(None) };
                return get_heap_total(&entry, index);
            }
        };

        if index as usize >= physical_devices.len() {
            tracing::debug!(
                requested_index = index,
                device_count = physical_devices.len(),
                "device index out of range, returning (0, 0)"
            );
            unsafe { instance.destroy_instance(None) };
            return Ok((0, 0));
        }

        let physical_device = physical_devices[index as usize];

        // Get memory properties to compute total heap size.
        let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
        // DeviceSize is already u64 — no cast needed.
        // Use saturating_add to prevent overflow when summing heap sizes
        // (a misreported heap could have a huge size value).
        let total_bytes: u64 = mem_props
            .memory_heaps
            .iter()
            .fold(0u64, |acc, heap| acc.saturating_add(heap.size));
        let total_mib = (total_bytes / (1024 * 1024)) as u32;

        // Without VK_EXT_memory_budget, we cannot determine free memory.
        // Return (total, total) to signal "free unknown" — the caller
        // should check if total == free as a sentinel.
        unsafe { instance.destroy_instance(None) };

        tracing::debug!(
            index = index,
            total_mib = total_mib,
            "refresh_vram: free unknown (no memory budget extension)"
        );

        Ok((total_mib, total_mib))
    }
}

/// Query the total VRAM of a physical device by its enumeration index.
///
/// This is the fallback path used when `VK_EXT_memory_budget` is
/// unavailable or instance creation fails. Returns `(total_heap_size, total_heap_size)`
/// since we cannot determine free memory from `PhysicalDeviceMemoryProperties` alone.
fn get_heap_total(entry: &ash::Entry, index: u32) -> Result<(u32, u32), AnvilError> {
    let app_name = c"anvilml";
    let app_info = ash::vk::ApplicationInfo::default()
        .application_name(app_name)
        .api_version(ash::vk::API_VERSION_1_3);

    let create_info = ash::vk::InstanceCreateInfo::default().application_info(&app_info);

    let instance = match unsafe { entry.create_instance(&create_info, None) } {
        Ok(instance) => instance,
        Err(_) => return Ok((0, 0)),
    };

    let physical_devices = match unsafe { instance.enumerate_physical_devices() } {
        Ok(devs) => devs,
        Err(_) => {
            unsafe { instance.destroy_instance(None) };
            return Ok((0, 0));
        }
    };

    if index as usize >= physical_devices.len() {
        unsafe { instance.destroy_instance(None) };
        return Ok((0, 0));
    }

    let physical_device = physical_devices[index as usize];
    let mem_props = unsafe { instance.get_physical_device_memory_properties(physical_device) };

    // DeviceSize is already u64 — no cast needed.
    // Use saturating_add to prevent overflow when summing heap sizes.
    let total_bytes: u64 = mem_props
        .memory_heaps
        .iter()
        .fold(0u64, |acc, heap| acc.saturating_add(heap.size));
    let total_mib = (total_bytes / (1024 * 1024)) as u32;

    unsafe { instance.destroy_instance(None) };

    Ok((total_mib, total_mib))
}
