# Implementation Report: P4-A2

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P4-A2                              |
| Phase         | 004 — Hardware Detection           |
| Description   | anvilml-hardware: VulkanDetector SDK-free GPU enumeration |
| Implemented   | 2026-06-15T08:45:00Z              |
| Status        | COMPLETE                           |

## Summary

Implemented `VulkanDetector` in `crates/anvilml-hardware/src/vulkan.rs` — a zero-sized struct that implements `DeviceDetector` to enumerate physical Vulkan GPUs using the `ash` crate with the `loaded` feature (runtime `dlopen`). The detector loads the Vulkan entry point, creates an instance with `VK_KHR_driver_properties` and `VK_EXT_memory_budget` extensions, enumerates physical devices, and extracts device metadata (name, driver version, VRAM). All Vulkan errors are handled gracefully by returning `Ok(vec![])`. Four integration tests verify construction, detection, VRAM refresh, and Send+Sync traits. All 73 workspace tests pass, all 4 platform cross-checks pass, clippy reports zero warnings, and format check passes.

## Resolved Dependencies

| Type   | Name   | Version resolved | Source          |
|--------|--------|------------------|-----------------|
| crate  | ash    | 0.38.0+1.3.281   | crates.io API   |

The plan specified `ash = { version = "0.38" }` which matches the latest `0.38.0+1.3.281`. The `loaded` feature enables `Entry::load()` for SDK-free Vulkan loader discovery.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/vulkan.rs` | `VulkanDetector` struct + `DeviceDetector` impl (~260 lines) |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Added `pub mod vulkan;` and `pub use vulkan::VulkanDetector;` |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Added `ash` dependency; bumped version 0.1.1 → 0.1.2 |
| CREATE | `crates/anvilml-hardware/tests/vulkan_tests.rs` | 4 integration tests for `VulkanDetector` |
| MODIFY | `docs/TESTS.md` | Added 4 test entries for new Vulkan tests |
| MODIFY | `Cargo.lock` | Updated by cargo (added `ash` and `libloading` transitive dep) |

## Commit Log

```
 .forge/reports/P4-A2_plan.md                  | 156 ++++++++++++++++
 .forge/state/CURRENT_TASK.md                  |   6 +-
 .forge/state/state.json                       |  13 +-
 Cargo.lock                                    |  22 ++-
 crates/anvilml-hardware/Cargo.toml            |   3 +-
 crates/anvilml-hardware/src/lib.rs            |   2 +
 crates/anvilml-hardware/src/vulkan.rs         | 260 ++++++++++++++++++++++++++
 crates/anvilml-hardware/tests/vulkan_tests.rs |  84 +++++++++
 docs/TESTS.md                                 |  32 ++++
 9 files changed, 567 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/vulkan_tests.rs (target/debug/deps/vulkan_tests-2e103d91e42f0cb6)

running 4 tests
test test_vulkan_detector_detect_returns_empty_or_devices ... ok
test test_vulkan_detector_is_send_sync ... ok
test test_vulkan_detector_new ... ok
test test_vulkan_detector_refresh_vram_returns_zero ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s

Full workspace: 73 tests passed, 0 failed.
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.77s
--- CHECK 1 PASSED ---

# 2. Mock-hardware Windows:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.64s
--- CHECK 2 PASSED ---

# 3. Real-hardware Linux:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.91s
--- CHECK 3 PASSED ---

# 4. Real-hardware Windows:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.01s
--- CHECK 4 PASSED ---
```

## Project Gates

```
# Gate 1 — Config Surface Sync:
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

# Clippy (mock-hardware):
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.08s

# Clippy (real-hardware):
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.13s

# Python tests:
1 passed in 0.01s
```

## Public API Delta

```
+pub mod vulkan;
+pub use vulkan::VulkanDetector;
```

New pub items introduced:
- `pub mod vulkan` — module path `anvilml_hardware::vulkan`
- `pub use vulkan::VulkanDetector` — re-exported from `anvilml_hardware`

Internal pub items in `vulkan.rs` (not re-exported at crate root):
- `pub struct VulkanDetector` — `anvilml_hardware::vulkan::VulkanDetector`
- `pub const fn new() -> Self` — `anvilml_hardware::vulkan::VulkanDetector::new`
- `fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>` — `DeviceDetector` trait impl
- `fn refresh_vram(&self, index: u32) -> Result<(u32, u32), AnvilError>` — `DeviceDetector` trait impl

## Deviations from Plan

- **ash API differences (ash 0.38.0+1.3.281):** The plan referenced `ash::khr::driver_properties::VK_KHR_DRIVER_PROPERTIES_EXTENSION_NAME` and `ash::ext::memory_budget::VK_EXT_MEMORY_BUDGET_EXTENSION_NAME`, but ash 0.38 relocated these to `ash::vk::KHR_DRIVER_PROPERTIES_NAME` and `ash::vk::EXT_MEMORY_BUDGET_NAME`. Also, `ash::ext::memory_budget` module does not exist in ash 0.38 — the extension is defined in `ash::vk::extensions.rs` directly.
- **Builder pattern removed:** ash 0.38 removed the builder pattern for `ApplicationInfo` and `InstanceCreateInfo`. Used struct literal syntax with explicit field initialization instead.
- **Type changes:** `PhysicalDeviceProperties::device_name` is `[c_char; 256]` (i8) not `[u8; 256]`. `vendor_id` and `device_id` are `u32` not `u16` (cast to `u16` for `GpuDevice` fields). `MemoryHeap::size` is `DeviceSize` (u64), not u32.
- **Version functions deprecated:** `vk::version_major/minor/patch` are deprecated in ash 0.38; used `vk::api_version_major/minor/patch` instead.
- **Unsafe calls:** `get_physical_device_properties` and `get_physical_device_memory_properties` are `unsafe` methods in ash 0.38 — wrapped in `unsafe` blocks with inline comments explaining the invariant.
- **Module-level doc comment:** Used `//!` (inner doc comment) for the module-level doc comment instead of `///` (outer) to comply with clippy's `empty_line_after_doc_comments` lint.

## Blockers

None.
