# Implementation Report: P6-C1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P6-C1                              |
| Phase         | 006 — Model Registry               |
| Description   | anvilml-core: add db_name field to GpuDevice |
| Implemented   | 2026-06-16T00:35:00Z               |
| Status        | COMPLETE                           |

## Summary

Added a `pub db_name: Option<String>` field to the `GpuDevice` struct in `anvilml-core`, positioned immediately after the `name` field. Updated all 10 `GpuDevice` struct literals across the workspace (`anvilml-core`, `anvilml-hardware`, `anvilml-server`) to initialise the new field as `None`. Bumped `anvilml-core` version from `0.1.12` to `0.1.13` and `anvilml-hardware` from `0.1.6` to `0.1.7`. Added a roundtrip assertion in `test_hardware_info_json_roundtrip` to verify `db_name` serialises and deserialises correctly. All 103 workspace tests pass, all 4 platform cross-checks pass, format and lint gates are clean.

## Resolved Dependencies

None. This task adds no new external crates or packages.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/hardware.rs` | Added `pub db_name: Option<String>` field to `GpuDevice` with doc comment |
| Modify | `crates/anvilml-core/Cargo.toml` | Bumped patch version `0.1.12 → 0.1.13` |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Bumped patch version `0.1.6 → 0.1.7` |
| Modify | `crates/anvilml-core/tests/hardware_tests.rs` | Added `db_name: None` to 2 GpuDevice literals; added roundtrip assertion for `db_name` |
| Modify | `crates/anvilml-hardware/src/detect.rs` | Added `db_name: None` to override-path GpuDevice literal |
| Modify | `crates/anvilml-hardware/src/mock.rs` | Added `db_name: None` to mock GpuDevice literal |
| Modify | `crates/anvilml-hardware/src/vulkan.rs` | Added `db_name: None` to Vulkan-detected GpuDevice literal |
| Modify | `crates/anvilml-hardware/src/cpu.rs` | Added `db_name: None` to CPU-detected GpuDevice literal |
| Modify | `crates/anvilml-hardware/src/dxgi.rs` | Added `db_name: None` to DXGI-detected GpuDevice literal |
| Modify | `crates/anvilml-hardware/src/sysfs.rs` | Added `db_name: None` to sysfs-detected GpuDevice literal |
| Modify | `crates/anvilml-hardware/tests/device_db_tests.rs` | Added `db_name: None` to 6 GpuDevice literals |
| Modify | `crates/anvilml-server/tests/system_tests.rs` | Added `db_name: None` to test GpuDevice literal |

## Commit Log

```
 .forge/reports/P6-C1_plan.md                     | 151 +++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                     |   6 +-
 .forge/state/state.json                          |  13 +-
 Cargo.lock                                       |   4 +-
 crates/anvilml-core/Cargo.toml                   |   2 +-
 crates/anvilml-core/src/types/hardware.rs        |   6 +
 crates/anvilml-core/tests/hardware_tests.rs      |   3 +
 crates/anvilml-hardware/Cargo.toml               |   2 +-
 crates/anvilml-hardware/src/cpu.rs               |   1 +
 crates/anvilml-hardware/src/detect.rs            |   1 +
 crates/anvilml-hardware/src/dxgi.rs              |   1 +
 crates/anvilml-hardware/src/mock.rs              |   1 +
 crates/anvilml-hardware/src/sysfs.rs             |   1 +
 crates/anvilml-hardware/src/vulkan.rs            |   1 +
 crates/anvilml-hardware/tests/device_db_tests.rs |   6 +
 crates/anvilml-server/tests/system_tests.rs      |   1 +
 16 files changed, 187 insertions(+), 13 deletions(-)
```

## Test Results

```
     Running tests/hardware_tests.rs (target/debug/deps/hardware_tests-f3b2943142e585bb)

running 4 tests
test test_device_type_variants ... ok
test test_enum_variants_roundtrip ... ok
test test_inference_caps_default ... ok
test test_hardware_info_json_roundtrip ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/device_db_tests.rs (target/debug/deps/device_db_tests-cd76aea7aefc1b7f)

running 7 tests
test test_device_db_non_empty ... ok
test test_resolve_amd_rdna3 ... ok
test test_resolve_cpu_fallback ... ok
test test_resolve_name_overwrite ... ok
test test_resolve_nvidia_ampere ... ok
test test_resolve_unknown_device ... ok
test test_resolve_vram_untouched ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/system_tests.rs (target/debug/deps/system_tests-8db93848b1ba7bd0)

running 2 tests
test test_system_env_returns_200_with_default_report ... ok
test test_system_returns_200_with_hardware_info ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Full workspace: 103 tests passed; 0 failed
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
1. Mock-hardware Linux:     Finished `dev` profile [...] target(s) in 0.24s
2. Mock-hardware Windows:   Finished `dev` profile [...] target(s) in 4.74s
3. Real-hardware Linux:     Finished `dev` profile [...] target(s) in 3.61s
4. Real-hardware Windows:   Finished `dev` profile [...] target(s) in 2.36s
All four checks exited 0.
```

## Project Gates

```
Gate 1 — Config Surface Sync:
  cargo test -p anvilml --features mock-hardware -- config_reference
  Running tests/config_reference.rs
  test config_reference ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 2 (OpenAPI Drift) and Gate 3 (Node Parity) are not triggered by this task — no handler signatures, `ToSchema` derives, or node types were modified.

## Public API Delta

```
+    pub db_name: Option<String>,
```

One new `pub` field introduced: `GpuDevice::db_name` of type `Option<String>` in module `anvilml_core::types::hardware`. This extends an existing public struct — no new `pub fn`, `pub struct`, `pub enum`, `pub trait`, `pub const`, or `pub type` items were added.

## Deviations from Plan

None. All changes were implemented exactly as specified in the approved plan.

## Blockers

None.
