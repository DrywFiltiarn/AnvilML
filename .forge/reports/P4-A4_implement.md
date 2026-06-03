# Implementation Report: P4-A4

| Field         | Value                                           |
|---------------|-------------------------------------------------|
| Task ID       | P4-A4                                           |
| Phase         | 004 — Hardware Detection                        |
| Description   | anvilml-hardware: DXGI (Windows) + sysfs/NVML (Linux) fallback enumerators |
| Implemented   | 2026-06-03T11:30:00Z                            |
| Status        | COMPLETE                                        |

## Summary

Implemented three fallback GPU device enumerators for `anvilml-hardware`: a DXGI enumerator (Windows, gated behind `#[cfg(windows)]`) that reads adapter info via COM/IDXGIFactory1; a sysfs enumerator (Linux/unix, gated behind `#[cfg(unix)]`) that parses `/sys/bus/pci/devices/*/` PCI config space; and an NVML enumerator (Unix, gated behind `#[cfg(unix)]`) that uses `libloading` to dynamically load `libnvidia-ml.so` at runtime. All three implement the `DeviceDetector` trait, return `Ok(vec![])` when absent or errored, use `log::warn!` for per-device failures, and include fixture-driven unit tests. The Windows cross-check (`x86_64-pc-windows-gnu`) passes cleanly with proper cfg gating (dxgi compiles, sysfs/nvml excluded).

## Resolved Dependencies

| Type   | Name      | Version resolved | Source           |
|--------|-----------|-----------------|------------------|
| crate  | winapi    | 0.3             | rust-docs MCP + Cargo.lock (0.3.9) |
| crate  | libloading| 0.8             | rust-docs MCP (0.9.0 latest, lockfile has 0.8.9) |
| crate  | log       | 0.4             | New dependency — latest stable from crates.io |

**Dependency notes:**
- `nvml-wrapper` was not found via rust-docs MCP (404). Per the approved plan, fell back to `libloading` + raw FFI approach for NVML, which avoids any system library linking requirement.
- `winapi` 0.3.9 was already present in the workspace lockfile as a transitive dependency. Used specific features: `dxgi`, `combaseapi`, `objbase`, `winerror`.
- `log` 0.4 is a new runtime dependency added for warning-level logging from all three enumerators.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-hardware/src/dxgi.rs` | DxgiDetector: Windows DXGI IDXGIFactory1 GPU enumerator with COM guard, vendor→DeviceType mapping, and unit tests |
| Create | `crates/anvilml-hardware/src/sysfs.rs` | SysfsDetector: Linux PCI sysfs enumeration with parse helpers (`parse_pci_id`, `read_vram_from_amdgpu_sysfs`), amdgpu VRAM reader, and fixture-driven tests |
| Create | `crates/anvilml-hardware/src/nvml.rs` | NvmlDetector: Unix NVML lazy-loaded enumerator via libloading + raw FFI with NvmlLibrary wrapper, PCI/memory info parsing, and graceful fallback tests |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Added `log = "0.4"` dependency; added cfg-gated `[target.'cfg(windows)'.dependencies]` for winapi; added cfg-gated `[target.'cfg(unix)'.dependencies]` for libloading |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Added `pub mod dxgi;` (cfg windows), `pub mod sysfs;` (cfg unix), `pub mod nvml;` (cfg unix); added compile-check tests for all three new modules |

## Commit Log

```
 .forge/reports/P4-A4_plan.md         | 116 +++++++++
 .forge/state/CURRENT_TASK.md         |   6 +-
 .forge/state/state.json              |  13 +-
 Cargo.lock                           |   3 +
 crates/anvilml-hardware/Cargo.toml   |  14 +
 crates/anvilml-hardware/src/dxgi.rs  | 253 ++++++++++++++++++
 crates/anvilml-hardware/src/lib.rs   |  45 +++-
 crates/anvilml-hardware/src/nvml.rs  | 351 +++++++++++++++++++++++++
 crates/anvilml-hardware/src/sysfs.rs | 491 +++++++++++++++++++++++++++++++++++
 9 files changed, 1282 insertions(+), 10 deletions(-)
