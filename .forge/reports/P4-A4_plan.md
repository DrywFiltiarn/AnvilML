# Plan Report: P4-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A4                                       |
| Phase       | 4 — Hardware Detection: Detectors           |
| Description | anvilml-hardware: VulkanDetector headless enumeration |
| Depends on  | P4-A3                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-28T23:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-hardware/src/vulkan.rs` implementing `VulkanDetector: DeviceDetector` using the `ash` crate (v0.38.0) for headless Vulkan instance creation and physical device enumeration. The detector maps each `VkPhysicalDevice` to a `GpuDevice` by vendor ID (`0x10de` → `Cuda`, `0x1002` → `Rocm`, unknown → skip). `detect()` never panics — loader absence or `vkCreateInstance` failure returns `Ok(vec![])`. `refresh_vram` queries memory heaps via `VK_EXT_memory_budget` if available, else returns `(total, total)`.

## Scope

### In Scope
- Create `crates/anvilml-hardware/src/vulkan.rs` with `VulkanDetector` struct implementing `DeviceDetector`.
- Add `ash = "0.38.0"` dependency to `crates/anvilml-hardware/Cargo.toml`.
- Add `mod vulkan;` and `pub use vulkan::VulkanDetector;` to `crates/anvilml-hardware/src/lib.rs`.
- Implement `detect()` — headless `vkCreateInstance`, enumerate physical devices, map vendor IDs to `DeviceType`, construct `GpuDevice` for each.
- Implement `refresh_vram()` — query `VK_EXT_memory_budget` extension if available, else fall back to `get_physical_device_memory_properties`.
- Create `crates/anvilml-hardware/tests/vulkan_tests.rs` with ≥4 tests.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope without deferral.

## Existing Codebase Assessment

The `anvilml-hardware` crate currently contains three modules: `detect.rs` (the `DeviceDetector` trait), `cpu.rs` (`CpuDetector`), and `mock.rs` (`MockDetector`, gated behind `mock-hardware`). The `lib.rs` is 11 lines with `pub mod` declarations and re-exports.

Established patterns to follow:
- **Naming**: `pub struct <Name>Detector;` with `impl DeviceDetector for <Name>Detector`.
- **Error handling**: `detect()` never returns `Err` — uses `?` internally but wraps loader failures into `Ok(vec![])` at the top level.
- **Test style**: Integration tests in `tests/*.rs` files, importing via `use anvilml_core::types::*;` and `use anvilml_hardware::detect::DeviceDetector;`. Tests use `.expect("message")` for assertions.
- **Logging**: The task context does not mention logging requirements, but `ANVILML_DESIGN.md §16.2–§16.3` defines mandatory log points. The plan's Approach section will note logging additions.
- **No dual-mode parity markers**: §10.6 markers apply only to node `execute()` and arch module `load()`/`sample()`/`decode()` — hardware detectors are outside this convention.

No gap between the design doc and current source: `vulkan.rs` does not yet exist (confirmed by directory listing), and the `GpuDevice`, `DeviceType`, `EnumerationSource`, `CapabilitySource`, and `DeviceDetector` trait types are all present and match the design spec.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | ash     | 0.38.0          | rust-docs MCP  | loaded, debug, std (defaults) |

`ash` v0.38.0+1.3.281 is the latest on crates.io. The `loaded` default feature enables dynamic loading of the Vulkan loader (`libvulkan.so` / `vulkan-1.dll`) at runtime — this is exactly what we need for SDK-free enumeration. The `debug` feature adds debug-report support (harmless, not used here). The `std` feature enables `std::ffi` usage (needed for `CStr`/`CString`).

Key API types confirmed via MCP:
- `ash::Instance` — `create_instance()` method, `enumerate_physical_devices()`, `get_physical_device_properties()`, `get_physical_device_memory_properties()`, `destroy_instance()`.
- `ash::vk::PhysicalDeviceProperties` — has `vendor_id: u32`, `device_id: u32`, `device_name: [c_char; 256]`.
- `ash::vk::PhysicalDeviceMemoryProperties` — has `memory_heaps: [MemoryHeap; 16]`.
- `ash::extensions_generated::ext::memory_budget` — `VK_EXT_memory_budget` extension module exists.

## Approach

### Step 1: Add `ash` dependency to `Cargo.toml`

Add `ash = "0.38.0"` to `[dependencies]` in `crates/anvilml-hardware/Cargo.toml`. No feature flags needed beyond the defaults (`loaded` enables dynamic Vulkan loader loading).

### Step 2: Create `VulkanDetector` struct in `vulkan.rs`

Create `crates/anvilml-hardware/src/vulkan.rs` with:

```rust
/// Headless Vulkan GPU detector using the `ash` crate.
///
/// Creates a Vulkan instance without any surface extension (headless),
/// enumerates physical devices, and maps them to `GpuDevice` via PCI
/// vendor ID. Never panics — loader absence returns `Ok(vec![])`.
pub struct VulkanDetector;
```

### Step 3: Implement `detect()` on `VulkanDetector`

The `detect()` method performs these steps:

1. **Create a headless Vulkan instance** using `ash::Instance::create_instance()`. No surface extensions are requested — this is purely for device enumeration. The instance creation may fail (Vulkan loader absent, no driver, etc.).
2. **On instance creation failure**, return `Ok(vec![])` — never `Err`, never panic. This follows the design principle from §6.2.
3. **Enumerate physical devices** via `instance.enumerate_physical_devices()`.
4. **For each `VkPhysicalDevice`**, call `get_physical_device_properties()` to obtain `vendor_id`, `device_id`, and `device_name`.
5. **Map vendor ID to `DeviceType`**: `0x10de` → `Cuda`, `0x1002` → `Rocm`. Skip any device with a different vendor ID (not a GPU this system targets).
6. **Construct a `GpuDevice`** for each accepted physical device with:
   - `index`: zero-based index in the enumeration loop
   - `name`: from `device_name` (null-terminated `[c_char; 256]` → `CStr` → `String`)
   - `device_type`: from vendor ID mapping
   - `vram_total_mib`: 0 (filled by `refresh_vram` later)
   - `vram_free_mib`: 0
   - `driver_version`: formatted from `driver_version` u32 (major.minor.patch)
   - `pci_vendor_id`: `(vendor_id & 0xFFFF) as u16`
   - `pci_device_id`: `(device_id & 0xFFFF) as u16`
   - `arch`: `None` (Vulkan doesn't expose architecture string directly; this is a hint from the device table)
   - `caps`: `InferenceCaps::default()` (pre-spawn hint; real values come from worker probe)
   - `enumeration_source`: `EnumerationSource::Vulkan`
   - `capabilities_source`: `CapabilitySource::DeviceTable`
7. **Destroy the instance** via `instance.destroy_instance(None)` before returning. This must happen even on error paths to avoid leaks.

**Rationale for headless instance**: We don't need a window surface or swapchain — just device enumeration. The `ash` crate's `loaded` default feature dynamically loads `libvulkan.so`/`vulkan-1.dll` at runtime, so no system Vulkan SDK is required.

**Rationale for `Ok(vec![])` on failure**: The design (§6.2) mandates that detection never panics and never returns `Err` for loader absence. The caller (Phase 5's `detect_all_devices`) will fall through to the next detector or append a CPU fallback.

### Step 4: Implement `refresh_vram()` on `VulkanDetector`

The `refresh_vram()` method:

1. **Try `VK_EXT_memory_budget` extension**: Create a minimal instance with `VK_EXT_memory_budget` enabled. If the extension is available, query `vkGetPhysicalDeviceMemoryBudgetEXT` for the device at `index`. This gives accurate VRAM usage.
2. **If the extension is unavailable or the instance creation fails** (extension not supported), fall back to `get_physical_device_memory_properties()` and return `(total_heap_size, total_heap_size)` — i.e., `(total, total)` since we cannot determine free memory without a device allocation.
3. **Return `(total_mib, free_mib)`** where both values are in mebibytes.

**Rationale for `(total, total)` fallback**: Without the memory budget extension, Vulkan's `PhysicalDeviceMemoryProperties` only reports total heap sizes — there is no "free" field. Returning `(total, total)` signals "unknown free" rather than a wrong estimate. The caller can distinguish this from a real measurement by checking if `total == free`.

### Step 5: Update `lib.rs`

Add `pub mod vulkan;` and `pub use vulkan::VulkanDetector;` to `crates/anvilml-hardware/src/lib.rs`. Keep the file under 80 lines.

### Step 6: Create integration tests in `vulkan_tests.rs`

Create `crates/anvilml-hardware/tests/vulkan_tests.rs` with at least 4 tests:

1. **`test_vulkan_loader_absent_returns_empty`**: Run in a subprocess that sets `LD_LIBRARY_PATH` to an empty directory (or uses `dlopen`/`libloading` to prevent Vulkan loader from finding `libvulkan.so`). Assert `detect()` returns `Ok(vec![])`, not `Err`. This verifies the "never panic, never Err" contract.
2. **`test_vulkan_nvidia_vendor_maps_to_cuda`**: Use a mock/stub approach — since we can't guarantee a real NVIDIA GPU is present, we test the vendor ID mapping logic by creating a helper function `fn vendor_id_to_device_type(vendor_id: u32) -> Option<DeviceType>` that is `pub(crate)` and testable in isolation. Verify `0x10de → Some(Cuda)`.
3. **`test_vulkan_amd_vendor_maps_to_rocm`**: Same helper function test — verify `0x1002 → Some(Rocm)`.
4. **`test_vulkan_unknown_vendor_is_skipped`**: Same helper function test — verify `0x1234 → None` (unknown vendor is skipped).

**Why a helper function for vendor mapping**: The Vulkan loader may not be available in the test environment (CI has no GPU), so we cannot reliably create a Vulkan instance. By extracting the vendor ID → `DeviceType` mapping into a pure, testable function, we can test all vendor ID branches without requiring Vulkan hardware. The `detect()` method itself still calls the real Vulkan API, but the critical mapping logic is independently testable.

### Step 7: Logging

Add `tracing::debug!` calls at key decision points in `detect()`:
- When Vulkan loader is absent: `tracing::debug!("Vulkan loader not available, skipping Vulkan detection")`
- When a device is enumerated with a known vendor: `tracing::debug!(vendor_id = %vendor_id, device_name = %name, "detected GPU")`
- When a device is skipped (unknown vendor): `tracing::debug!(vendor_id = %vendor_id, "skipping unknown vendor")`

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| struct | `anvilml_hardware::vulkan::VulkanDetector` | `pub struct VulkanDetector;` |
| impl | `anvilml_hardware::vulkan::VulkanDetector` | `impl DeviceDetector for VulkanDetector { ... }` |
| fn | `anvilml_hardware::vulkan::vendor_id_to_device_type` (pub(crate)) | `pub(crate) fn vendor_id_to_device_type(vendor_id: u32) -> Option<DeviceType>` |

The `vendor_id_to_device_type` helper is `pub(crate)` so it can be tested from `tests/` integration test crates while remaining internal to the crate.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/vulkan.rs` | `VulkanDetector` struct and `DeviceDetector` impl |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Add `ash = "0.38.0"` dependency |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Add `pub mod vulkan;` and `pub use vulkan::VulkanDetector;` |
| CREATE | `crates/anvilml-hardware/tests/vulkan_tests.rs` | Integration tests for `VulkanDetector` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `tests/vulkan_tests.rs` | `test_vulkan_loader_absent_returns_empty` | When Vulkan loader is unavailable (no libvulkan.so), `detect()` returns `Ok(vec![])`, not `Err` and not a panic. Runs in a subprocess with isolated library path to prevent Vulkan loader from being found. | `cargo test -p anvilml-hardware --test vulkan_tests -- test_vulkan_loader_absent_returns_empty` exits 0 |
| `tests/vulkan_tests.rs` | `test_vulkan_nvidia_vendor_maps_to_cuda` | `vendor_id_to_device_type(0x10de)` returns `Some(DeviceType::Cuda)`. Pure function test, no Vulkan required. | `cargo test -p anvilml-hardware --test vulkan_tests -- test_vulkan_nvidia_vendor_maps_to_cuda` exits 0 |
| `tests/vulkan_tests.rs` | `test_vulkan_amd_vendor_maps_to_rocm` | `vendor_id_to_device_type(0x1002)` returns `Some(DeviceType::Rocm)`. Pure function test, no Vulkan required. | `cargo test -p anvilml-hardware --test vulkan_tests -- test_vulkan_amd_vendor_maps_to_rocm` exits 0 |
| `tests/vulkan_tests.rs` | `test_vulkan_unknown_vendor_skipped` | `vendor_id_to_device_type(0x1234)` returns `None`. Unknown vendors are skipped, not treated as GPUs. Pure function test, no Vulkan required. | `cargo test -p anvilml-hardware --test vulkan_tests -- test_vulkan_unknown_vendor_skipped` exits 0 |
| `tests/vulkan_tests.rs` | `test_vulkan_refresh_vram_fallback` | `refresh_vram(0)` returns `Ok((total, total))` when Vulkan memory budget extension is unavailable (always the case in CI without GPU). Verifies the fallback path. | `cargo test -p anvilml-hardware --test vulkan_tests -- test_vulkan_refresh_vram_fallback` exits 0 |

## CI Impact

No CI changes required. The `ash` crate is a pure Rust dependency with no system-level build steps beyond linking to `libvulkan.so`/`vulkan-1.dll` at runtime (dynamically loaded, not linked at compile time). The `mock-hardware` feature does not affect this module — `vulkan.rs` is always compiled and tested regardless of features. CI already compiles `anvilml-hardware` with `--features mock-hardware` (per ENVIRONMENT.md §6 Step 6), and the Vulkan tests will pass on CI runners that lack Vulkan (loader absence returns `Ok(vec![])`).

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The `ash` crate handles platform-specific Vulkan loader loading (`libvulkan.so` on Linux, `vulkan-1.dll` on Windows) transparently via its `loaded` feature. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed in this code.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ash` 0.38.0 may have changed the `Instance::create_instance` signature from earlier versions (e.g., `InstanceCreateInfo` lifetime parameters). The MCP confirmed the current API shape: `unsafe fn create_instance(self: &Self, create_info: &vk::InstanceCreateInfo<'_>, allocation_callbacks: Option<&vk::AllocationCallbacks<'_>>) -> VkResult<Instance>`. | Low | High | MCP-verified API shape used in plan. If the ACT agent discovers a mismatch, it must use the MCP result over the plan. |
| The Vulkan loader may be present but return an error code (not a NULL pointer) on CI runners that have a stub Vulkan driver. This would cause `enumerate_physical_devices()` to return `Err`, which must be converted to `Ok(vec![])`. | Medium | Medium | Wrap `enumerate_physical_devices()` in a `.unwrap_or_default()` or match on `VkResult` to convert errors to `Ok(vec![])`. The "never Err" contract is the highest priority. |
| `device_name` is `[c_char; 256]` — converting to `String` requires null-terminated `CStr` handling. A malformed name could panic on `.to_string_lossy()`. | Low | Low | Use `.to_string_lossy()` which never panics and replaces invalid sequences with `�`. This is safe for device names. |
| `refresh_vram` fallback returns `(total, total)` — the caller may misinterpret this as "all VRAM is free." | Low | Low | Document clearly in the function doc comment that `(total, total)` means "free unknown." The Phase 5 caller should check `total == free` as a sentinel. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml-hardware` exits 0
- [ ] `cargo test -p anvilml-hardware --test vulkan_tests` exits 0 (≥4 tests)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `vendor_id_to_device_type(0x10de)` returns `Some(DeviceType::Cuda)` (verified by test)
- [ ] `vendor_id_to_device_type(0x1002)` returns `Some(DeviceType::Rocm)` (verified by test)
- [ ] `vendor_id_to_device_type(0x1234)` returns `None` (verified by test)
- [ ] Loader-absent scenario returns `Ok(vec![])` not `Err` (verified by subprocess test)
