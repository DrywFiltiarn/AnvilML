# Plan Report: P4-A3

| Field | Value |
|-------|-------|
| Task ID | P4-A3 |
| Phase | 004 — Hardware Detection |
| Description | Vulkan GPU enumerator (primary, SDK-free, fixture-tested) |
| Depends on | P4-A2 |
| Project | AnvilML |
| Planned at | 2026-06-03T17:58:00Z |
| Attempt | 1 |

## Objective

Create `crates/anvilml-hardware/src/vulkan.rs` implementing a headless Vulkan GPU enumerator using the `ash` crate. The detector creates a no-surface VkInstance, enumerates physical devices via vkEnumeratePhysicalDevices, and for each device reads properties2 with KHR_driver_properties (name/driver) + EXT_memory_budget (VRAM budget/usage). It populates every field in GpuDevice including PCI vendor/device IDs derived from the Vulkan driver's own data — no nvidia-smi / rocm-smi required. When the loader is absent, returns `Ok(vec![])` instead of panicking or aborting startup.

## Scope

### In Scope
- Create `src/vulkan.rs`: full VulkanDetector struct implementing DeviceDetector trait (P4-A1).
- Add `ash = "0.38"` dependency to Cargo.toml with feature flags for KHR_driver_properties and EXT_memory_budget extensions, plus platform-specific loader loading (`vulkan-loader` or system pkg-config on Linux; embedded VK_LOADER_API_VERSION guard + optional linking fallbacks on Windows cross-compilation).
- Headless VkInstance creation (no surface/window — `ash-window`, no external windowing dependency needed since we skip vkCreateDisplayPlaneSurface and use only physical device queries, not instance surfaces).
- Device enumeration: iterate all VK_KHR_driver_properties names/driver strings; parse Vulkan build-number driver version into human-readable form.
- VRAM calculation per §5.2 of ANVILML_DESIGN.md total_vram = largest DEVICE_LOCAL heap's heapSize (ignoring small host-visible Resizable-BAR heaps); available = budget ext values if present, else conservative fallback to heapSize minus usage estimate.
- PCI ID population from physicalDeviceProperties.vendorID / deviceID; vendor → DeviceType mapping table (0x10DE→Cuda, 0x1002/other non-intel→Rocm, Intel-only→Cpu).
- Driver version string extraction: KHR_driver_properties.driverName + driverVersion from properties. Source = Vulkan on every device record.
- `src/lib.rs` update to declare `pub mod vulkan;`.

### Out of Scope
- DXGI fallback (P4-A4, Windows-specific) — not part of this task's deliverable module.
- PCI sysfs / NVML Linux-fallback enumerators (also P4-A4).
- device_db.rs capability table lookup and resolution logic (P4-A5 orchestrator work).
- detect_all_devices orchestration function that chains Vulkan → fallbacks → CPU — handled in later tasks.

## Approach

1. **Add ash dependency to Cargo.toml.** Add `ash = { version = "0.38", features = ["linked"] }` (the `"linked"` feature delegates loader loading at runtime so absence of vulkan.dll / libvulkan.so returns a clean error rather than linker failure). This is critical for the Windows cross-compilation target (`x86_64-pc-windows-gnu`) where no Vulkan DLL exists in the sysroot.

2. **Create `src/vulkan.rs`.** The file follows this structure:
   - Module-level documentation referencing ANVILML_DESIGN §5 and P4-A3 task spec.
   - Imports from ash (Instance, PhysicalDevice, DeviceProperties2KHR extensions for driver name/version), vk::Extent2D helper if needed.
   - `pub struct VulkanDetector` — zero-sized or trivially-constructible type with Default derive.
   - `impl DeviceDetector for VulkanDetector`: the core enumeration logic as described below in numbered sub-steps (3a–f).

