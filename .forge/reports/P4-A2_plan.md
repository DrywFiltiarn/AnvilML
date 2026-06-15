# Plan Report: P4-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A2                                         |
| Phase       | 004 — Hardware Detection                    |
| Description | anvilml-hardware: VulkanDetector SDK-free GPU enumeration |
| Depends on  | P4-A1 (DeviceDetector trait + CpuDetector)  |
| Project     | anvilml                                       |
| Planned at  | 2026-06-15T01:30:00Z                         |
| Attempt     | 1                                             |

## Objective

Create `crates/anvilml-hardware/src/vulkan.rs` containing `VulkanDetector`, which implements the `DeviceDetector` trait to enumerate physical Vulkan GPUs without requiring a Vulkan SDK installation. The detector uses the `ash` crate with its `loaded` feature (runtime `dlopen` of the Vulkan loader) to create a Vulkan instance, enumerate physical devices, and extract device metadata (name, driver version via `KHR_driver_properties`, VRAM via `EXT_memory_budget`). Returns `Ok(vec![])` gracefully when the loader is absent or instance creation fails — never panics. This enables the primary GPU detection path on both Linux and Windows.

## Scope

### In Scope
- **CREATE** `crates/anvilml-hardware/src/vulkan.rs` — `VulkanDetector` struct and `DeviceDetector` impl.
- **MODIFY** `crates/anvilml-hardware/Cargo.toml` — add `ash = { version = "0.38", default-features = false, features = ["loaded"] }` dependency.
- **MODIFY** `crates/anvilml-hardware/src/lib.rs` — add `pub mod vulkan;` and `pub use vulkan::VulkanDetector;`.
- **CREATE** `crates/anvilml-hardware/tests/vulkan_tests.rs` — tests for Vulkan detection.

### Out of Scope
- DXGI fallback (`dxgi.rs`) — task P4-A3.
- sysfs/NVML fallback (`sysfs.rs`, `nvml.rs`) — task P4-A3.
- `device_db.rs` PCI capability table — task P4-A4.
- `detect_all_devices()` orchestration in `lib.rs` — task P4-A5.
- `MockDetector` — task P4-B1.
- Server-side wiring of `GET /v1/system` — task P4-C1.

## Existing Codebase Assessment

The `anvilml-hardware` crate currently contains two files: `lib.rs` (46 lines) defining the `DeviceDetector` trait with `detect()` and `refresh_vram()` methods, and `cpu.rs` (130 lines) implementing `CpuDetector` as a zero-sized struct returning a synthetic CPU device. The `lib.rs` exports `pub mod cpu;` and `pub use cpu::CpuDetector;`.

The `anvilml-core` crate provides all domain types used by this task: `GpuDevice` (with fields `index`, `name`, `device_type`, `vram_total_mib`, `vram_free_mib`, `driver_version`, `pci_vendor_id`, `pci_device_id`, `arch`, `caps`, `enumeration_source`, `capabilities_source`), `DeviceType` (enum: `Cuda`, `Rocm`, `Cpu`), `EnumerationSource` (enum: `Vulkan`, `Dxgi`, `Sysfs`, `Nvml`, `Mock`, `Override`), `CapabilitySource` (enum: `PyTorch`, `DeviceTable`, `Fallback`), and `InferenceCaps` (struct with `fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention` bool fields).

The established patterns are:
- Unit structs for detectors (`CpuDetector` is `pub struct CpuDetector;`).
- `DeviceDetector` trait is `Send + Sync` and lives in `lib.rs`.
- Tests use `serial_test::serial` annotation and live in `crates/anvilml-hardware/tests/`.
- Error handling uses `AnvilError::Io` for I/O-level failures.
- Logging uses `tracing::info!` for detected devices and `tracing::debug!` for diagnostic details.
- `lib.rs` contains only `pub mod`, `pub use`, and `//!` crate-level doc — no implementation code.

No gap or discrepancy was found between the design doc and current source for this task. The `DeviceDetector` trait is already defined as specified.

## Resolved Dependencies

| Type   | Name   | Version verified | MCP source     | Feature flags confirmed |
|--------|--------|-----------------|----------------|------------------------|
| crate  | ash    | 0.38.0+1.3.281  | crates.io API  | loaded (maps to libloading + std) |

