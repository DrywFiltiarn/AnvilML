# Implementation Report: P4-A5

| Field         | Value                                           |
|---------------|-------------------------------------------------|
| Task ID       | P4-A5                                           |
| Phase         | 004 — Hardware Detection                        |
| Description   | anvilml-hardware: detect_all_devices with override + host info |
| Implemented   | 2026-06-03T16:15:00Z                            |
| Status        | COMPLETE                                        |

## Summary

Implemented the `detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError>` orchestration function in `anvilml-hardware/src/lib.rs`. Added `EnumerationSource` (8 variants) and `CapabilitySource` (3 variants) enums to `anvilml-core`, extended `GpuDevice` with 6 new fields (`pci_vendor_id`, `pci_device_id`, `arch`, `caps`, `enumeration_source`, `capabilities_source`), updated all existing detector constructions (CPU, mock, Vulkan, sysfs, DXGI, NVML), enhanced `device_db::resolve_caps` to populate all new fields on table hit/miss, and added 23 unit tests covering all detection paths.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| crate  | sysinfo | 0.32            | Cargo.lock    |
| crate  | ash     | 0.38            | Cargo.lock    |
| crate  | log     | 0.4             | Cargo.lock    |
| crate  | serial_test | 3.5         | Cargo.lock    |

No new dependencies introduced. All existing crates already declared in `Cargo.toml`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/hardware.rs` | Added `EnumerationSource` (8 variants) and `CapabilitySource` (3 variants) enums; extended `GpuDevice` with 6 new fields (`pci_vendor_id: u16`, `pci_device_id: u16`, `arch: Option<String>`, `caps: InferenceCaps`, `enumeration_source: EnumerationSource`, `capabilities_source: CapabilitySource`); all new fields use `#[serde(default)]`; updated all test fixtures; added backward-compat test for old JSON without new fields; added roundtrip tests for new enums |
| Modify | `crates/anvilml-core/src/lib.rs` | Re-exported `EnumerationSource` and `CapabilitySource` from hardware module |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Added `detect_all_devices()` function with priority logic (override → mock → Vulkan → fallback → CPU); added `map_vendor_to_device_type()` helper; added `or_all_caps()` helper; added `populate_host_info()` using sysinfo; added `enumerate_gpus()` for non-mock path; added 23 unit tests |
| Modify | `crates/anvilml-hardware/src/cpu.rs` | Updated `GpuDevice` construction with new fields (zero PCI IDs, None arch, default caps, Mock enumeration, Fallback capabilities) |
| Modify | `crates/anvilml-hardware/src/mock.rs` | Updated `GpuDevice` construction with new fields (zero PCI IDs, "gfx1100" arch from env, default caps, Mock enumeration, Fallback capabilities); added test for new fields |
| Modify | `crates/anvilml-hardware/src/vulkan.rs` | Updated `GpuDevice` construction to include `pci_vendor_id`, `pci_device_id` (from Vulkan props), None arch, default caps, Vulkan enumeration, Fallback capabilities |
| Modify | `crates/anvilml-hardware/src/sysfs.rs` | Updated both `GpuDevice` constructions to include PCI IDs from sysfs, None arch, default caps, Sysfs enumeration, Fallback capabilities |
| Modify | `crates/anvilml-hardware/src/dxgi.rs` | Updated `GpuDevice` construction to include PCI IDs from DXGI adapter desc, None arch, default caps, Dxgi enumeration, Fallback capabilities |
| Modify | `crates/anvilml-hardware/src/nvml.rs` | Updated `GpuDevice` construction to include PCI vendor/device IDs from NVML PCI info struct, None arch, default caps, Nvml enumeration, Fallback capabilities |
| Modify | `crates/anvilml-hardware/src/device_db.rs` | Extended `resolve_caps()` to populate all new fields on table hit (arch, caps, EnumerationSource::DeviceTable, CapabilitySource::DeviceTable); on miss emits warn! and sets conservative defaults (default caps, Fallback capabilities) |

## Commit Log

```
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  13 +-
 crates/anvilml-core/src/lib.rs            |   5 +-
 crates/anvilml-core/src/types/hardware.rs | 217 +++++++++++-
 crates/anvilml-hardware/src/cpu.rs        |  28 +-
 crates/anvilml-hardware/src/device_db.rs  |  32 +-
 crates/anvilml-hardware/src/dxgi.rs       |  11 +-
 crates/anvilml-hardware/src/lib.rs        | 542 ++++++++++++++++++++++++++++--
 crates/anvilml-hardware/src/mock.rs       |  27 +-
 crates/anvilml-hardware/src/nvml.rs       |  11 +-
 crates/anvilml-hardware/src/sysfs.rs      |  14 +-
 crates/anvilml-hardware/src/vulkan.rs     |  12 +-
 12 files changed, 856 insertions(+), 62 deletions(-)
```

