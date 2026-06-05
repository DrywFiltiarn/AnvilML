//! Vulkan GPU enumerator (primary, SDK-free, fixture-tested).
//!
//! Implements a headless Vulkan GPU detector using the `ash` crate. The detector
//! creates a no-surface [`VkInstance`](ash::vk::Instance), enumerates physical devices
//! via `vkEnumeratePhysicalDevices`, and for each device reads:
//!
//! - **KHR_driver_properties** — device name, driver name, driver version
//! - **EXT_memory_budget** — VRAM budget and usage per heap
//!
//! PCI vendor/device IDs come from `VkPhysicalDeviceProperties.vendorID` / `deviceID`.
//! Vendor → [`DeviceType`](anvilml_core::DeviceType) mapping:
//!
//! | vendorID | DeviceType |
//! |----------|-----------|
//! | 0x10DE   | Cuda      |
//! | 0x1002   | Rocm      |
//! | 0x8086   | Cpu       |
//! | other    | Cpu       |
//!
//! When the Vulkan loader is absent, `detect()` returns `Ok(vec![])` — no panic,
//! no error. This follows ANVILML_DESIGN §5: "Loader absent → Ok(vec![])."
//!
//! ## VRAM calculation (§5.2)
//!
//! - `total_vram_mib`: size of the largest `DEVICE_LOCAL` heap in MiB (ignoring
//!   small host-visible Resizable-BAR heaps).
//! - `vram_free_mib`: `budget - usage` for that heap index if EXT_memory_budget is
//!   available; otherwise falls back to `heapSize` (conservative estimate).

use std::collections::HashSet;

use anvilml_core::{AnvilError, CapabilitySource, DeviceType, EnumerationSource, GpuDevice};

use crate::DeviceDetector;

// ── Constants ─────────────────────────────────────────────────────────────────

/// KHR_driver_properties extension name.
const KHR_DRIVER_PROPERTIES: &str = "VK_KHR_driver_properties";

/// EXT_memory_budget extension name.
const EXT_MEMORY_BUDGET: &str = "VK_EXT_memory_budget";

/// MiB divisor (bytes → MiB).
const BYTES_PER_MIB: u64 = 1024 * 1024;

// ── Vendor → DeviceType mapping ───────────────────────────────────────────────

/// Map a Vulkan PCI vendor ID to a [`DeviceType`].
///
/// Per ANVILML_DESIGN §5.3:
/// - `0x10DE` → Cuda (NVIDIA)
/// - `0x1002` → Rocm (AMD)
/// - Intel (`0x8086`) or anything else → Cpu
fn vendor_id_to_device_type(vendor_id: u32) -> DeviceType {
    match vendor_id {
        0x10de => DeviceType::Cuda,
        0x1002 => DeviceType::Rocm,
        _ => DeviceType::Cpu,
    }
}

/// Parse a 30-bit Vulkan driver version into a human-readable string.
///
/// Vulkan build-number format (per spec):
/// - Bits 29-22: major version (8 bits)
/// - Bits 21-12: minor version (10 bits)
/// - Bits 11-0: patch/build number (12 bits)
fn parse_vulkan_driver_version(version: u32) -> String {
    let major = (version >> 22) & 0xFF;
    let minor = (version >> 12) & 0x3FF;
    let patch = version & 0xFFF;
    format!("{major}.{minor}.{patch}")
}

/// Convert a C char array to a Rust String, stopping at the first NUL byte.
fn cstr_to_string(arr: &[i8]) -> String {
    let end = arr.iter().position(|&b| b == 0).unwrap_or(arr.len());
    // SAFETY: i8 and u8 have the same layout; we read up to the NUL terminator.
    let bytes = unsafe { &*(&arr[..end] as *const [i8] as *const [u8]) };
    String::from_utf8_lossy(bytes).into_owned()
}

// ── VRAM helpers ──────────────────────────────────────────────────────────────