3. **Enumeration algorithm** inside `detect()`. All fallthrough errors return early:
   
   a) Create VkInstance via ash's Instance::new with extension names collected at compile-time from ash constants (`VK_KHR_DRIVER_PROPERTIES_EXTENSION_NAME` + `VK_EXT_MEMORY_BUDGET_EXTENSION_NAME`). No surface extensions needed since we never create windows or swapchains. If instance creation fails (e.g., no loader found), log and return Ok(vec![]).
   
   b) Enumerate physical devices with Instance::enumerate_physical_devices(). For each PhysicalDevice:
      - Query VkPhysicalDeviceProperties2 chain including driver properties via ash's vkGetPhysicalDeviceProperties2KHR. Extract name from pNext-chained structure (driverName string), and parse the 30-bit Vulkan build-number into "major.minor.patch" format for device.driver_version. If KHR_driver_properties is unavailable, fall back to property.pDriverVersion formatted as a version tuple + set driver_name = unknown placeholder.
      - Query VkPhysicalDeviceMemoryProperties2 with EXT_memory_budget pNext chain via ash's vkGetPhysicalDeviceMemoryPropertyTypes (or the 38.x equivalent). Extract heapCount and memoryTypeCount from properties, then budget/usage for each heap type from extension data.
      
   c) VRAM calculation: iterate all heaps; find those matching VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT; pick largest by heapSize into total_vram_mib (convert bytes to MiB with saturating division). For available vram: if EXT_memory_budget is present, take the budget − usage for that same index as free. Otherwise fall back conservatively using a heuristic or just report 0u32 and let worker refresh later at Ready state per §5 of ANVILML_DESIGN.md (the task says "available = heapBudget - heapUsage if budget ext else heapSize" so we use total_vram_mib as the available figure when no budget extension).

   d) PCI ID extraction: read physicalDeviceProperties.vendorID and deviceID from VkPhysicalDeviceProperties. Map vendor → DeviceType via match table (0x10DE→Cuda, 0x1002/other non-Intel→Rocm, Intel-only IDs like 8086 or others that aren't NVIDIA/AMD→Cpu). Note: this is the *candidate* backend; worker's torch build confirms at Ready (§5.4 of DESIGN.md — not part of vulkan.rs scope but documented as a comment in code for clarity on intent).
   
   e) Populate GpuDevice fields with all data gathered, setting enumeration_source = Vulkan (once P4-B2 adds the type), capabilities_source from device_db lookup result or Fallback if no table yet. For now — since anvilml-core's current 6-field GpuDevice has only index/name/device_type/vram_total_mib/vram_free_mib/driver_version — construct devices with those fields populated; add a comment noting the P4-B2 extension points for pci_vendor_id, arch (from driver properties), enumeration_source.
   
   f) Return Ok(devices).

4. **Update `src/lib.rs`.** Add `pub mod vulkan;` alongside existing cpu and mock modules so VulkanDetector is publicly accessible to downstream consumers once detect_all_devices wiring happens in P4-A5. Keep the compile-check test for CpuDetector intact — add a matching one that casts &VulkanDetector::default() as dyn DeviceDetector (compile-time trait impl verification).

5. **Tests.** Add `#[cfg(test)] mod tests` inside vulkan.rs:
   - Test 1 (`vulkan_detect_compiles`) — compile-only test verifying VulkanDetector implements the trait and can be constructed without panicking at module load time. This passes on all platforms even when no GPU exists, because we return Ok(vec![]).
   
     Actually rethinking this since it's not really a "fixture" per se... Let me define tests more concretely:

   - Test A (`vulkan_detect_no_vulkan_loader`): simulate loader absence by testing that the detection path handles `vkEnumeratePhysicalDevices = Err(NoInstance)` or similar gracefully. Since we can't mock ash's raw Vulkan bindings in unit tests, this test verifies compilation and structure correctness — it calls detect() which will either enumerate real GPUs (on a machine with one) or return Ok(vec![]) on CI/CI-like environments without drivers. Assert that the result is always `Ok` regardless of environment state (the key invariant: no panics, never Err).
   
   - Test B (`vulkan_detect_properties_parsing`) — fixture test using known-good PCI IDs from ANVILML_DESIGN §5.3 vendor table to verify our mapping logic produces correct DeviceType for 0x10DE→Cuda and 0x1002→Rocm regardless of whether actual Vulkan is present (this tests the pure parsing/mapping function in isolation, not ash calls).
   
   - Test C (`vulkan_detect_vram_calculation`): fixture test with synthetic heap data — construct mock VkMemoryHeap values representing a 8GB device-local + small host-visible Resizable-Bar configuration and verify that our `total_vram_mib = largest_device_local_heap_size_in_MiB` logic produces exactly the expected value (e.g., 8192 MiB ignoring a hypothetical second heap of only 64MB). Tests byte→MiB conversion, truncation safety for values > u32::MAX.

   All three tests use `serial_test` as already available in dev-dependencies to avoid env-var interference with other modules' serial-locked fixtures.
   
   Test invocation: `cargo test -p anvilml-hardware -- vulkan`. Tests must exit 0 regardless of whether a physical GPU is present — the detection path never panics; absence yields Ok(vec![]) which passes all assertions about "no errors returned".