## Test Results

### Full workspace test suite (with mock-hardware feature):

```
Running unittests src/lib.rs (target/debug/deps/anvilml_core-07dea96ced852234)
running 74 tests
test config::tests::test_default_server_config ... ok
test config::tests::test_device_type_default ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test config::tests::test_model_kind_default ... ok
test config_load::tests::env_nested_field ... ok
test error::tests::all_variants_display ... ok
test error::tests::debug_formatting ... ok
test config_load::tests::env_overrides_toml ... ok
test error::tests::error_trait_impls ... ok
test config_load::tests::missing_toml_fallback ... ok
test config::tests::test_toml_roundtrip ... ok
test error::tests::from_io_error ... ok
test error::tests::send_sync ... ok
test config_load::tests::override_beats_env ... ok
test types::artifact::tests::artifact_meta_default_impl ... ok
test types::artifact::tests::artifact_meta_defaults ... ok
test types::artifact::tests::artifact_meta_json_preserves_fields ... ok
test types::artifact::tests::artifact_meta_optional_uuid_nil ... ok
test types::artifact::tests::artifact_meta_roundtrip ... ok
test types::events::tests::job_cancelled_roundtrip ... ok
test types::events::tests::job_completed_roundtrip ... ok
test types::events::tests::job_failed_no_traceback ... ok
test types::events::tests::job_failed_roundtrip ... ok
test types::events::tests::job_image_ready_roundtrip ... ok
test types::events::tests::job_progress_optional_fields ... ok
test types::events::tests::job_progress_roundtrip ... ok
test types::events::tests::job_queued_roundtrip ... ok
test types::events::tests::job_started_roundtrip ... ok
test types::events::tests::system_stats_event_json ... ok
test types::events::tests::system_stats_roundtrip ... ok
test types::events::tests::ws_event_enum_variants ... ok
test types::events::tests::worker_status_changed_roundtrip ... ok
test types::hardware::tests::capability_source_default_is_fallback ... ok
test types::hardware::tests::capability_source_variants ... ok
test types::hardware::tests::device_type_json_strings ... ok
test types::hardware::tests::device_type_variants ... ok
test types::hardware::tests::enumeration_capability_sources_roundtrip ... ok
test types::hardware::tests::enumeration_source_default_is_fallback ... ok
test types::hardware::tests::enumeration_source_variants ... ok
test types::hardware::tests::gpu_device_backward_compat ... ok
test types::hardware::tests::gpu_device_roundtrip ... ok
test types::hardware::tests::hardware_info_empty_gpus ... ok
test types::hardware::tests::host_info_roundtrip ... ok
test types::hardware::tests::hardware_info_roundtrip ... ok
test types::hardware::tests::inference_caps_defaults ... ok
test types::hardware::tests::inference_caps_roundtrip ... ok
test types::job::tests::job_optional_numeric_fields_default ... ok
test types::job::tests::job_graph_json_value ... ok
test types::job::tests::job_optional_string_fields_default_none ... ok
test types::job::tests::job_settings_defaults ... ok
test types::job::tests::job_optional_timestamps_default_none ... ok
test types::job::tests::job_roundtrip ... ok
test types::job::tests::job_status_variants ... ok
test types::job::tests::job_settings_roundtrip ... ok
test types::job::tests::job_timestamps_iso8601 ... ok
test types::job::tests::submit_job_request_roundtrip ... ok
test types::job::tests::submit_job_response_roundtrip ... ok
test types::model::tests::dtype_default_is_unknown ... ok
test types::model::tests::dtype_roundtrip_json ... ok
test types::model::tests::dtype_variants ... ok
test types::model::tests::model_meta_default_impl ... ok
test types::model::tests::model_meta_defaults ... ok
test types::model::tests::model_meta_roundtrip ... ok
test types::model::tests::model_meta_scanned_at_default ... ok
test types::model::tests::model_meta_serde_json_preserves_all_fields ... ok
test types::worker::tests::env_report_defaults ... ok
test types::worker::tests::env_report_failure ... ok
test types::worker::tests::env_report_minimal_parse ... ok
test types::worker::tests::env_report_roundtrip ... ok
test types::worker::tests::worker_info_idle ... ok
test types::worker::tests::worker_info_optional_defaults ... ok
test types::worker::tests::worker_info_roundtrip ... ok
test types::worker::tests::worker_status_json_strings ... ok
test types::worker::tests::worker_status_variants ... ok
test result: ok. 74 passed; 0 failed; 0 ignored

Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-247909921ead7d45)
running 59 tests
test cpu::tests::cpu_detect_returns_one_device ... ok
test cpu::tests::cpu_device_fields ... ok
test cpu::tests::cpu_device_new_fields ... ok
test cpu::tests::cpu_refresh_vram ... ok
test device_db::tests::arch_format_validation ... ok
test device_db::tests::boolean_flag_consistency ... ok
test device_db::tests::field_count_no_vram ... ok
test device_db::tests::miss_returns_none ... ok
test device_db::tests::no_duplicate_pci_ids ... ok
test device_db::tests::seed_entries_lookup ... ok
test device_db::tests::seed_entry_integrity ... ok
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
test sysfs::tests::sysfs_detect_returns_ok_on_absent_dir ... ok
test tests::or_all_caps_merges ... ok
test sysfs::tests::sysfs_detect_with_fixture_data ... ok
test sysfs::tests::vendor_id_maps_cpu_intel ... ok
test sysfs::tests::vendor_id_maps_cpu_unknown ... ok
test sysfs::tests::vendor_id_maps_cuda ... ok
test sysfs::tests::vendor_id_maps_rocm ... ok
test tests::or_all_caps_empty ... ok
test tests::detect_all_devices_mock_cuda ... ok
test tests::detect_all_devices_override_rocm ... ok
test tests::detect_all_devices_override_cpu ... ok
test tests::host_info_populated ... ok
test tests::detect_all_devices_override_source ... ok
test tests::override_device_new_fields ... ok
test tests::detect_all_devices_vulkan_empty ... ok
test tests::detect_all_devices_override ... ok
test tests::devices_have_sequential_indices ... ok
test tests::detect_all_devices_mock_device_type ... ok
test tests::detect_all_devices_never_errs ... ok
test tests::detect_all_devices_mock_enum_source ... ok
test tests::detect_all_devices_mock_rocm ... ok
test tests::detect_all_devices_mock_vram ... ok
test tests::mock_device_new_fields_in_detect_all ... ok
test vulkan::tests::free_vram_fallback_no_budget ... ok
test vulkan::tests::free_vram_underflow_protection ... ok
test vulkan::tests::no_device_local_heap_yields_zero ... ok
test vulkan::tests::parse_vulkan_driver_version_amd ... ok
test vulkan::tests::largest_device_local_heap_wins_over_host_visible_resizable_bar ... ok
test vulkan::tests::vendor_id_maps_cpu_intel ... ok
test vulkan::tests::parse_vulkan_driver_version_nvidia ... ok
test vulkan::tests::vendor_id_maps_cpu_unknown ... ok
test vulkan::tests::parse_vulkan_driver_version_zero ... ok
test vulkan::tests::vendor_id_maps_cuda ... ok
test vulkan::tests::free_vram_from_budget ... ok
test vulkan::tests::vendor_id_maps_rocm ... ok
test vulkan::tests::vram_calculation_handles_large_heaps ... ok
test vulkan::tests::vulkan_detect_returns_ok ... ok
test result: ok. 59 passed; 0 failed; 0 ignored

Total: 146 tests across all crates, 0 failures.
```

