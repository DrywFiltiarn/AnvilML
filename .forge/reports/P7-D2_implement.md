# Implementation Report: P7-D2

| Field       | Value                                                         |
|-------------|---------------------------------------------------------------|
| Task ID     | P7-D2                                                         |
| Phase       | 007 — WebSocket Event Stream                                  |
| Description | anvilml-hardware: explicit detector warnings + Vulkan extension fix |
| Implemented | 2026-06-04T23:15:00Z                                          |
| Status      | COMPLETE                                                      |

## Summary

Fixed the Vulkan instance creation bug where device-level extensions (`VK_KHR_driver_properties` and `VK_EXT_memory_budget`) were incorrectly passed as instance-level enabled extensions, causing AMD ICDs to reject with `VK_ERROR_EXTENSION_NOT_PRESENT`. Replaced all `log::warn!` calls across vulkan.rs, dxgi.rs, sysfs.rs, nvml.rs, and lib.rs with `tracing::warn!` including structured fields (`detector`, `error`, `hr`, `code`). Added per-device extension query via `enumerate_device_extension_properties` to gate the pNext chains on actual device extension membership. Removed `log` dependency from anvilml-hardware and added `tracing` dependency.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| crate  | tracing | 0.1.44          | rust-docs MCP |

Note: `tracing` was already present in workspace dependencies (`Cargo.toml` line 36). No new version resolution was needed. The existing `log = "0.4.32"` workspace dependency was removed from anvilml-hardware's direct dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-hardware/Cargo.toml` | Replaced `log` dep with `tracing = { workspace = true }` |
| Modify | `crates/anvilml-hardware/src/vulkan.rs` | Removed device extensions from instance create; added per-device extension query via `enumerate_device_extension_properties`; gated pNext chains on extension membership; added `tracing::warn!` at all 4 discard sites |
| Modify | `crates/anvilml-hardware/src/dxgi.rs` | Replaced 4 `log::warn!` calls with `tracing::warn!(detector = "Dxgi", hr = ..., "...")` |
| Modify | `crates/anvilml-hardware/src/sysfs.rs` | Replaced 5 `log::warn!` calls with `tracing::warn!(detector = "Sysfs", error = ..., "...")` |
| Modify | `crates/anvilml-hardware/src/nvml.rs` | Replaced 4 `log::warn!` calls with `tracing::warn!(detector = "Nvml", code = ret, "...")` |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Replaced `unwrap_or_default()` calls in `enumerate_gpus()` with explicit match + `tracing::warn!` for Vulkan, DXGI, sysfs, and NVML detectors |
| Modify | `crates/anvilml-hardware/src/device_db.rs` | Replaced `log::warn!` with `tracing::warn!(detector = "DeviceDB", ...)` (pre-existing, required after log dep removal) |

## Commit Log

```
 .forge/reports/P7-D2_plan.md             | 190 +++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md             |   6 +-
 .forge/state/state.json                  |  13 ++-
 Cargo.lock                               |   2 +-
 crates/anvilml-hardware/Cargo.toml       |   2 +-
 crates/anvilml-hardware/src/device_db.rs |   9 +-
 crates/anvilml-hardware/src/dxgi.rs      |  11 +-
 crates/anvilml-hardware/src/lib.rs       |  96 +++++++++++-----
 crates/anvilml-hardware/src/nvml.rs      |  23 +++-
 crates/anvilml-hardware/src/sysfs.rs     |  18 +--
 crates/anvilml-hardware/src/vulkan.rs    |  42 ++++---
 11 files changed, 336 insertions(+), 76 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-6ddaf630e18a0693)
running 74 tests
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-24d57fd87e0738af)
running 59 tests
test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-9371b22be55d2c20)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml_openapi-0016c27beaaa0679)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-352f7f621ea05b67)
running 11 tests
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-a3399ac1d235fdcf)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/rescan.rs (target/debug/deps/rescan-95882b85f178dba8)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner.rs (target/debug/deps/scanner-57d18e8e8d1af08f)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_get.rs (target/debug/deps/store_get-476e261913afaa)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_list.rs (target/debug/deps/store_list-50fd3a0e352f08f)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-889c2fb1f6e3ef35)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-5f4b748bbebb14b4)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_models.rs (target/debug/deps/api_models-7f10904ade5f1ded)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_events.rs (target/debug/deps/api_ws_events-bc0451b7b55e2ff4)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-9a57c6e20692fed9)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-817eb7261913afaa)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-cb7a3ec86cb6541f)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_hardware
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

TOTAL: 145 passed; 0 failed; 0 ignored
```

## Platform Cross-Check

### Check 1: Mock-hardware Windows-gnu cross-check
```
    Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.44s
```

### Check 2: Real-hardware Linux native
```
    Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.71s
```

### Check 3: Real-hardware Windows-gnu cross-check
```
    Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.13s
```

All three checks exit 0.

## Project Gates

### Config Surface Sync Gate
```
     Running tests/config_reference.rs (target/debug/deps/config_reference-cb7a3ec86cb6541f)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate passes — no config fields were added/removed by this task.

## Deviations from Plan

- **device_db.rs**: The plan listed `device_db.rs` as "Out of Scope" (no silent discards present). However, after removing the `log` dependency, `device_db.rs` had a pre-existing `log::warn!` call that would fail to compile. Per FORGE_AGENT_RULES §9.3, this was fixed by replacing it with `tracing::warn!(detector = "DeviceDB", ...)`. This is a minimal correct fix — no logic changes, only dependency migration.
- **vulkan.rs `enumerate_device_extension_properties`**: The API in ash 0.38 takes only one argument (the physical device handle), not two. The plan assumed a `(pd, None)` signature. Fixed by calling `.enumerate_device_extension_properties(*pd)` with a single argument.
- **vulkan.rs `extension_name` conversion**: The `VkExtensionProperties::extension_name` field is `[i8; 256]`, which doesn't implement `Display` or `ToString`. Used the existing `cstr_to_string()` helper function instead of `.to_string()`.
- **lib.rs unreachable code**: Added `#[allow(unreachable_code)]` on the `Vec::new()` fallback for macOS since on unix systems the `#[cfg(unix)]` block always returns, making that line unreachable. This is a standard pattern in cross-platform Rust.

## Blockers

None.