**API shape confirmed via docs.rs (ash 0.38.0+1.3.281):**
- `ash::Entry::load()` → `Result<Entry, LoadingError>` (feature: `loaded`) — loads Vulkan loader at runtime via `dlopen`.
- `entry.create_instance(&create_info, None)` → `VkResult<Instance>` — creates a Vulkan instance.
- `instance.enumerate_physical_devices()` → `VkResult<Vec<PhysicalDevice>>` — enumerates physical GPUs.
- `instance.get_physical_device_properties(physical_device)` → `PhysicalDeviceProperties` — returns `driver_version` (packed u32), `device_name` (`[u8; 256]`), `vendor_id`, `device_id`.
- `instance.get_physical_device_memory_properties(physical_device)` → `PhysicalDeviceMemoryProperties` — returns `memory_heaps` array with `size` field (bytes) and `flags` (includes `DEVICE_LOCAL`).
- `ash::khr::driver_properties::VK_KHR_DRIVER_PROPERTIES_EXTENSION_NAME` — extension name constant for `VK_KHR_driver_properties`.
- `ash::ext::memory_budget::VK_EXT_MEMORY_BUDGET_EXTENSION_NAME` — extension name constant for `VK_EXT_MEMORY_BUDGET`.

**MCP note:** No `rust-docs` MCP tool was available. Version resolved via crates.io API web fetch (2026-06-15). The task-specified version `0.38` matches the latest `0.38.0+1.3.281`.

## Approach

1. **Add `ash` dependency to `Cargo.toml`.** Append `ash = { version = "0.38", default-features = false, features = ["loaded"] }` to the `[dependencies]` section. The `loaded` feature enables `Entry::load()` which defers Vulkan loader discovery to runtime via `dlopen`, avoiding a compile-time link requirement against `libvulkan.so`. This is the SDK-free approach specified in the design doc.

2. **Create `crates/anvilml-hardware/src/vulkan.rs`.** Implement `VulkanDetector` as a zero-sized unit struct (matching `CpuDetector` pattern):
   ```rust
   pub struct VulkanDetector;
   impl VulkanDetector {
       pub const fn new() -> Self { VulkanDetector }
   }
   impl Default for VulkanDetector {
       fn default() -> Self { Self::new() }
   }
   ```