6. **Cross-compilation check.** The task requires passing: `cargo check --target x86_64-pc-windows-gnu --features mock-hardware`. This exercises that ash's `"linked"` feature (which defers Vulkan DLL loading to runtime) compiles cleanly against the GNU Windows target in CI without requiring a native vulkan.dll in the sysroot. The test agent verifies this command exits 0 with no warnings — it does not run code, only checks compilation.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-hardware/src/vulkan.rs` | VulkanDetector struct + DeviceDetector impl; enumeration algorithm; VRAM calculation logic; PCI ID vendor mapping tests |
| Modify | `Cargo.toml` (anvilml-hardware) | Add ash dependency with extension feature flags and linked loader mode |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| vulkan.rs (`#[cfg(test)]`) | `vulkan_detect_no_vulkan_loader` (or equivalent name: e.g. `detect_returns_ok_when_empty_or_present`) | detect() always returns Ok — never panics or errors, even without Vulkan loader installed; on systems with GPUs it yields non-empty Vec<GpuDevice> populated from real device data |
| vulkan.rs (`#[cfg(test)]`) | Vendor mapping unit tests (e.g. `vendor_id_maps_cuda`, `vendor_id_maps_rocm`) | Pure parsing/mapping functions produce correct DeviceType for 0x10DE→Cuda, 0x1002/other non-Intel→Rocm; Intel-only IDs → Cpu fallback |
| vulkan.rs (`#[cfg(test)]`) | VRAM calculation fixture test (e.g. `largest_device_local_heap_wins_over_host_visible_resizable_bar`) | Given synthetic memory heap data: a large device-local + small host-visible Resizable-BAR, total_vram_mib = size of largest DEVICE_LOCAL only; correct byte→MiB conversion and u32 truncation safety for >4GB heaps |
| N/A (check-only) | `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` verifies ash compiles against GNU Windows target without requiring vulkan.dll in sysroot (`linked` feature works correctly; no linker errors on missing DLL because loading is deferred to runtime) |

## CI Impact

