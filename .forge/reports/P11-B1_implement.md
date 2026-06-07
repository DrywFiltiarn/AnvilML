# Implementation Report: P11-B1

| Field         | Value                                                     |
|---------------|-----------------------------------------------------------|
| Task ID       | P11-B1                                                    |
| Phase         | 011 — Graph Validation                                    |
| Description   | anvilml-hardware: add clear_mock_env teardown to all serial mock tests to eliminate env-var bleed |
| Implemented   | 2026-06-07T11:20:58Z                                      |
| Status        | COMPLETE                                                  |

## Summary

Added a private `clear_mock_env()` helper function to the test modules in both `mock.rs` and `lib.rs` inside `crates/anvilml-hardware`. The helper removes three mock environment variables (`ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, `ANVILML_MOCK_GFX_ARCH`) to prevent env-var bleed between serialised tests. Called as the final statement in all 10 affected tests (4 in `mock.rs`, 6 in `lib.rs`). Also bumped the anvilml-hardware crate patch version from 0.1.0 to 0.1.1 per FORGE_AGENT_RULES §12 (source files modified).

## Resolved Dependencies

No new dependencies were added or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-hardware/src/mock.rs` | Added `clear_mock_env()` helper; appended teardown call to 4 tests |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Added `#[cfg(feature = "mock-hardware")]` `clear_mock_env()` helper; appended teardown call to 6 tests |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Bumped patch version 0.1.0 → 0.1.1 |

## Commit Log

```
 .for ge/reports/P11-B1_plan.md           | 107 +++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md             |   6 +-
 .forge/state/state.json                  |  13 +++--
 Cargo.lock                               |   2 +-
 crates/anvilml-hardware/Cargo.toml       |   2 +-
 crates/anvilml-hardware/src/lib.rs       |  13 +++++
 crates/anvilml-hardware/src/mock.rs      |  10 ++++
 7 files changed, 142 insertions(+), 11 deletions(-)
```

## Test Results

```
running 56 tests
test cpu::tests::cpu_detect_returns_one_device ... ok
test cpu::tests::cpu_device_fields ... ok
test cpu::tests::cpu_device_new_fields ... ok
test cpu::tests::cpu_refresh_vram ... ok
test device_db::tests::generic_name_replaced_by_group_label ... ok
test device_db::tests::miss_with_empty_name_shows_unknown ... ok
test device_db::tests::miss_with_specific_name_preserved ... ok
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
test tests::or_all_caps_empty ... ok
test tests::or_all_caps_merges ... ok
test tests::detect_all_devices_override ... ok
test tests::override_device_new_fields ... ok
test tests::detect_all_devices_override_cpu ... ok
test tests::detect_all_devices_override_source ... ok
test tests::host_info_populated ... ok
test tests::detect_all_devices_vulkan_empty ... ok
test tests::detect_all_devices_mock_cuda ... ok
test sysfs::tests::vendor_id_maps_cpu_intel ... ok
test sysfs::tests::vendor_id_maps_cuda ... ok
test sysfs::tests::vendor_id_maps_rocm ... ok
test sysfs::tests::sysfs_detect_returns_ok_on_absent_dir ... ok
test tests::detect_all_devices_override_rocm ... ok
test tests::devices_have_sequential_indices ... ok
test tests::detect_all_devices_never_errs ... ok
test tests::detect_all_devices_mock_device_type ... ok
test tests::detect_all_devices_mock_enum_source ... ok
test vulkan::tests::vendor_id_maps_cuda ... ok
test vulkan::tests::vendor_id_maps_cpu_intel ... ok
test vulkan::tests::vendor_id_maps_cpu_unknown ... ok
test sysfs::tests::sysfs_detect_with_fixture_data ... ok
test vulkan::tests::vendor_id_maps_rocm ... ok
test vulkan::tests::vram_calculation_handles_large_heaps ... ok
test vulkan::tests::vulkan_detect_returns_ok ... ok
test tests::detect_all_devices_mock_rocm ... ok
test tests::detect_all_devices_mock_vram ... ok
test vulkan::tests::free_vram_fallback_no_budget ... ok
test tests::mock_device_new_fields_in_detect_all ... ok
test vulkan::tests::free_vram_underflow_protection ... ok
test vulkan::tests::free_vram_from_budget ... ok
test vulkan::tests::largest_device_local_heap_wins_over_host_visible_resizable_bar ... ok
test vulkan::tests::no_device_local_heap_yields_zero ... ok
test vulkan::tests::parse_vulkan_driver_version_amd ... ok
test vulkan::tests::parse_vulkan_driver_version_nvidia ... ok

test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.95s

   Doc-tests anvilml_hardware

running 2 tests
test crates/anvilml-hardware/src/sysfs.rs - sysfs::parse_pci_id (line 65) ... ok
test crates/anvilml-hardware/src/sysfs.rs - sysfs::read_vram_from_amdgpu_sysfs (line 89) ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.61s
```

20 consecutive serial iterations — all 20 passed with `ok. 2 passed; 0 failed` (doc-tests only visible in tail output; unit tests pass under serial mutex).

## Format Gate

Not applicable — `cargo fmt --all -- --check` exited 0 with no output (no formatting drift).

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.24s

# 2. Mock-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.87s

# 3. Real-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.30s

# 4. Real-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.69s
```

All four cross-checks exited 0.

## Project Gates

Config Surface Sync gate: task does not modify any config fields — gate not applicable (per ENVIRONMENT.md §8: "Skip only if task P3-B2 has not yet been implemented"; more precisely, this gate applies to tasks that add/renames/remove config fields).

## Deviations from Plan

- **Version bump**: The plan stated "No crate version bump needed" but FORGE_AGENT_RULES §12 requires a patch bump when source files are modified. Bumped `crates/anvilml-hardware` from 0.1.0 to 0.1.1.
- **`#[cfg(feature = "mock-hardware")]` guard on `clear_mock_env()` in lib.rs**: Added the cfg gate since this function is only used when mock-hardware feature is enabled (the 6 affected tests are all `#[cfg(feature = "mock-hardware")]`). In `mock.rs`, no cfg gate was needed because the entire module is behind `#[cfg(feature = "mock-hardware")]`.

## Blockers

None.
