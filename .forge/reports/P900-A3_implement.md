# Implementation Report: P900-A3

| Field       | Value                                                         |
|-------------|---------------------------------------------------------------|
| Task ID     | P900-A3                                                       |
| Phase       | 900 — Logging Retrofit                                        |
| Description | anvilml-hardware: retrofit DEBUG fallback log to lib.rs (Vulkan→DXGI/sysfs fallback) |
| Implemented | 2026-06-06T00:12:00Z                                          |
| Status      | COMPLETE                                                      |

## Summary

Added two `#[cfg]`-gated `tracing::debug!` calls inside the `enumerate_gpus()` function's Vulkan empty-result arm (`Ok(_)` branch) in `crates/anvilml-hardware/src/lib.rs`. On Windows, a debug log fires with `fallback = "dxgi"`; on Unix, with `fallback = "sysfs_nvml"`. This satisfies FORGE_AGENT_RULES §11.5's mandatory DEBUG log point for hardware detection fallback paths. The code is behind `#[cfg(not(feature = "mock-hardware"))]` so it does not affect mock-hardware builds.

## Resolved Dependencies

N/A — no new dependencies added or modified. The `tracing` crate was already a workspace dependency used in this crate (evidenced by existing `tracing::warn!` calls at lines 116–123).

## Files Changed

| Action | Path                                      | Description |
|--------|-------------------------------------------|-------------|
| Modify | `crates/anvilml-hardware/src/lib.rs`      | Added two cfg-gated `tracing::debug!` calls in `enumerate_gpus()` Vulkan empty-result arm (lines 120–126) |

## Commit Log

```
 .forge/reports/P900-A3_plan.md     | 87 ++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md       |  6 +--
 .forge/state/state.json            | 13 +++---
 crates/anvilml-hardware/src/lib.rs |  7 +++
 4 files changed, 104 insertions(+), 9 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-33040787c2a7b0ce)

running 56 tests
test cpu::tests::cpu_detect_returns_one_device ... ok
test cpu::tests::cpu_device_fields ... ok
test cpu::tests::cpu_device_new_fields ... ok
test cpu::tests::cpu_refresh_vram ... ok
test device_db::tests::generic_name_replaced_by_group_label ... ok
test device_db::tests::miss_with_empty_name_shows_unknown ... ok
test device_db::tests::miss_with_specific_name_preserved ... ok
test device_db::tests::specific_vulkan_name_preserved ... ok
test mock::tests::mock_detect_cuda ... ok
test mock::tests::mock_detect_default_cpu ... ok
test mock::tests::mock_detect_rocm ... ok
test mock::tests::mock_device_new_fields ... ok
test nvml::tests::nvml_all_devices_are_cuda ... ok
test nvml::tests::nvml_detect_returns_ok ... ok
test nvml::tests::nvml_init_fallback_no_library ... ok
test nvml::tests::nvml_library_load_fails_gracefully ... ok
test nvml::tests::nvml_shutdown_in_drop_no_panic ... ok
test sysfs::tests::parse_pci_ids_valid_hex ... ok
test sysfs::tests::read_vram_helper_converts_bytes_to_mib ... ok
test sysfs::tests::vendor_id_maps_rocm ... ok
test tests::or_all_caps_empty ... ok
test tests::or_all_caps_merges ... ok
test sysfs::tests::sysfs_detect_with_fixture_data ... ok
test sysfs::tests::vendor_id_maps_cpu_intel ... ok
test sysfs::tests::vendor_id_maps_cpu_unknown ... ok
test sysfs::tests::vendor_id_maps_cuda ... ok
test sysfs::tests::sysfs_detect_returns_ok_on_absent_dir ... ok
test tests::detect_all_devices_vulkan_empty ... ok
test tests::detect_all_devices_override ... ok
test tests::detect_all_devices_mock_device_type ... ok
test tests::detect_all_devices_override_cpu ... ok
test tests::host_info_populated ... ok
test tests::detect_all_devices_override_source ... ok
test tests::devices_have_sequential_indices ... ok
test tests::override_device_new_fields ... ok
test tests::detect_all_devices_override_rocm ... ok
test tests::detect_all_devices_never_errs ... ok
test tests::detect_all_devices_mock_cuda ... ok
test vulkan::tests::vulkan_detect_returns_ok ... ok
test tests::detect_all_devices_mock_enum_source ... ok
test tests::detect_all_devices_mock_rocm ... ok
test tests::detect_all_devices_mock_vram ... ok
test tests::mock_device_new_fields_in_detect_all ... ok
test vulkan::tests::free_vram_fallback_no_budget ... ok
test vulkan::tests::free_vram_from_budget ... ok
test vulkan::tests::largest_device_local_heap_wins_over_host_visible_resizable_bar ... ok
test vulkan::tests::no_device_local_heap_yields_zero ... ok
test vulkan::tests::parse_vulkan_driver_version_amd ... ok
test vulkan::tests::free_vram_underflow_protection ... ok
test vulkan::tests::parse_vulkan_driver_version_zero ... ok
test vulkan::tests::parse_vulkan_driver_version_nvidia ... ok
test vulkan::tests::vendor_id_maps_cpu_intel ... ok
test vulkan::tests::vendor_id_maps_cpu_unknown ... ok
test vulkan::tests::vendor_id_maps_cuda ... ok
test vulkan::tests::vendor_id_maps_rocm ... ok
test vulkan::tests::vram_calculation_handles_large_heaps ... ok

test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.11s

Full workspace: 198 tests across all crates, 0 failed.
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.25s

# 2. Mock-hardware Windows cross-check (x86_64-pc-windows-gnu)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.62s

# 3. Real-hardware Linux check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.27s

# 4. Real-hardware Windows cross-check (x86_64-pc-windows-gnu)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.64s

All four checks exited 0.
```

## Project Gates

```
Gate 1 — Config Surface Sync:
    Running tests/config_reference.rs
    running 1 test
    test test_toml_key_set_matches_default ... ok
    test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- `cargo fmt` reformatted the `#[cfg(unix)]` debug call from a single-line form to a multi-line form (lines 123–126 in the final file) because the line exceeded the formatter's column limit. This is expected cargo fmt behavior — the semantics are identical and the structured `fallback = "sysfs_nvml"` field notation is preserved exactly.

## Blockers

None.