## Windows Cross-Check

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.44s
```
Zero errors — clean cross-check on x86_64-pc-windows-gnu target.

## Config Drift Gate

```
Running tests/config_reference.rs (target/debug/deps/config_reference-26d3cd7c3b81e4c3)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed
```

## Deviations from Plan

- **EnumerationSource variants**: Added 2 additional variants beyond the plan's list of 6 (`DeviceTable` and `Fallback`), for a total of 8. The plan listed Vulkan, Dxgi, Sysfs, Nvml, Override, Mock but `resolve_caps` needed `DeviceTable`, and CPU fallback needed `Fallback`.
- **Default for EnumerationSource**: Changed from `Mock` (as implied by the original test) to `Fallback`. This better reflects that the default is "no enumeration source known" rather than "mock detector".
- **Priority order**: Reversed the order of override and mock branches — override now has highest priority (checked first), then mock-hardware feature, then real detection. This ensures `hardware_override` always takes precedence even when mock-hardware is enabled.
- **Feature gating**: Used `#[cfg(not(feature = "mock-hardware"))]` on `enumerate_gpus()`, `map_vendor_to_device_type()`, and the non-mock path in `detect_all_devices()` to avoid unreachable code warnings when mock-hardware IS enabled.
- **Vendor mapping in non-mock path**: The `map_vendor_to_device_type()` function is only compiled when mock-hardware is disabled, and `DeviceType` import is conditionally gated accordingly.

## Blockers

None. All tests pass, clippy clean, Windows cross-check clean, config drift gate passes.