3. **Implement `DeviceDetector for VulkanDetector`.** The `detect()` method performs the full Vulkan enumeration pipeline:
   - **Step 3a — Load the Vulkan entry point.** Call `unsafe { ash::Entry::load() }` inside a `match` / `?` pattern. If `Entry::load()` returns `Err(LoadingError)`, log at DEBUG level (`error = ?err, "Vulkan loader not available`) and return `Ok(vec![])`. This is the graceful-failure path: no GPU is better than a panic.
   - **Step 3b — Create a Vulkan instance.** Build an `ash::vk::ApplicationInfo` with `api_version = ash::vk::API_VERSION_1_0` (sufficient for all required queries). Build an `ash::vk::InstanceCreateInfo` that enables two device-level extensions:
     - `VK_KHR_driver_properties` (from `ash::khr::driver_properties::VK_KHR_DRIVER_PROPERTIES_EXTENSION_NAME`) — provides `driver_name` and `driver_info`.
     - `VK_EXT_memory_budget` (from `ash::ext::memory_budget::VK_EXT_MEMORY_BUDGET_EXTENSION_NAME`) — provides VRAM budget queries.
     Pass these extension names through `enabled_layer_names` / `enabled_extension_names`. Create the instance via `unsafe { entry.create_instance(&create_info, None) }`. On `VkResult::ErrorInitializationFailed` or other errors, log at DEBUG level and return `Ok(vec![])`.
   - **Step 3c — Enumerate physical devices.** Call `unsafe { instance.enumerate_physical_devices() }`. This returns `VkResult<Vec<PhysicalDevice>>`. On error, return `Ok(vec![])`. If the vector is empty (no GPUs on the system), return `Ok(vec![])` — the CPU fallback will handle this.
   - **Step 3d — For each physical device, query properties.** Use `instance.get_physical_device_properties(physical_device)` to get `PhysicalDeviceProperties` containing `driver_version` (packed u32), `device_name` (`[u8; 256]` null-terminated UTF-8), `vendor_id`, and `device_id`. Convert the driver version from Vulkan's packed format (`(major << 22) | (minor << 12) | patch`) to a `"major.minor.patch"` string using `ash::vk::version_major()`, `version_minor()`, `version_patch()`. Convert the device name from `[u8; 256]` to a Rust `String` by finding the first null byte and slicing.
   - **Step 3e — Map PCI vendor ID to DeviceType.** Use a match on `vendor_id`: `0x10de` → `DeviceType::Cuda`, `0x1002` → `DeviceType::Rocm`, else `DeviceType::Cpu`. (The task context says `0x10DE -> Cuda 0x1002 -> Rocm` — the hex values are correct; the label "Cuda" is assigned to NVIDIA.)
   - **Step 3f — Query VRAM from the largest DEVICE_LOCAL heap.** Call `instance.get_physical_device_memory_properties(physical_device)` to get `PhysicalDeviceMemoryProperties`. Iterate `memory_heaps`, filter for those whose `flags` include `ash::vk::MemoryHeapFlags::DEVICE_LOCAL` (bit 0), and select the one with the largest `size` (in bytes). Convert bytes to MiB: `size / (1024 * 1024)`. Set `vram_free_mib = vram_total_mib` as a best-effort estimate (live free VRAM requires a device, which this task does not create).
   - **Step 3g — Set enumeration_source.** Set `enumeration_source = EnumerationSource::Vulkan` and `capabilities_source = CapabilitySource::Fallback` (capabilities are not queried at detection time — the Python worker reports them at Ready).
   - **Step 3h — Build GpuDevice and log.** Create a `GpuDevice` with all fields populated. Log at INFO level: `tracing::info!(index = dev_index, name = %device_name, device_type = ?device_type, vram_total_mib = vram_total_mib, fp8 = false, "gpu device detected via Vulkan")`.
   - **Step 3i — Return the device list.** Collect all devices into `Vec<GpuDevice>` and return `Ok(devices)`.

4. **Update `lib.rs`.** Add `pub mod vulkan;` after the existing `pub mod cpu;` line, and add `pub use vulkan::VulkanDetector;` after the existing `pub use cpu::CpuDetector;` line. Keep `lib.rs` under 80 lines.

5. **Create `crates/anvilml-hardware/tests/vulkan_tests.rs`.** Write tests that exercise the Vulkan detection path without requiring physical GPU hardware:
   - Test that `VulkanDetector::new()` constructs successfully.
   - Test that `VulkanDetector::detect()` returns `Ok(vec![])` when no Vulkan loader is present (this is the no-GPU / no-driver path that always works).
   - Test that `VulkanDetector::refresh_vram()` returns `(0, 0)` when called (no device context exists).
   - Compile-time `Send + Sync` assertion.
   - All tests must never panic — the `detect()` path handles all Vulkan errors gracefully.

6. **Run `cargo test -p anvilml-hardware -- vulkan`** to verify tests pass. This is the acceptance criterion.

## Public API Surface