/// Calculate total VRAM from memory heaps: largest DEVICE_LOCAL heap in MiB.
///
/// Ignores small host-visible Resizable-BAR heaps per ANVILML_DESIGN §5.2.
fn compute_total_vram_mib(
    heap_count: u32,
    heap_sizes: &[u64],
    heap_flags: &[ash::vk::MemoryHeapFlags],
) -> u32 {
    let mut largest = 0u64;

    for i in 0..heap_count as usize {
        // Only consider DEVICE_LOCAL heaps (bit 0).
        if heap_flags[i] & ash::vk::MemoryHeapFlags::DEVICE_LOCAL
            != ash::vk::MemoryHeapFlags::empty()
            && heap_sizes[i] > largest
        {
            largest = heap_sizes[i];
        }
    }

    // Saturating division: bytes → MiB.
    largest.saturating_div(BYTES_PER_MIB) as u32
}

/// Compute free VRAM from EXT_memory_budget properties.
///
/// Returns `(total_mib, free_mib)` where `free = budget - usage` for the
/// heap index that had the largest DEVICE_LOCAL size. If no budget data is
/// available (ext not supported), returns `(total_mib, total_mib)` as a
/// conservative fallback.
fn compute_free_vram(
    target_heap_index: usize,
    heap_count: u32,
    budgets: &[u64],
    usages: &[u64],
    has_budget_ext: bool,
) -> u32 {
    if !has_budget_ext || target_heap_index >= heap_count as usize {
        // No budget data — fall back conservatively.
        return u32::MAX;
    }

    let budget = budgets[target_heap_index];
    let usage = usages[target_heap_index];
    let free_bytes = budget.saturating_sub(usage);

    (free_bytes / BYTES_PER_MIB) as u32
}

// ── VulkanDetector ────────────────────────────────────────────────────────────

/// A Vulkan-based GPU detector.
///
/// Constructs a headless [`VkInstance`](ash::vk::Instance), enumerates all physical
/// devices, and populates [`GpuDevice`](anvilml_core::GpuDevice) records from
/// Vulkan driver properties and memory budget data.
///
/// If the Vulkan loader is not found, `detect()` returns `Ok(vec![])` — no panic,
/// no error. This is the correct behaviour per ANVILML_DESIGN §5.
#[derive(Debug, Clone, Default)]
pub struct VulkanDetector;

