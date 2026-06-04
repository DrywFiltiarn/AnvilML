# Plan Report: P7-D2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-D2                                       |
| Phase       | 007 — WebSocket Event Stream                |
| Description | anvilml-hardware: explicit detector warnings + Vulkan extension fix |
| Depends on  | P7-D1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-04T22:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Fix the Vulkan instance creation bug caused by passing device-level extensions (`VK_KHR_driver_properties` and `VK_EXT_MEMORY_BUDGET`) as instance-level enabled extensions, and add explicit `tracing::warn!` logging at every silent discard site across all hardware detector modules so that runtime failures are visible instead of silently returning `Ok(vec![])`.

## Scope

### In Scope
- **vulkan.rs**: Remove `KHR_driver_properties` and `EXT_memory_budget` from the instance creation extension list; pass empty `enabled_extension_names`; add per-device extension query via `enumerate_device_extension_properties`; gate `PhysicalDeviceDriverProperties` and `PhysicalDeviceMemoryBudgetPropertiesEXT` pNext chains on actual device extension membership; add `tracing::warn!` at every early-return site.
- **vulkan.rs**: Replace all `log::warn!` calls with `tracing::warn!` (no existing log calls in vulkan.rs, but ensure consistency).
- **dxgi.rs**: Replace existing `log::warn!` calls with `tracing::warn!(detector = "Dxgi", error = ...)`.
- **sysfs.rs**: Replace existing `log::warn!` calls with `tracing::warn!(detector = "Sysfs", error = ...)`.
- **nvml.rs**: Replace existing `log::warn!` calls with `tracing::warn!(detector = "Nvml", error = ...)`.
- **lib.rs**: Replace `unwrap_or_default()` calls in `enumerate_gpus()` with explicit `match` + `tracing::warn!(detector, error)`.
- **Cargo.toml (anvilml-hardware)**: Add `tracing` dependency; remove `log` dependency if no longer used by any module.

### Out of Scope
- Changes to `device_db.rs`, `cpu.rs`, or `mock.rs` (no silent discards present).
- Any changes outside `crates/anvilml-hardware/`.
- New tests (task scope is logging + bug fix; existing tests must continue to pass).
- Migration of `log` usage in other crates (`anvilml-registry`, etc.) — handled by P7-D3.

## Approach

### Step 1: Add tracing dependency and remove log dependency

In `crates/anvilml-hardware/Cargo.toml`:
- Add `tracing = { workspace = true }` to `[dependencies]`.
- Remove `log = { workspace = true }` after verifying no remaining `log::` calls exist in the crate.

### Step 2: Fix vulkan.rs — remove device-level extensions from instance creation

The root cause: lines 157–163 build an `extensions` vec containing `VK_KHR_driver_properties` and `VK_EXT_memory_budget`, which are then passed to `InstanceCreateInfo::pp_enabled_extension_names`. These are **device** extensions only — they do not appear in `vkEnumerateInstanceExtensionProperties`. Passing them to `vkCreateInstance` violates the Vulkan spec; AMD ICDs reject with `VK_ERROR_EXTENSION_NOT_PRESENT`.

Changes:
1. Remove lines 157–163 (the extension name vec and pointer construction).
2. Set `enabled_extension_count: 0` and `pp_enabled_extension_names: std::ptr::null()` in `InstanceCreateInfo` (lines 184–185).
3. After enumerating physical devices, for each device call `instance.enumerate_device_extension_properties(*pd, None)` to build a `HashSet<String>` of supported extension names.
4. Wrap the `PhysicalDeviceDriverProperties` pNext chain (lines 225–263) in a check: only chain if `"VK_KHR_driver_properties"` is in the device's extension set. If not, skip the chain and use basic properties only (which already provides `device_name` and `driver_version`).
5. Wrap the `PhysicalDeviceMemoryBudgetPropertiesEXT` pNext chain (lines 266–296) in a check: only chain if `"VK_EXT_memory_budget"` is in the device's extension set. If not, skip the chain and fall back to using `heapSize` for free VRAM estimation.
6. Add `use std::collections::HashSet` at the top of the `detect()` method.

### Step 3: Fix vulkan.rs — add tracing::warn! at every discard site

Replace all bare `return Ok(Vec::new())` patterns with explicit error logging:

- **Line 154** (`Entry::load()` failure):
  ```rust
  Err(e) => {
      tracing::warn!(detector = "Vulkan", error = %e, "Vulkan loader not available");
      return Ok(Vec::new());
  }
  ```

- **Line 191** (`create_instance` failure):
  ```rust
  Err(e) => {
      tracing::warn!(detector = "Vulkan", error = %e, "vkCreateInstance failed — device extensions will not be available");
      return Ok(Vec::new());
  }
  ```

- **Lines 198–200** (`enumerate_physical_devices` failure):
  ```rust
  Err(e) => {
      tracing::warn!(detector = "Vulkan", error = %e, "vkEnumeratePhysicalDevices failed");
      unsafe { instance.destroy_instance(None) };
      return Ok(Vec::new());
  }
  ```

- **Lines 203–206** (empty device list):
  ```rust
  if phys_devs.is_empty() {
      tracing::warn!(detector = "Vulkan", "No Vulkan physical devices found");
      unsafe { instance.destroy_instance(None) };
      return Ok(Vec::new());
  }
  ```

### Step 4: Fix dxgi.rs — replace log::warn! with tracing::warn!

Replace all four `log::warn!` calls (lines 120, 134, 153, 166) with:
```rust
tracing::warn!(detector = "Dxgi", error = ..., "DXGI: ...");
```