This task adds `ash = "0.38"` as a new dependency of anvilml-hardware with the `"linked"` loader mode and extension features for KHR_driver_properties / EXT_memory_budget. The build matrix (per ARCHITECTURE.md §9): Linux jobs run `cargo fmt --all --check`, then clippy + test under mock-hardware, so ash must compile on ubuntu-latest — typically available via system vulkan-headers/vulkan-loader-dev packages or the `"linked"` feature's pkg-config fallback. The Windows job runs `cargo check/clippy/test` for x86_64-pc-windows-gnu (already in rust-toolchain.toml); ash with linked mode defers DLL loading so no native Vulkan SDK is needed at compile time on this target, only a successful compilation pass. No CI YAML changes are required since the existing matrix already covers these commands — but if ubuntu-latest lacks vulkan-headers-dev for pkg-config resolution during clippy's dependency fetch phase, adding `libvulkan-dev` to any self-hosted runner setup (if applicable) or switching ash features from `"linked"` → explicit optional linking with a fallback would be needed. The implementation agent should verify the exact CI environment and adjust if compilation fails due to missing Vulkan headers on ubuntu-latest runners; this is unlikely in 2026+ but worth checking during verification of `cargo check` output for clippy warnings/errors about unused imports (ash extension types that are only used conditionally).

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| ash + `"linked"` feature may fail to compile on ubuntu-latest CI runners if libvulkan headers/development packages aren't installed. The linked mode uses pkg-config which needs vulkan.pc shipped by `libvulkan-dev`. | During verification, check clippy output for compilation failures; add a comment noting that installing `sudo apt install -y libvulkan-dev` resolves this on CI (though in practice ash's `"ash-window"` and related crates sometimes bundle their own loader). If pkg-config fails as hard error rather than graceful fallback: switch to optional linking feature or use the embedded VK_LOADER_API_VERSION approach. |
| Cross-compilation (`x86_64-pc-windows-gnu`) may fail if Vulkan DLL symbols are expected at link time despite `"linked"` deferring loading, because ash's `vk_loader` crate still pulls in system headers for build scripts that run on the host (Linux). | Verify with a dedicated check command. If it fails: add `[target.x86_64-pc-windows-gnu] dependencies.ash.optional = true` or use conditional compilation to exclude Vulkan-dependent code from cross-compilation while keeping mock-hardware functional — though this is unlikely given ash's design for runtime loading only. |
| VRAM values exceed u32::MAX on modern GPUs (> 16GB), causing truncation in heapSize→MiB conversion if not handled carefully (bytes → MiB: divide by `1048576`, then saturate to u32). The task says vram_total_mib is u32. | Use `.saturating_div(1 << 20)` for bytes-to-MiB and clamp with min(u32::MAX) as defensive measure; document the truncation point in code comments referencing §5 of DESIGN.md which specifies MiB storage type. This is a known design constraint, not a bug — any GPU > ~4 billion MiB (impossible at current hardware scales). |
| KHR_driver_properties may be unavailable on older drivers or macOS Vulkan implementations; EXT_memory_budget similarly optional even with modern Linux NVIDIA/AMD drivers. | Both extensions are queried via vkGetPhysicalDeviceProperties2KHR pNext chain — if the extension is not available, ash returns Option::None for driver properties and uses default memory property values without budget data (available VRAM falls back to heapSize). No panics or hard failures; just less precise metrics which P4-A5's fallback path will refine via sysfs/NVML anyway. |
| Current anvilml-core GpuDevice struct lacks pci_vendor_id, arch, enumeration_source fields that ANVILML_DESIGN §4.3 and §5 specify vulkan.rs should populate (P4-B2 adds these). | Plan against the target schema from DESIGN.md — code will reference types like EnumerationSource { Vulkan } which P4-B2 introduces in anvilml-core/types/hardware.rs before this module is wired into detect_all_devices by P4-A5. The implementation agent for vulkan.rs should coordinate with whoever implements P4-B2 (or assume it's done first since both are Phase-004 tasks and sequential within the phase). If types don't exist yet: use placeholder values / Option fields until extension lands, then update in a single pass across all detectors. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware -- vulkan` exits 0 (all tests green regardless of physical GPU presence; detection never panics or returns Err)
- [ ] VulkandDetector implements the DeviceDetector trait (`&VulkanDetector as &dyn DeviceDetector`) — verified by compile-time check in lib.rs test module and runtime call to `detect()` + `refresh_vram()`.
- [ ] Each detected GpuDevice has: name from KHR_driver_properties or VkPhysicalDeviceProperties, device_type mapped correctly (0x10DE→Cuda; 0x1002/other non-intel → Rocm), driver_version parsed from Vulkan build number + optional vendor string, source field = "Vulkan"
- [ ] VRAM calculation: total_vram_mib equals the largest DEVICE_LOCAL heap's size in MiB (ignoring small host-visible Resizable-BAR heaps); available vram uses EXT_memory_budget when present else falls back to heapSize. Verified by fixture test with synthetic data representing known configuration patterns.
- [ ] Loader-absent case: VkInstance creation failure returns `Ok(vec![])` — no panic, no Err wrapping the platform error; detection is silent and graceful per §5 of DESIGN.md ("Loader absent → Ok(vec!)").
- [ ] PCI vendor/device IDs populated from physicalDeviceProperties.vendorID / deviceID. Verified by fixture test that mapping logic produces correct DeviceType for known vendors (0x10DE→Cuda, 0x1002→Rocm).
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits with no errors or warnings — ash compiles cleanly against the GNU Windows cross-compilation target.