```

## Test Results

### Linux native (cargo test -p anvilml-hardware --features mock-hardware)

```
running 37 tests
test cpu::tests::cpu_detect_returns_one_device ... ok
test cpu::tests::cpu_device_fields ... ok
test cpu::tests::cpu_refresh_vram ... ok
test mock::tests::mock_detect_cuda ... ok
test mock::tests::mock_detect_default_cpu ... ok
test mock::tests::mock_detect_rocm ... ok
test nvml::tests::nvml_all_devices_are_cuda ... ok
test nvml::tests::nvml_detect_returns_ok ... ok
test nvml::tests::nvml_library_load_fails_gracefully ... ok
test nvml::tests::nvml_init_fallback_no_library ... ok
test nvml::tests::nvml_shutdown_in_drop_no_panic ... ok
test sysfs::tests::parse_pci_ids_valid_hex ... ok
test sysfs::tests::read_vram_helper_converts_bytes_to_mib ... ok
test sysfs::tests::vendor_id_maps_cuda ... ok
test tests::cpu_detector_implements_trait ... ok
test sysfs::tests::sysfs_detect_returns_ok_on_absent_dir ... ok
test sysfs::tests::vendor_id_maps_cpu_intel ... ok
test sysfs::tests::vendor_id_maps_cpu_unknown ... ok
test tests::nvml_detector_implements_trait ... ok
test tests::sysfs_detector_implements_trait ... ok
test sysfs::tests::sysfs_detect_with_fixture_data ... ok
test sysfs::tests::vendor_id_maps_rocm ... ok
test vulkan::tests::free_vram_from_budget ... ok
test vulkan::tests::free_vram_fallback_no_budget ... ok
test vulkan::tests::free_vram_underflow_protection ... ok
test vulkan::tests::largest_device_local_heap_wins_over_host_visible_resizable_bar ... ok
test vulkan::tests::parse_vulkan_driver_version_zero ... ok
test vulkan::tests::parse_vulkan_driver_version_amd ... ok
test vulkan::tests::no_device_local_heap_yields_zero ... ok
test vulkan::tests::parse_vulkan_driver_version_nvidia ... ok
test vulkan::tests::vendor_id_maps_cuda ... ok
test vulkan::tests::vram_calculation_handles_large_heaps ... ok
test vulkan::tests::vendor_id_maps_cpu_unknown ... ok
test vulkan::tests::vendor_id_maps_rocm ... ok
test tests::vulkan_detector_implements_trait ... ok
test vulkan::tests::vendor_id_maps_cpu_intel ... ok

test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Doc-tests anvilml_hardware
running 2 tests
test crates/anvilml-hardware/src/sysfs.rs - sysfs::read_vram_from_amdgpu_sysfs (line 89) ... ok
test crates/anvilml-hardware/src/sysfs.rs - sysfs::parse_pci_id (line 65) ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Full workspace test suite

```
anvilml-core:      68 passed, 0 failed
anvilml-hardware:  37 passed, 0 failed
anvilml-ipc:        0 passed, 0 failed
anvilml-openapi:    0 passed, 0 failed
anvilml-registry:   0 passed, 0 failed
anvilml-scheduler:  0 passed, 0 failed
anvilml-server:     2 passed, 0 failed
anvilml-worker:     0 passed, 0 failed
anvilml (binary):   8 passed, 0 failed
config_reference:   1 passed, 0 failed
Doc-tests anvilml_hardware: 2 passed, 0 failed

Total: 118 passed; 0 failed
```

## Windows Cross-Check

```
cargo check --target x86_64-pc-windows-gnu --features mock-hardware

Compiling winapi v0.3.9
Checking ntapi v0.4.3
Checking sysinfo v0.32.1
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.99s
```

Clean build with zero errors and zero warnings. The dxgi module compiles under the mingw target, while sysfs and nvml are properly excluded by `#[cfg(unix)]` gates.

## Config Drift Gate

```
cargo test -p backend --features mock-hardware -- test_toml

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out
```

## Deviations from Plan

- **NVML implementation approach**: The approved plan suggested `nvml-wrapper` crate, but the rust-docs MCP server returned 404 (crate not found/unavailable). Per the plan's risk mitigation section, fell back to `libloading` + raw FFI approach for NVML, which avoids any system library linking requirement. This is actually better than the plan since it requires zero system dependencies.
- **DXGI API path**: The plan mentioned `IDXGIFactory1::EnumAdapters(0)` and `GetDesc3()`/`GetName()`. However, winapi 0.3.9 does not have `GetDesc3` or `GetName` methods on the `IDXGIAdapter` interface. Instead, used `GetDesc()` which returns `DXGI_ADAPTER_DESC` containing a `Description` field (WCHAR[128]) with the adapter name — this provides all needed data in a single call without needing separate GetName/GetDesc3 calls.
- **winapi features**: The plan mentioned `dxgi1_4` and `d3dcommon` features, but the actual winapi 0.3.9 feature names are different. Used `dxgi`, `combaseapi`, `objbase`, and `winerror` features which provide the correct modules.
- **No EnumerationSource field**: The plan noted that P4-B2 will add the `EnumerationSource` enum type later, so these detectors populate `GpuDevice` with existing 6 fields only (no source tracking). This matches the plan's scope constraint.

## Blockers

None. All tests pass, cross-check is clean, and config drift gate passes.