The `ComGuard::ensure()` call on line 119 already returns a raw `i32` (HRESULT). Format it as hex:
```rust
if let Err(hr) = self.com_guard.ensure() {
    tracing::warn!(detector = "Dxgi", hr = %format_args!("0x{hr:x}"), "CoInitializeEx failed");
    return Ok(Vec::new());
}
```

### Step 5: Fix sysfs.rs — replace log::warn! with tracing::warn!

Replace all five `log::warn!` calls (lines 172, 194, 203, 212, 224) with:
```rust
tracing::warn!(detector = "Sysfs", error = %e, "sysfs: ...");
```

### Step 6: Fix nvml.rs — replace log::warn! with tracing::warn!

Replace all four `log::warn!` calls (lines 190, 198, 206, 217) with:
```rust
tracing::warn!(detector = "Nvml", error = ..., "nvml: ...");
```

For NVML return code paths where the error is an `i32` (not a `std::io::Error`), format it as a decimal code.

### Step 7: Fix lib.rs — add tracing::warn! at unwrap_or_default() sites in enumerate_gpus()

Replace the chain of `unwrap_or_default()` calls with explicit match + warn patterns:

```rust
let devices = match vulkan::VulkanDetector.detect() {
    Ok(devs) if !devs.is_empty() => devs,
    Ok(_) => {
        tracing::warn!(detector = "Vulkan", "Vulkan detector returned empty device list");
        Vec::new()
    }
    Err(e) => {
        tracing::warn!(detector = "Vulkan", error = %e, "Vulkan detection failed");
        Vec::new()
    }
};
```

Apply the same pattern to `dxgi::DxgiDetector`, `sysfs::SysfsDetector`, and `nvml::NvmlDetector` calls within the fallback branches.

### Step 8: Clean up unused imports

- Remove `use log;` if any module imports it (none currently do — all use `log::warn!` directly).
- Ensure no dead-code warnings from removed `log` dependency.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-hardware/Cargo.toml` | Add `tracing` dep, remove `log` dep |
| Modify | `crates/anvilml-hardware/src/vulkan.rs` | Remove device extensions from instance create; add per-device extension query; gate pNext chains; add tracing::warn! at discard sites |
| Modify | `crates/anvilml-hardware/src/dxgi.rs` | Replace log::warn! → tracing::warn! |
| Modify | `crates/anvilml-hardware/src/sysfs.rs` | Replace log::warn! → tracing::warn! |
| Modify | `crates/anvilml-hardware/src/nvml.rs` | Replace log::warn! → tracing::warn! |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Replace unwrap_or_default() with match + tracing::warn! in enumerate_gpus() |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `vulkan.rs` (mod tests) | `vulkan_detect_returns_ok` | detect() still returns Ok (warns before returning empty vec on no-GPU systems) |
| `dxgi.rs` (mod tests) | `dxgi_detect_returns_ok` | Same invariant for DXGI detector |
| `sysfs.rs` (mod tests) | `sysfs_detect_returns_ok_on_absent_dir` | Same invariant for sysfs detector |
| `nvml.rs` (mod tests) | `nvml_detect_returns_ok`, `nvml_init_fallback_no_library` | NVML detector still returns Ok |
| `lib.rs` (mod tests) | All 20+ existing tests | No regression: override, mock, vendor mapping, caps OR-ing, host info all pass |

## CI Impact

No CI workflow files are modified. The task only touches source code in `anvilml-hardware`. However, the `--features mock-hardware` test path exercises the `enumerate_gpus()` function (which is gated behind `#[cfg(not(feature = "mock-hardware"))]`) — so under `mock-hardware`, `enumerate_gpus()` is never compiled. The changes to `lib.rs`'s `enumerate_gpus()` will only be verified by native compilation (without mock-hardware). All other changes are in modules that compile under both feature configurations.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Per-device `enumerate_device_extension_properties` call may fail on some drivers | The call is made inside the per-device loop; if it fails, the extension set will be empty and both pNext chains are skipped (safe fallback to basic properties). No error propagation needed. |
| Removing `log` dependency might affect other crates that depend on `anvilml-hardware` and transitively use `log` via this crate | `log` is a direct dependency of `anvilml-hardware`, not re-exported. Other crates depend on `log` directly. Removing it from this crate's Cargo.toml has no transitive impact. |
| `HashSet` allocation in per-device loop may be slow | Only called once per physical device (not per-frame); negligible overhead during startup enumeration. |
| Existing tests use `#[serial]` and may have ordering dependencies with logging output | Tests assert on return values, not log output. `tracing::warn!` calls are side-effect-only and do not change return behavior. |
| AMD ICD rejection of instance extensions is the root cause — but some drivers may silently ignore invalid extensions | The fix is spec-compliant regardless. Drivers that silently ignored the bad extensions will continue to work; drivers that reject (like AMD) will now succeed because valid extensions are no longer passed at instance level. |

## Acceptance Criteria

- [ ] `cargo clippy --workspace -- -D warnings` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] Vulkan instance creation no longer includes `VK_KHR_driver_properties` or `VK_EXT_MEMORY_BUDGET` in the enabled extension list
- [ ] `tracing::warn!` is present at every silent discard site across vulkan.rs, dxgi.rs, sysfs.rs, nvml.rs, and lib.rs
- [ ] No `log::` calls remain in `crates/anvilml-hardware/src/`
- [ ] `tracing` dependency added to `anvilml-hardware/Cargo.toml`, `log` dependency removed