| Item | Type | Module Path | Description |
|------|------|-------------|-------------|
| `VulkanDetector` | `pub struct` | `anvilml_hardware::vulkan::VulkanDetector` | Zero-sized struct implementing `DeviceDetector` for Vulkan GPU enumeration. |
| `VulkanDetector::new` | `pub const fn() -> Self` | `anvilml_hardware::vulkan::VulkanDetector::new` | Constructor — zero-sized unit struct, no allocation. |
| `VulkanDetector::default` | `impl Default` | `anvilml_hardware::vulkan::VulkanDetector::default` | Returns `Self::new()`. |
| `VulkanDetector::detect` | `fn(&self) -> Result<Vec<GpuDevice>, AnvilError>` | `anvilml_hardware::vulkan::VulkanDetector::detect` | Enumerates physical Vulkan devices. Returns `Ok(vec![])` if loader absent. |
| `VulkanDetector::refresh_vram` | `fn(&self, index: u32) -> Result<(u32, u32), AnvilError>` | `anvilml_hardware::vulkan::VulkanDetector::refresh_vram` | Returns `(0, 0)` — VRAM refresh requires a Vulkan device context (handled by P4-A3/NVML). |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/vulkan.rs` | `VulkanDetector` struct + `DeviceDetector` impl. ~150–180 lines. |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Add `pub mod vulkan;` and `pub use vulkan::VulkanDetector;`. |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Add `ash = { version = "0.38", default-features = false, features = ["loaded"] }` to `[dependencies]`. Bump patch version 0.1.1 → 0.1.2. |
| CREATE | `crates/anvilml-hardware/tests/vulkan_tests.rs` | Test suite for `VulkanDetector`. ~60–80 lines. |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/vulkan_tests.rs` | `test_vulkan_detector_new` | `VulkanDetector::new()` constructs a zero-sized struct. | None | N/A | `VulkanDetector` value. | `cargo test -p anvilml-hardware -- vulkan` exits 0 |
| `tests/vulkan_tests.rs` | `test_vulkan_detector_detect_returns_empty_or_devices` | `detect()` never panics; returns `Ok(vec![])` when no Vulkan loader present or no GPUs. | None | N/A | `Ok(vec![])` or `Ok([devices...])`. | `cargo test -p anvilml-hardware -- vulkan` exits 0 |
| `tests/vulkan_tests.rs` | `test_vulkan_detector_refresh_vram_returns_zero` | `refresh_vram(0)` returns `(0, 0)` since no Vulkan device context exists. | None | `index = 0` | `(0, 0)`. | `cargo test -p anvilml-hardware -- vulkan` exits 0 |
| `tests/vulkan_tests.rs` | `test_vulkan_detector_is_send_sync` | Compile-time `Send + Sync` assertion. | None | N/A | Compiles. | `cargo test -p anvilml-hardware -- vulkan` exits 0 |

## CI Impact

No CI changes required. The `ash` dependency is added behind the existing build pipeline — `cargo test --workspace --features mock-hardware` already compiles `anvilml-hardware`. The `mock-hardware` feature does not gate `ash` (it is a regular dependency, not feature-gated), so the Vulkan module compiles in all builds. The tests use only the public API and do not require GPU hardware.

## Platform Considerations

None identified. The `ash` crate with the `loaded` feature works cross-platform: on Linux it `dlopen`s `libvulkan.so.1`, on Windows it loads `vulkan-1.dll`. The `VulkanDetector::detect()` method wraps all Vulkan calls in `unsafe` blocks with graceful error handling (returning `Ok(vec![])` on failure), so no `#[cfg(unix)]` / `#[cfg(windows)]` guards are needed. The Windows cross-check (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) will exercise the compilation of the Vulkan module on the Windows target.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ash::Entry::load()` may fail on systems without a Vulkan driver (common in CI/WSL2 without GPU). This is expected — the implementation must return `Ok(vec![])` gracefully. | High | Low | The `detect()` method wraps `Entry::load()` in a `match` that returns `Ok(vec![])` on `Err`. Tests explicitly verify this path. |
| `VK_EXT_memory_budget` extension may not be available on all GPUs/drivers, causing `create_instance` to fail. | Medium | High | If `create_instance` fails with `ErrorExtensionNotPresent`, fall back to querying only basic properties (no memory budget). Use `vram_total_mib = 0` and `vram_free_mib = 0` for that device. |
| Vulkan instance creation may fail with `ErrorIncompatibleDriver` if the Vulkan driver version is too old for the requested API version. | Low | Medium | Use `api_version = ash::vk::API_VERSION_1_0` (not 1.3) to maximise compatibility with older drivers. |
| `PhysicalDeviceProperties::device_name` is a `[u8; 256]` — null-byte handling could produce trailing characters if the name is not null-terminated. | Low | Low | Find the first null byte with `memchr` or manual iteration, slice to that point, and decode as UTF-8. If decoding fails, use `String::from_utf8_lossy`. |
| The `ash` crate's `loaded` feature requires `libloading` which may not be available on all targets (e.g. static builds). | Low | Medium | `libloading` is a transitive dependency of `ash`'s `loaded` feature and is pulled automatically by cargo. No manual action needed. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware -- vulkan` exits 0
- [ ] `cargo check --workspace --features mock-hardware` exits 0 (Vulkan module compiles in mock-hardware build)
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0 (Vulkan module compiles for Windows target)