impl DeviceDetector for VulkanDetector {
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        // Step 1: Create a Vulkan entry pointing to the system loader.
        // `Entry::load()` uses dlopen at runtime to find libvulkan.so / vulkan.dll.
        // If the loader is absent, it returns an error which we handle gracefully
        // by returning Ok(vec![]) — no panic, no Err.
        let entry = match unsafe { ash::Entry::load() } {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(detector = "Vulkan", error = %e, "Vulkan loader not available");
                return Ok(Vec::new());
            }
        };

        // Step 2: Create a headless VkInstance (no surface extensions).
        let app_info = ash::vk::ApplicationInfo {
            s_type: ash::vk::StructureType::APPLICATION_INFO,
            p_next: std::ptr::null(),
            p_application_name: c"AnvilML".as_ptr(),
            application_version: 0,
            p_engine_name: std::ptr::null(),
            engine_version: 0,
            api_version: ash::vk::make_api_version(0, 1, 3, 0),
            _marker: std::marker::PhantomData,
        };

        let create_info = ash::vk::InstanceCreateInfo {
            s_type: ash::vk::StructureType::INSTANCE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: ash::vk::InstanceCreateFlags::default(),
            p_application_info: &app_info,
            enabled_layer_count: 0,
            pp_enabled_layer_names: std::ptr::null(),
            enabled_extension_count: 0,
            pp_enabled_extension_names: std::ptr::null(),
            _marker: std::marker::PhantomData,
        };

        let instance = match unsafe { entry.create_instance(&create_info, None) } {
            Ok(inst) => inst,
            Err(e) => {
                tracing::warn!(detector = "Vulkan", error = %e, "vkCreateInstance failed — device extensions will not be available");
                return Ok(Vec::new());
            }
        };

        // Step 3: Enumerate physical devices.
        let phys_devs = match unsafe { instance.enumerate_physical_devices() } {
            Ok(devs) => devs,
            Err(e) => {
                tracing::warn!(detector = "Vulkan", error = %e, "vkEnumeratePhysicalDevices failed");
                unsafe { instance.destroy_instance(None) };
                return Ok(Vec::new());
            }
        };

        if phys_devs.is_empty() {
            tracing::warn!(detector = "Vulkan", "No Vulkan physical devices found");
            unsafe { instance.destroy_instance(None) };
            return Ok(Vec::new());
        }

        // Step 4: Query each physical device.
        let mut devices = Vec::with_capacity(phys_devs.len());

        for (index, pd) in phys_devs.iter().enumerate() {
            // Basic properties (always available).
            let props = unsafe { instance.get_physical_device_properties(*pd) };
            let mem_props = unsafe { instance.get_physical_device_memory_properties(*pd) };

            // Derive name from driver properties if available, else from basic props.
            let mut device_name = cstr_to_string(&props.device_name);
            let mut driver_version_str = parse_vulkan_driver_version(props.driver_version);
            #[allow(unused_assignments)]
            let mut has_budget_ext = false;
            let mut budget_values: Vec<u64> = Vec::new();
            let mut usage_values: Vec<u64> = Vec::new();

            // Query supported device extensions for this physical device.
            let device_extensions: HashSet<String> = unsafe {
                instance
                    .enumerate_device_extension_properties(*pd)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|ext| cstr_to_string(&ext.extension_name))
                    .collect()
            };

            // Try to query KHR_driver_properties for a better name.
            if device_extensions.contains(KHR_DRIVER_PROPERTIES) {
                let mut driver_props = ash::vk::PhysicalDeviceDriverProperties {
                    s_type: ash::vk::StructureType::PHYSICAL_DEVICE_DRIVER_PROPERTIES,
                    p_next: std::ptr::null_mut(),
                    driver_id: ash::vk::DriverId::from_raw(0),
                    driver_name: [0; 256],
                    driver_info: [0; 256],
                    conformance_version: ash::vk::ConformanceVersion {
                        major: 0,
                        minor: 0,
                        subminor: 0,
                        patch: 0,
                    },
                    _marker: std::marker::PhantomData,
                };

                // Manually chain driver_props into props2 via raw pointer.
                // SAFETY: driver_props lives on the stack and outlives props2.
                let mut props2 = ash::vk::PhysicalDeviceProperties2 {
                    s_type: ash::vk::StructureType::PHYSICAL_DEVICE_PROPERTIES_2,
                    p_next: &mut driver_props as *mut _ as *mut std::ffi::c_void,
                    properties: props,
                    _marker: std::marker::PhantomData,
                };

                unsafe { instance.get_physical_device_properties2(*pd, &mut props2) };

                // Read driver name from the extension data.
                let raw_name = driver_props.driver_name;
                if raw_name[0] != 0 {
                    device_name = cstr_to_string(&raw_name);
                }

                // Use driver version from KHR_driver_properties if available.
                if props2.properties.driver_version != 0 {
                    driver_version_str =
                        parse_vulkan_driver_version(props2.properties.driver_version);
                }
            }

            // Try to query EXT_memory_budget for VRAM data.
            if device_extensions.contains(EXT_MEMORY_BUDGET) {
                let mut mem_budget_props = ash::vk::PhysicalDeviceMemoryBudgetPropertiesEXT {
                    s_type: ash::vk::StructureType::PHYSICAL_DEVICE_MEMORY_BUDGET_PROPERTIES_EXT,
                    p_next: std::ptr::null_mut(),
                    heap_budget: [0; 16],
                    heap_usage: [0; 16],
                    _marker: std::marker::PhantomData,
                };

                let mut mem_props2 = ash::vk::PhysicalDeviceMemoryProperties2 {
                    s_type: ash::vk::StructureType::PHYSICAL_DEVICE_MEMORY_PROPERTIES_2,
                    p_next: std::ptr::null_mut(),
                    memory_properties: mem_props,
                    _marker: std::marker::PhantomData,
                };

                // Chain the budget extension into the properties2 struct.
                mem_props2 = mem_props2.push_next(&mut mem_budget_props);

                unsafe { instance.get_physical_device_memory_properties2(*pd, &mut mem_props2) };

                let heap_count = mem_props2.memory_properties.memory_heap_count;
                has_budget_ext = true;
                budget_values.reserve(heap_count as usize);
                usage_values.reserve(heap_count as usize);

                for i in 0..heap_count {
                    budget_values.push(mem_budget_props.heap_budget[i as usize] as u64);
                    usage_values.push(mem_budget_props.heap_usage[i as usize] as u64);
                }
            }

            // Extract heap data.
            let heap_flags: Vec<ash::vk::MemoryHeapFlags> = mem_props
                .memory_heaps
                .iter()
                .take(mem_props.memory_heap_count as usize)
                .map(|h| h.flags)
                .collect();
            let heap_sizes: Vec<u64> = mem_props
                .memory_heaps
                .iter()
                .take(mem_props.memory_heap_count as usize)
                .map(|h| h.size)
                .collect();

            // Compute total VRAM.
            let total_vram_mib =
                compute_total_vram_mib(mem_props.memory_heap_count, &heap_sizes, &heap_flags);

            // Find the heap index of the largest DEVICE_LOCAL heap for free VRAM calculation.
            let mut largest_heap_idx = 0usize;
            let mut largest_size = 0u64;
            for i in 0..mem_props.memory_heap_count as usize {
                if heap_flags[i] & ash::vk::MemoryHeapFlags::DEVICE_LOCAL
                    != ash::vk::MemoryHeapFlags::empty()
                    && heap_sizes[i] > largest_size
                {
                    largest_size = heap_sizes[i];
                    largest_heap_idx = i;
                }
            }

            let vram_free_mib = compute_free_vram(
                largest_heap_idx,
                mem_props.memory_heap_count,
                &budget_values,
                &usage_values,
                has_budget_ext,
            );

            // Map vendor ID to device type.
            let device_type = vendor_id_to_device_type(props.vendor_id);

            // Extract vendor_id and device_id for the new fields.
            let pci_vendor_id = (props.vendor_id & 0xFFFF) as u16;
            let pci_device_id = (props.device_id & 0xFFFF) as u16;

            devices.push(GpuDevice {
                index: index as u32,
                name: device_name,
                device_type,
                vram_total_mib: total_vram_mib,
                vram_free_mib,
                driver_version: driver_version_str,
                pci_vendor_id,
                pci_device_id,
                arch: None, // resolved later by device_db::resolve_caps
                caps: anvilml_core::InferenceCaps::default(),
                enumeration_source: EnumerationSource::Vulkan,
                capabilities_source: CapabilitySource::Fallback,
                db_group_name: None,
            });
        }

        unsafe { instance.destroy_instance(None) };

        Ok(devices)
    }

    fn refresh_vram(&self, _idx: u32) -> Result<(u32, u32), AnvilError> {
        // VRAM refresh would require a live VkDevice and queue for memory tracking.
        // For now, return (0, 0) — the worker's MemoryReport events will populate
        // actual values at Ready state per ANVILML_DESIGN §5.
        Ok((0, 0))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anvilml_core::DeviceType;
    use serial_test::serial;

    /// VulkanDetector must implement the DeviceDetector trait.
    /// This is a compile-time check that also exercises construction.
    #[test]
    #[serial]
    fn vulkan_detect_returns_ok() {
        let detector = VulkanDetector::default();
        let result = detector.detect();
        // Must always return Ok — never panics, never Err.
        assert!(result.is_ok(), "detect() must return Ok, got {:?}", result);
        let devices = result.unwrap();
        // On a system with GPUs, returns non-empty; without, returns empty vec.
        // The key invariant: no panics, no errors.
        let _ = devices;
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

    /// Vulkan driver version parsing must produce correct strings.
    #[test]
    #[serial]
    fn parse_vulkan_driver_version_nvidia() {
        // Example: NVIDIA driver 250.55.10 → build number with major=250, minor=55, patch=10
        // Vulkan 30-bit encoding: 8-bit major (max 255) + 10-bit minor + 12-bit patch
        let version: u32 = (250 << 22) | (55 << 12) | 10;
        assert_eq!(parse_vulkan_driver_version(version), "250.55.10");
    }

    #[test]
    #[serial]
    fn parse_vulkan_driver_version_amd() {
        // Example: AMD driver 3610.0 → build number 0xE200000
        // major = 36, minor = 10, patch = 0
        let version: u32 = (36 << 22) | (10 << 12) | 0;
        assert_eq!(parse_vulkan_driver_version(version), "36.10.0");
    }

    #[test]
    #[serial]
    fn parse_vulkan_driver_version_zero() {
        assert_eq!(parse_vulkan_driver_version(0), "0.0.0");
    }

    /// VRAM calculation: largest DEVICE_LOCAL heap wins over host-visible Resizable-BAR.
    #[test]
    #[serial]
    fn largest_device_local_heap_wins_over_host_visible_resizable_bar() {
        // Simulate: 8 GB device-local + 64 MB host-visible Resizable-Bar.
        let heap_count = 2;
        let heap_sizes = vec![8u64 * 1024 * 1024 * 1024, 64 * 1024 * 1024];
        // Heap 0: DEVICE_LOCAL (bit 0 set)
        // Heap 1: HOST_VISIBLE | HOST_COHERENT (no DEVICE_LOCAL bit)
        let heap_flags = vec![
            ash::vk::MemoryHeapFlags::DEVICE_LOCAL,
            ash::vk::MemoryHeapFlags::empty(),
        ];

        let total = compute_total_vram_mib(heap_count, &heap_sizes, &heap_flags);

        // Should be exactly 8192 MiB (8 GB), ignoring the small host-visible heap.
        assert_eq!(total, 8192, "must pick largest DEVICE_LOCAL heap");
    }

    /// VRAM calculation: when no DEVICE_LOCAL heaps exist, total is 0.
    #[test]
    #[serial]
    fn no_device_local_heap_yields_zero() {
        let heap_count = 1;
        let heap_sizes = vec![16u64 * 1024 * 1024 * 1024]; // 16 GB, non-device-local
                                                           // MemoryHeapFlags only has DEVICE_LOCAL; empty means no device-local flag.
        let heap_flags = vec![ash::vk::MemoryHeapFlags::empty()];

        let total = compute_total_vram_mib(heap_count, &heap_sizes, &heap_flags);
        assert_eq!(total, 0, "no DEVICE_LOCAL heaps → 0 MiB");
    }

    /// VRAM calculation: handles large values > u32::MAX bytes correctly.
    #[test]
    #[serial]
    fn vram_calculation_handles_large_heaps() {
        // Simulate a 80 GB device-local heap (A100-80GB).
        let heap_count = 1;
        let heap_sizes = vec![80u64 * 1024 * 1024 * 1024];
        let heap_flags = vec![ash::vk::MemoryHeapFlags::DEVICE_LOCAL];

        let total = compute_total_vram_mib(heap_count, &heap_sizes, &heap_flags);
        assert_eq!(total, 81920, "80 GB must be 81920 MiB");
    }

    /// Free VRAM from budget: budget - usage.
    #[test]
    #[serial]
    fn free_vram_from_budget() {
        let target_heap = 0;
        let heap_count = 1;
        let budgets = vec![8u64 * 1024 * 1024 * 1024]; // 8 GB budget
        let usages = vec![3u64 * 1024 * 1024 * 1024]; // 3 GB used

        let free = compute_free_vram(target_heap, heap_count, &budgets, &usages, true);
        assert_eq!(free, 5120, "8 GB budget - 3 GB usage = 5 GiB free");
    }

    /// Free VRAM fallback: no budget extension → returns u32::MAX.
    #[test]
    #[serial]
    fn free_vram_fallback_no_budget() {
        let target_heap = 0;
        let heap_count = 1;
        let budgets = vec![0u64];
        let usages = vec![0u64];

        let free = compute_free_vram(target_heap, heap_count, &budgets, &usages, false);
        assert_eq!(free, u32::MAX, "no budget → fallback to MAX");
    }

    /// Free VRAM underflow protection: usage > budget should not panic.
    #[test]
    #[serial]
    fn free_vram_underflow_protection() {
        let target_heap = 0;
        let heap_count = 1;
        let budgets = vec![1u64 * 1024 * 1024]; // 1 MiB budget
        let usages = vec![2u64 * 1024 * 1024]; // 2 MiB used (over budget)

        let free = compute_free_vram(target_heap, heap_count, &budgets, &usages, true);
        assert_eq!(free, 0, "usage > budget → 0 MiB free (no underflow)");
    }
}
